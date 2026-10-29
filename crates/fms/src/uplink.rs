use serde_json::Value;

use crate::geo::Coordinates;
use crate::navdb::{Airway, Fix, NavDatabase};
use crate::plan::{FmsError, FmsResult};
use crate::service::{Command, Fms, PlanRef};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct NavlogFix {
    pub ident: String,
    pub fix_type: String,
    pub via_airway: String,
    pub is_sid_star: String,
    pub pos_lat: String,
    pub pos_long: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct OfpAirport {
    pub icao: String,
    pub runway: String,
    pub trans_alt: f64,
    pub trans_level: f64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Ofp {
    pub origin: OfpAirport,
    pub destination: OfpAirport,
    pub alternate: OfpAirport,
    pub navlog: Vec<NavlogFix>,
    pub cost_index: f64,
    pub cruise_altitude: f64,
    pub average_tropopause: String,
    pub callsign: String,
}

fn text(v: &Value, key: &str) -> String {
    match v.get(key) {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => n.to_string(),
        _ => String::new(),
    }
}

fn number(v: &Value, key: &str) -> f64 {
    text(v, key).trim().parse().unwrap_or(0.)
}

impl Ofp {
    pub fn from_simbrief_json(json: &str) -> Result<Ofp, String> {
        let v: Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
        let airport = |key: &str| {
            let a = v.get(key).cloned().unwrap_or(Value::Null);
            OfpAirport { icao: text(&a, "icao_code"), runway: text(&a, "plan_rwy"), trans_alt: number(&a, "trans_alt"), trans_level: number(&a, "trans_level") }
        };
        let navlog = v
            .get("navlog")
            .and_then(|n| n.get("fix").or(Some(n)))
            .and_then(|f| f.as_array())
            .map(|a| {
                a.iter()
                    .map(|f| NavlogFix {
                        ident: text(f, "ident"),
                        fix_type: text(f, "type"),
                        via_airway: text(f, "via_airway"),
                        is_sid_star: text(f, "is_sid_star"),
                        pos_lat: text(f, "pos_lat"),
                        pos_long: text(f, "pos_long"),
                    })
                    .collect()
            })
            .unwrap_or_default();
        let general = v.get("general").cloned().unwrap_or(Value::Null);
        let atc = v.get("atc").cloned().unwrap_or(Value::Null);
        Ok(Ofp {
            origin: airport("origin"),
            destination: airport("destination"),
            alternate: airport("alternate"),
            navlog,
            cost_index: number(&general, "costindex"),
            cruise_altitude: number(&general, "initial_altitude"),
            average_tropopause: text(&general, "avg_tropopause"),
            callsign: text(&atc, "callsign"),
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum RouteChunk {
    Airway { ident: String, hint: Coordinates },
    AirwayTermination { ident: String, hint: Coordinates },
    Waypoint { ident: String, hint: Coordinates },
    LatLong { lat: f64, long: f64 },
    Procedure { ident: String },
    SidEnrouteTransition { ident: String, hint: Coordinates },
}

fn hint(f: &NavlogFix) -> Coordinates {
    Coordinates::new(f.pos_lat.trim().parse().unwrap_or(f64::NAN), f.pos_long.trim().parse().unwrap_or(f64::NAN))
}

fn is_nat_track(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 4 && &b[..3] == b"NAT" && b[3].is_ascii_uppercase()
}

pub fn route_chunks(ofp: &Ofp) -> Vec<RouteChunk> {
    let mut out: Vec<RouteChunk> = Vec::new();
    for i in 0..ofp.navlog.len() {
        let last_fix = if i > 0 { ofp.navlog.get(i - 1) } else { None };
        let fix = &ofp.navlog[i];
        if fix.ident == "TOC" || fix.ident == "TOD" || fix.fix_type == "apt" {
            continue;
        }
        let last = out.last().cloned();
        if fix.is_sid_star == "1" {
            if !matches!(last, Some(RouteChunk::Procedure { .. })) {
                out.push(RouteChunk::Procedure { ident: fix.via_airway.clone() });
            }
        } else if matches!(&last, Some(RouteChunk::Procedure { ident }) if *ident == fix.via_airway) {
            out.push(RouteChunk::SidEnrouteTransition { ident: fix.ident.clone(), hint: hint(fix) });
        } else if fix.via_airway == "DCT" || fix.via_airway == "DCT*" || is_nat_track(&fix.via_airway) || out.is_empty() {
            if fix.fix_type == "ltlg" {
                out.push(RouteChunk::LatLong { lat: fix.pos_lat.trim().parse().unwrap_or(0.), long: fix.pos_long.trim().parse().unwrap_or(0.) });
            } else {
                out.push(RouteChunk::Waypoint { ident: fix.ident.clone(), hint: hint(fix) });
            }
        } else if !matches!(&last, Some(RouteChunk::Airway { ident, .. }) if *ident == fix.via_airway) {
            if let (Some(lf), Some(RouteChunk::Airway { .. })) = (last_fix, &last) {
                if fix.via_airway != lf.via_airway {
                    out.push(RouteChunk::AirwayTermination { ident: lf.ident.clone(), hint: hint(lf) });
                }
            }
            out.push(RouteChunk::Airway { ident: fix.via_airway.clone(), hint: hint(fix) });
        }
        if matches!(out.last(), Some(RouteChunk::Airway { .. })) && ofp.navlog.get(i + 1).map(|n| n.via_airway.as_str()) != Some(fix.via_airway.as_str()) {
            out.push(RouteChunk::AirwayTermination { ident: fix.ident.clone(), hint: hint(fix) });
        }
    }
    out
}

fn pick_fix(fixes: &[Fix], hint: &Coordinates) -> Fix {
    let mut best = 0;
    let mut min = f64::MAX;
    for (i, f) in fixes.iter().enumerate() {
        let d = f.location.coordinates().distance_to(hint);
        if d < min {
            min = d;
            best = i;
        }
    }
    fixes[best].clone()
}

fn pick_airway(airways: &[Airway], hint: &Coordinates) -> Airway {
    let mut best = 0;
    let mut min = f64::MAX;
    for (i, a) in airways.iter().enumerate() {
        let d = a.fixes[0].location.coordinates().distance_to(hint);
        if d < min {
            min = d;
            best = i;
        }
    }
    airways[best].clone()
}

pub fn uplink_flight_plan_from_simbrief(fms: &mut Fms, db: &dyn NavDatabase, ofp: &Ofp, log: &mut Vec<String>) -> FmsResult<()> {
    let up = PlanRef::Uplink;
    fms.execute(db, Command::NewCityPair { from: ofp.origin.icao.clone(), to: ofp.destination.icao.clone(), alternate: None, plan: up })?;
    if let Err(e) = fms.execute(db, Command::SetAlternate { icao: Some(ofp.alternate.icao.clone()), plan: up }) {
        log.push(format!("[SimBriefUplinkAdapter](uplinkFlightPlanFromSimbrief) Failed to set alternate: {e}"));
    }

    {
        let plan = fms.plan_mut(up)?;
        plan.performance.database_transition_altitude = Some(ofp.origin.trans_alt);
        plan.performance.database_transition_level = Some(ofp.destination.trans_level / 100.);
        plan.performance.cost_index = Some(ofp.cost_index);
        plan.performance.cruise_flight_level = Some(ofp.cruise_altitude / 100.);
        plan.performance.pilot_tropopause = ofp.average_tropopause.trim().parse::<i64>().ok().map(|t| t as f64);
        plan.flight_number = ofp.callsign.clone();
    }

    let mut insert_head = fms.plan(up).map(|p| p.insert_head_at_end_of_enroute()).unwrap_or(-1);

    for chunk in route_chunks(ofp) {
        match chunk {
            RouteChunk::Procedure { .. } => {}
            RouteChunk::SidEnrouteTransition { ident, hint } | RouteChunk::Waypoint { ident, hint } => {
                if insert_head == -1 {
                    insert_head = fms.plan(up).map(|p| p.insert_head_at_end_of_enroute()).unwrap_or(-1);
                }
                let fixes = db.search_fix(&ident);
                if !fixes.is_empty() {
                    let fix = if fixes.len() > 1 { pick_fix(&fixes, &hint) } else { fixes[0].clone() };
                    fms.execute(db, Command::NextWaypoint { index: insert_head as usize, waypoint: fix, plan: up })?;
                    insert_head += 1;
                } else {
                    log.push(format!("[SimBriefUplinkAdapter](uplinkFlightPlanFromSimbrief) Found no fixes for waypoint chunk: {ident}"));
                    let is_leg = fms.plan(up).and_then(|p| p.element_at(insert_head as usize)).is_some_and(|e| !e.is_disco());
                    if is_leg {
                        fms.execute(db, Command::InsertDiscontinuityAfter { index: insert_head as usize, plan: up })?;
                        insert_head += 1;
                    }
                }
            }
            RouteChunk::LatLong { lat, long } => {
                if insert_head == -1 {
                    insert_head = fms.plan(up).map(|p| p.insert_head_at_end_of_enroute()).unwrap_or(-1);
                }
                let ident = lat_long_ident(lat, long);
                let fix = Fix::lat_long(&ident, lat, long);
                fms.execute(db, Command::NextWaypoint { index: insert_head as usize, waypoint: fix, plan: up })?;
                insert_head += 1;
            }
            RouteChunk::Airway { ident, hint } => {
                let search_fix: Option<Fix>;
                let pending = fms.plan(up).and_then(|p| p.pending_airways.clone());
                match pending {
                    None => {
                        let element = fms.plan(up).and_then(|p| p.element_at(insert_head as usize)).cloned();
                        match element.as_ref().and_then(|e| e.leg()) {
                            Some(leg) => search_fix = leg.termination_waypoint().cloned(),
                            None => continue,
                        }
                        fms.plan_mut(up)?.start_airway_entry(insert_head as usize)?;
                    }
                    Some(p) => {
                        let tail = p.elements.last();
                        search_fix = tail.and_then(|t| t.to.clone().or_else(|| t.airway.as_ref().and_then(|a| a.fixes.last().cloned())));
                    }
                }
                let Some(search_fix) = search_fix else {
                    return Err(FmsError(format!("[SimBriefUplinkAdapter](uplinkFlightPlanFromSimbrief) Found no search fix for \"airway\" chunk: {ident}")));
                };
                let airways = db.search_airway(&ident, &search_fix);
                if !airways.is_empty() {
                    fms.plan_mut(up)?.continue_airway_entry_via_airway(pick_airway(&airways, &hint))?;
                } else {
                    log.push(format!("[SimBriefUplinkAdapter](uplinkFlightPlanFromSimbrief) Found no airways at fix \"{}\" for airway: \"{ident}\"", search_fix.ident));
                    fms.plan_mut(up)?.clear_pending_airways();
                }
            }
            RouteChunk::AirwayTermination { ident, hint } => {
                let fixes = db.search_fix(&ident);
                if fixes.is_empty() {
                    log.push(format!("[SimBriefUplinkAdapter](uplinkFlightPlanFromSimbrief) Found no fixes for \"airwayTermination\" chunk: {ident}. Cancelling airway entry..."));
                    fms.plan_mut(up)?.clear_pending_airways();
                    let is_leg = fms.plan(up).and_then(|p| p.element_at(insert_head as usize)).is_some_and(|e| !e.is_disco());
                    if is_leg {
                        fms.execute(db, Command::InsertDiscontinuityAfter { index: insert_head as usize, plan: up })?;
                        insert_head += 1;
                    }
                    continue;
                }
                let pick = |fixes: &[Fix]| if fixes.len() > 1 { pick_fix(fixes, &hint) } else { fixes[0].clone() };
                let pending = fms.plan(up).and_then(|p| p.pending_airways.clone());
                let Some(pending) = pending else {
                    fms.execute(db, Command::NextWaypoint { index: insert_head as usize, waypoint: pick(&fixes), plan: up })?;
                    let is_leg = fms.plan(up).and_then(|p| p.element_at(insert_head as usize)).is_some_and(|e| !e.is_disco());
                    if is_leg {
                        fms.execute(db, Command::InsertDiscontinuityAfter { index: insert_head as usize, plan: up })?;
                        insert_head += 1;
                    }
                    insert_head += 1;
                    continue;
                };
                let tail_airway = pending.elements.last().and_then(|e| e.airway.clone());
                let airway_fix = tail_airway.as_ref().and_then(|a| fixes.iter().find(|f| a.fixes.iter().any(|x| x.ident == f.ident && x.icao_code == f.icao_code)).cloned());
                if let Some(airway_fix) = airway_fix {
                    fms.plan_mut(up)?.continue_airway_entry_to_fix(&airway_fix, false)?;
                    if fms.plan(up).is_some_and(|p| p.pending_airways.is_some()) {
                        fms.plan_mut(up)?.finalise_airway_entry()?;
                        insert_head = fms.plan(up).map(|p| p.insert_head_at_end_of_enroute()).unwrap_or(-1);
                    }
                } else {
                    log.push(format!("[SimBriefUplinkAdapter](uplinkFlightPlanFromSimbrief) Airway termination {ident} not found on airway."));
                    fms.plan_mut(up)?.clear_pending_airways();
                    fms.execute(db, Command::NextWaypoint { index: insert_head as usize, waypoint: pick(&fixes), plan: up })?;
                    let is_leg = fms.plan(up).and_then(|p| p.element_at(insert_head as usize)).is_some_and(|e| !e.is_disco());
                    if is_leg {
                        fms.execute(db, Command::InsertDiscontinuityAfter { index: insert_head as usize, plan: up })?;
                        insert_head += 1;
                    }
                    insert_head += 1;
                }
            }
        }
    }
    let events = fms.sync_perf();
    fms.perf_events.extend(events);
    Ok(())
}

pub fn lat_long_ident(lat: f64, long: f64) -> String {
    let ns = if lat >= 0. { 'N' } else { 'S' };
    let ew = if long >= 0. { 'E' } else { 'W' };
    format!("{ns}{:02}{ew}{:03}", lat.abs().round() as i64, long.abs().round() as i64)
}

#[cfg(test)]
mod tests {
    use super::Ofp;

    #[test]
    fn navlog_reads_from_json_1_and_json_v2() {
        let fix = r#"{"ident":"KADOM","type":"wpt","via_airway":"DCT","is_sid_star":"0","pos_lat":"-33.7","pos_long":"150.3"}"#;
        let v1 = format!(r#"{{"navlog":{{"fix":[{fix}]}}}}"#);
        let v2 = format!(r#"{{"navlog":[{fix}]}}"#);
        for json in [v1, v2] {
            let ofp = Ofp::from_simbrief_json(&json).unwrap();
            assert_eq!(ofp.navlog.len(), 1, "{json}");
            assert_eq!(ofp.navlog[0].ident, "KADOM");
        }
    }
}

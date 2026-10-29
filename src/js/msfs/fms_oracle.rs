use std::collections::HashMap;

use serde_json::json;

use crate::js::{CallReply, Host, LogLevel};

pub struct Recording<H: Host> {
    pub inner: H,
    pub now_ms: f64,
    pub events: Vec<serde_json::Value>,
    last: HashMap<String, u64>,
}

impl<H: Host> Recording<H> {
    pub fn new(inner: H) -> Self {
        Self { inner, now_ms: 0., events: Vec::new(), last: HashMap::new() }
    }
}

impl<H: Host> Host for Recording<H> {
    fn get_var(&mut self, name: &str, unit: &str) -> f64 {
        let value = self.inner.get_var(name, unit);
        let knots_var = matches!(name.trim_start_matches("A:"), "GPS GROUND SPEED" | "GROUND VELOCITY" | "AIRSPEED TRUE");
        if knots_var && unit.eq_ignore_ascii_case("meters per second") {
            value * 1852. / 3600.
        } else {
            value
        }
    }

    fn set_var(&mut self, name: &str, unit: &str, value: f64) {
        if self.last.insert(name.to_string(), value.to_bits()) != Some(value.to_bits()) {
            self.events.push(json!({ "t": self.now_ms, "var": name, "value": value }));
        }
        self.inner.set_var(name, unit, value)
    }

    fn log(&mut self, level: LogLevel, message: &str) {
        self.inner.log(level, message)
    }

    fn get_string(&mut self, name: &str) -> String {
        self.inner.get_string(name)
    }

    fn set_string(&mut self, name: &str, value: &str) {
        self.inner.set_string(name, value)
    }

    fn call(&mut self, name: &str, args_json: &str) -> CallReply {
        self.inner.call(name, args_json)
    }

    fn trigger(&mut self, name: &str, args_json: &str) {
        self.inner.trigger(name, args_json)
    }

    fn send_event(&mut self, name: &str, values: &[f64]) {
        self.inner.send_event(name, values)
    }

    fn get_magvar(&mut self, lat: f64, lon: f64) -> f64 {
        self.inner.get_magvar(lat, lon)
    }

    fn generic_data(&mut self, key: &str, json: &str) {
        self.events.push(json!({ "t": self.now_ms, "key": key, "json": json }));
        self.inner.generic_data(key, json)
    }

    fn set_now(&mut self, ms: f64) {
        self.now_ms = ms;
        self.inner.set_now(ms)
    }
}

use std::path::PathBuf;

use super::fms_offline::{arinc_word, attach_navdata, canned_ofp_json, new_cockpit, power_up_on_ground, FmsHost, Mfd, VarStore, DUMMY_SIMBRIEF_USER, EGLL_LAT, EGLL_LON, SIMBRIEF_USER_KEY};
use super::Cockpit;

const SNAPSHOT_JS: &str = include_str!("fms_oracle_snapshot.js");
const STEP_MS: f64 = 50.;

type OracleHost = FmsHost<Recording<VarStore>>;

fn wait_ready(cockpit: &mut Cockpit, host: &mut OracleHost) -> Result<Mfd, String> {
    let mfd = Mfd::locate(cockpit)?;
    for _ in 0..120 {
        host.steps(cockpit, 20);
        if cockpit.all_loaded() && mfd.install_queries(cockpit).is_ok() && mfd.find(cockpit, "ACTIVE", true, ".mfd-page-selector-label").is_some() {
            return Ok(mfd);
        }
    }
    Err("the MFD never showed its page selectors".into())
}

fn fms_op(cockpit: &mut Cockpit, host: &mut OracleHost, mfd: &Mfd, body: &str) -> Result<(), String> {
    let start = format!(
        "(() => {{ globalThis.__xpOp = null; const fmc = (globalThis.__xpFmc || []).find(Boolean); const fps = fmc.flightPlanInterface; (async () => {{ {body} }})().then(() => {{ globalThis.__xpOp = 'ok'; }}, (e) => {{ globalThis.__xpOp = 'error: ' + ((e && e.stack) || e); }}); return 'started'; }})()"
    );
    cockpit.eval_in(&mfd.view, &start)?;
    for _ in 0..400 {
        host.step(cockpit);
        let state = cockpit.eval_in(&mfd.view, "String(globalThis.__xpOp)")?;
        if state == "ok" {
            return Ok(());
        }
        if state.starts_with("error") {
            return Err(format!("{body}: {state}"));
        }
    }
    Err(format!("{body}: still running after 20 s"))
}

fn snapshot(cockpit: &Cockpit, host: &OracleHost, mfd: &Mfd, label: &str) -> Result<serde_json::Value, String> {
    let raw = cockpit.eval_in(&mfd.view, SNAPSHOT_JS)?;
    let state: serde_json::Value = serde_json::from_str(&raw).map_err(|e| format!("snapshot {label}: {e}"))?;
    if let Some(err) = state.get("error") {
        return Err(format!("snapshot {label}: {err}"));
    }
    Ok(json!({ "label": label, "t": host.now_ms, "state": state, "mfd": mfd.positioned_texts(cockpit) }))
}

fn navigate(cockpit: &mut Cockpit, host: &mut OracleHost, mfd: &Mfd, selector: &str, item: &str) -> Result<(), String> {
    let uri = format!("fms/{}/{}", selector.to_lowercase().replace(' ', "-"), item.to_lowercase().replace(' ', "-"));
    fms_op(cockpit, host, mfd, &format!("fmc.mfdReference.uiService.navigateTo({});", json!(uri)))?;
    host.steps(cockpit, 10);
    Ok(())
}

fn load_route_js(cockpit: &mut Cockpit, host: &mut OracleHost, ofp_json: &str) -> Result<(), String> {
    host.ofp = ofp_json.to_string();
    cockpit.shared.store.op("set", SIMBRIEF_USER_KEY, DUMMY_SIMBRIEF_USER);
    power_up_on_ground(host, EGLL_LAT, EGLL_LON);
    let mfd = wait_ready(cockpit, host)?;
    navigate(cockpit, host, &mfd, "ACTIVE", "INIT")?;
    let request = "CPNY F-PLN REQUEST";
    if mfd.wait_for(cockpit, host, request, false, "", 800).is_none() {
        return Err(format!("the INIT page never offered {request:?}; it shows {:?}", mfd.texts(cockpit, "")));
    }
    mfd.click_text(cockpit, host, request, false, "")?;
    let received = "RECEIVED CPNY F-PLN";
    if mfd.wait_for(cockpit, host, received, false, "", 1200).is_none() {
        return Err(format!("the uplink never arrived; fetches {:?}; page shows {:?}", host.fetches, mfd.texts(cockpit, "")));
    }
    mfd.click_text(cockpit, host, received, false, "")?;
    host.steps(cockpit, 6);
    if mfd.click_text(cockpit, host, "INSERT*", true, "").is_err() {
        mfd.click_text(cockpit, host, "INSERT", false, "")?;
    }
    host.steps(cockpit, 40);
    Ok(())
}

fn destination(lat: f64, lon: f64, track: f64, nm: f64) -> (f64, f64) {
    let d = nm / 3440.065;
    let (la, lo, b) = (lat.to_radians(), lon.to_radians(), track.to_radians());
    let la2 = (la.sin() * d.cos() + la.cos() * d.sin() * b.cos()).asin();
    let lo2 = lo + (b.sin() * d.sin() * la.cos()).atan2(d.cos() - la.sin() * la2.sin());
    (la2.to_degrees(), ((lo2.to_degrees() + 540.) % 360.) - 180.)
}

fn bearing(lat: f64, lon: f64, lat2: f64, lon2: f64) -> f64 {
    let (a, b, dl) = (lat.to_radians(), lat2.to_radians(), (lon2 - lon).to_radians());
    let y = dl.sin() * b.cos();
    let x = a.cos() * b.sin() - a.sin() * b.cos() * dl.cos();
    (y.atan2(x).to_degrees() + 360.) % 360.
}

fn set_both(host: &mut OracleHost, name: &str, unit: &str, value: f64) {
    host.inner.inner.set_var(name, unit, value);
    host.inner.inner.set_var(&format!("A:{name}"), unit, value);
}

fn fg(host: &OracleHost, name: &str) -> f64 {
    host.inner.inner.vars.get(&format!("L:A32NX_FG_{name}")).copied().unwrap_or(0.)
}

fn active_leg(cockpit: &Cockpit, mfd: &Mfd) -> Result<i64, String> {
    let raw = cockpit.eval_in(&mfd.view, "String(((globalThis.__xpFmc || []).find(Boolean)).flightPlanInterface.active.activeLegIndex)")?;
    raw.parse::<f64>().map(|v| v as i64).map_err(|e| format!("activeLegIndex {raw:?}: {e}"))
}

fn predictions(cockpit: &Cockpit, mfd: &Mfd) -> serde_json::Value {
    let raw = cockpit.eval_in(
        &mfd.view,
        "(() => { const g = ((globalThis.__xpFmc || []).find(Boolean)).guidanceController.activeGeometry; if (!g) return '[]'; return JSON.stringify([...g.legs.entries()].map(([i, l]) => [i, l.predictedTas === undefined ? null : l.predictedTas, l.predictedGs === undefined ? null : l.predictedGs])); })()",
    );
    raw.ok().and_then(|r| serde_json::from_str(&r).ok()).unwrap_or(json!([]))
}

fn watch_recomputes(cockpit: &Cockpit, mfd: &Mfd) {
    let _ = cockpit.eval_in(
        &mfd.view,
        "(() => { const fmc = (globalThis.__xpFmc || []).find(Boolean); const gc = fmc.guidanceController; if (gc.__oracleRecomp) return ''; gc.__oracleRecomp = []; const orig = gc.recomputeGeometry.bind(gc); gc.recomputeGeometry = (geometry, plan) => { if (plan === fmc.flightPlanInterface.active) { const p = gc.lnavDriver.ppos; gc.__oracleRecomp.push({ tas: SimVar.GetSimVarValue('AIRSPEED TRUE', 'Knots'), gs: SimVar.GetSimVarValue('GPS GROUND SPEED', 'Knots'), track: SimVar.GetSimVarValue('GPS GROUND TRUE TRACK', 'degree'), lat: p ? p.lat : null, lon: p ? p.long : null, leg: plan.activeLegIndex, timer: gc.geometryRecomputationTimer }); } return orig(geometry, plan); }; return ''; })()",
    );
}

fn drain_recomputes(cockpit: &Cockpit, mfd: &Mfd) -> serde_json::Value {
    let raw = cockpit.eval_in(&mfd.view, "(() => { const gc = ((globalThis.__xpFmc || []).find(Boolean)).guidanceController; return JSON.stringify(gc.__oracleRecomp ? gc.__oracleRecomp.splice(0) : []); })()");
    raw.ok().and_then(|r| serde_json::from_str(&r).ok()).unwrap_or(json!([]))
}

fn plan_start(cockpit: &Cockpit, mfd: &Mfd) -> Result<(f64, f64, f64), String> {
    let raw = cockpit.eval_in(
        &mfd.view,
        "(() => { const p = ((globalThis.__xpFmc || []).find(Boolean)).flightPlanInterface.active; const at = (i) => { const e = p.allLegs[i]; const l = e && !e.isDiscontinuity && e.definition && e.definition.waypoint && e.definition.waypoint.location; return l ? [l.lat, l.long] : null; }; const a = p.activeLegIndex; return JSON.stringify([at(a - 1), at(a), at(a + 1)]); })()",
    )?;
    let pts: Vec<Option<Vec<f64>>> = serde_json::from_str(&raw).map_err(|e| format!("plan start {raw:?}: {e}"))?;
    let at = |i: usize| pts.get(i).cloned().flatten();
    match (at(0), at(1), at(2)) {
        (Some(a), Some(b), _) => Ok((a[0], a[1], bearing(a[0], a[1], b[0], b[1]))),
        (None, Some(a), Some(b)) => Ok((a[0], a[1], bearing(a[0], a[1], b[0], b[1]))),
        _ => Err(format!("plan start: legs around the active leg have no fix: {raw}")),
    }
}

fn fly(cockpit: &mut Cockpit, host: &mut OracleHost, mfd: &Mfd, spec: &serde_json::Value, trace: &mut Vec<serde_json::Value>) -> Result<(), String> {
    const DT: f64 = STEP_MS / 1000.;
    let gs = spec["kt"].as_f64().unwrap_or(250.);
    let max_s = spec["seconds"].as_f64().unwrap_or(600.);
    let until_leg = spec.get("until_leg").and_then(|v| v.as_i64());
    let sample_every = ((spec["sample_ms"].as_f64().unwrap_or(500.) / STEP_MS).round() as usize).max(1);
    let (mut lat, mut lon, mut track) = match spec.get("start") {
        Some(s) if s.as_str() == Some("plan") => plan_start(cockpit, mfd)?,
        Some(s) => (s["lat"].as_f64().ok_or("fly start lat")?, s["lon"].as_f64().ok_or("fly start lon")?, s["track"].as_f64().ok_or("fly start track")?),
        None => {
            let v = &host.inner.inner.vars;
            (v.get("PLANE LATITUDE").copied().unwrap_or(0.), v.get("PLANE LONGITUDE").copied().unwrap_or(0.), v.get("GPS GROUND TRUE TRACK").copied().unwrap_or(0.))
        }
    };
    let phase = spec["flight_phase"].as_f64().unwrap_or(2.);
    let mut roll = 0.;
    let max_roll_rate = 5.;
    let steps = (max_s / DT).ceil() as usize;
    watch_recomputes(cockpit, mfd);
    for n in 0..steps {
        let tas = gs;
        let (at_lat, at_lon, at_track) = (lat, lon, track);
        host.inner.inner.set_var("L:A32NX_FMGC_FLIGHT_PHASE", "number", phase);
        set_both(host, "PLANE LATITUDE", "degrees", lat);
        set_both(host, "PLANE LONGITUDE", "degrees", lon);
        set_both(host, "GPS POSITION LAT", "degrees", lat);
        set_both(host, "GPS POSITION LON", "degrees", lon);
        set_both(host, "GROUND VELOCITY", "knots", gs);
        set_both(host, "GPS GROUND SPEED", "knots", gs);
        set_both(host, "AIRSPEED TRUE", "knots", tas);
        set_both(host, "GPS GROUND TRUE TRACK", "degrees", track);
        set_both(host, "PLANE HEADING DEGREES TRUE", "degrees", track);
        set_both(host, "PLANE HEADING DEGREES MAGNETIC", "degrees", track);
        for ir in 1..=3 {
            let pre = format!("L:A32NX_ADIRS_IR_{ir}");
            for (suffix, value) in [("LATITUDE", lat), ("LONGITUDE", lon), ("HEADING", track), ("TRUE_HEADING", track), ("TRUE_TRACK", track), ("TRACK", track), ("GROUND_SPEED", gs)] {
                host.inner.inner.set_var(&format!("{pre}_{suffix}"), "number", arinc_word(value as f32, 3));
            }
        }
        host.step(cockpit);

        let avail = host.inner.inner.vars.get("L:A32NX_FM_LATERAL_FLIGHTPLAN_AVAIL").copied().unwrap_or(0.) != 0.;
        let (xte, tae, phi_cmd, phi_limit) = (fg(host, "CROSS_TRACK_ERROR"), fg(host, "TRACK_ANGLE_ERROR"), fg(host, "PHI_COMMAND"), fg(host, "PHI_LIMIT"));
        let limit = if phi_limit > 0. { phi_limit } else { 25. };
        let target = if avail { (phi_cmd + k2() * (k1() * xte + gs * tae.clamp(-45., 45.))).clamp(-limit, limit) } else { 0. };
        roll += (target - roll).clamp(-max_roll_rate * DT, max_roll_rate * DT);
        track = (track + (9.81 * roll.to_radians().tan() / (tas * 0.514444)).to_degrees() * DT + 360.) % 360.;
        let (la, lo) = destination(lat, lon, track, gs * DT / 3600.);
        lat = la;
        lon = lo;

        if n % 1200 == 0 {
            eprintln!("fly: t={:.0}s lat={lat:.4} lon={lon:.4} track={track:.1}", n as f64 * DT);
        }
        if n % sample_every == 0 {
            let leg = active_leg(cockpit, mfd)?;
            trace.push(json!({ "t": host.now_ms, "lat": at_lat, "lon": at_lon, "track": at_track, "gs": gs, "tas": tas, "roll": roll, "leg": leg, "xte": xte, "tae": tae, "phi_command": phi_cmd, "phi_limit": phi_limit, "avail": avail, "pred": predictions(cockpit, mfd), "recomp": drain_recomputes(cockpit, mfd) }));
            if until_leg.is_some_and(|u| leg >= u) {
                break;
            }
        }
    }
    Ok(())
}

fn k1() -> f64 {
    180. / 4. / (std::f64::consts::PI * std::f64::consts::PI) / 0.8 / (3. / 3600.)
}

fn k2() -> f64 {
    0.8 / std::f64::consts::PI / (9.81 * 6997.84) / (3. / 3600.)
}

pub fn run_scenario(scenario: &serde_json::Value) -> Result<serde_json::Value, String> {
    let pkg = PathBuf::from(std::env::var("FMS_ORACLE_PKG").unwrap_or_else(|_| r"E:\fbw-build\fms-oracle-pkg".into()));
    let xplane = PathBuf::from(std::env::var("FMS_XPLANE_ROOT").unwrap_or_else(|_| r"D:\Steam Games\steamapps\common\X-Plane 12".into()));
    let mut cockpit = new_cockpit(&pkg)?;
    let mut host = attach_navdata(Recording::new(VarStore::new()), &xplane)?;
    let lat = scenario["lat"].as_f64().ok_or("scenario lat")?;
    let lon = scenario["lon"].as_f64().ok_or("scenario lon")?;
    let steps = scenario["steps"].as_array().ok_or("scenario steps")?;
    let uplinks = steps.first().is_some_and(|s| s.get("uplink").is_some());
    let mut mfd = None;
    if !uplinks {
        power_up_on_ground(&mut host, lat, lon);
        mfd = Some(wait_ready(&mut cockpit, &mut host)?);
    }
    let mut snapshots = Vec::new();
    let mut trace = Vec::new();
    for step in steps {
        if step.get("uplink").is_some() {
            load_route_js(&mut cockpit, &mut host, &canned_ofp_json())?;
            mfd = Some(Mfd::locate(&cockpit)?);
            continue;
        }
        let m = mfd.as_ref().ok_or("no MFD yet")?;
        if let Some(ms) = step.get("run_ms").and_then(|v| v.as_f64()) {
            host.steps(&mut cockpit, (ms / STEP_MS).ceil() as usize);
        } else if let Some(body) = step.get("fms").and_then(|v| v.as_str()) {
            fms_op(&mut cockpit, &mut host, m, body)?;
        } else if let Some(page) = step.get("page").and_then(|v| v.as_array()) {
            let s = |i: usize| page.get(i).and_then(|v| v.as_str()).unwrap_or("");
            navigate(&mut cockpit, &mut host, m, s(0), s(1))?;
        } else if let Some(spec) = step.get("fly") {
            fly(&mut cockpit, &mut host, m, spec, &mut trace)?;
        } else if let Some(var) = step.get("var").and_then(|v| v.as_array()) {
            let name = var.first().and_then(|v| v.as_str()).ok_or("var name")?;
            let unit = var.get(1).and_then(|v| v.as_str()).unwrap_or("number");
            let value = var.get(2).and_then(|v| v.as_f64()).ok_or("var value")?;
            host.inner.inner.set_var(name, unit, value);
        } else if let Some(label) = step.get("snapshot").and_then(|v| v.as_str()) {
            snapshots.push(snapshot(&cockpit, &host, m, label)?);
        } else {
            return Err(format!("unknown step {step}"));
        }
    }
    let mut golden = json!({
        "scenario": scenario["name"],
        "navdata": host.nav.date_range(),
        "snapshots": snapshots,
        "events": std::mem::take(&mut host.inner.events),
    });
    if !trace.is_empty() {
        golden["trace"] = serde_json::Value::Array(trace);
    }
    Ok(golden)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::js::msfs::fms_offline::VarStore;

    #[test]
    fn recording_keeps_changes_and_generic_data_in_order() {
        let mut r = Recording::new(VarStore::new());
        r.set_now(50.);
        r.set_var("L:A", "number", 1.);
        r.set_var("L:A", "number", 1.);
        r.generic_data("K", "[1]");
        r.set_now(100.);
        r.set_var("L:A", "number", 2.);
        assert_eq!(r.events.len(), 3);
        assert_eq!(r.events[0]["t"], 50.);
        assert_eq!(r.events[1]["key"], "K");
        assert_eq!(r.events[2]["value"], 2.);
        assert_eq!(r.inner.vars.get("L:A"), Some(&2.));
    }

    #[test]
    #[ignore]
    fn record_fms_oracle() {
        let dir = std::path::PathBuf::from(std::env::var("FMS_ORACLE_SCENARIOS").unwrap_or_else(|_| "tools/fms_oracle/scenarios".into()));
        let out = std::path::PathBuf::from(std::env::var("FMS_ORACLE_OUT").unwrap_or_else(|_| r"E:\fbw-build\fms-oracle".into()));
        let only = std::env::var("FMS_ORACLE_ONLY").ok();
        std::fs::create_dir_all(&out).unwrap();
        let mut names: Vec<_> = std::fs::read_dir(&dir).unwrap().filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "json")).collect();
        names.sort();
        for path in names {
            let scenario: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
            let name = scenario["name"].as_str().unwrap().to_string();
            if only.as_deref().is_some_and(|o| o != name) {
                continue;
            }
            let golden = run_scenario(&scenario).unwrap_or_else(|e| panic!("{name}: {e}"));
            std::fs::write(out.join(format!("{name}.json")), serde_json::to_string_pretty(&golden).unwrap()).unwrap();
            println!("{name}: {} snapshots, {} events", golden["snapshots"].as_array().unwrap().len(), golden["events"].as_array().unwrap().len());
            assert!(golden["events"].as_array().unwrap().iter().any(|e| e["json"].as_str().is_some_and(|j| j.contains("A32NX_EFIS_VECTORS_"))), "{name}: no EFIS vector events recorded");
        }
    }
}

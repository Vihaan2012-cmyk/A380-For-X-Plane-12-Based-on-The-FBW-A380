use std::collections::HashMap;

use crate::computer::{Inputs, RawTraffic, Tcas, Update};
use crate::util::great_circle_distance;

pub const FT_PER_M: f64 = 3.281;
pub const NETWORK_UID_BASE: f64 = 16_777_216.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Source {
    XPlane,
    Vatsim,
    Ivao,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Quality {
    Live,
    Fresh,
    Extrapolated,
    Stale,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SimTarget {
    pub uid: u32,
    pub callsign: String,
    pub lat: f64,
    pub lon: f64,
    pub alt_m: f64,
    pub heading_deg: f64,
    pub on_ground: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NetworkReport {
    pub source: Source,
    pub callsign: String,
    pub lat: f64,
    pub lon: f64,
    pub alt_ft: f64,
    pub gs_kt: f64,
    pub track_deg: f64,
    pub transponder: Option<String>,
    pub aircraft_type: String,
    pub on_ground: Option<bool>,
    pub observed_s: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct OwnShip {
    pub lat: f64,
    pub lon: f64,
    pub alt_ft: f64,
    pub track_deg: f64,
    pub gs_kt: f64,
    pub callsign: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MergeConfig {
    pub ra_max_age_s: f64,
    pub extrapolate_max_s: f64,
    pub drop_age_s: f64,
    pub blend_s: f64,
    pub dedupe_callsign_nm: f64,
    pub dedupe_callsign_ft: f64,
    pub dedupe_position_nm: f64,
    pub dedupe_position_ft: f64,
    pub own_echo_nm: f64,
    pub own_echo_ft: f64,
    pub own_echo_track_deg: f64,
    pub own_echo_gs_kt: f64,
    pub max_vs_fpm: f64,
    pub ground_gs_kt: f64,
}

impl Default for MergeConfig {
    fn default() -> Self {
        Self {
            ra_max_age_s: 20.0,
            extrapolate_max_s: 60.0,
            drop_age_s: 120.0,
            blend_s: 4.0,
            dedupe_callsign_nm: 5.0,
            dedupe_callsign_ft: 1500.0,
            dedupe_position_nm: 0.5,
            dedupe_position_ft: 300.0,
            own_echo_nm: 1.5,
            own_echo_ft: 300.0,
            own_echo_track_deg: 30.0,
            own_echo_gs_kt: 60.0,
            max_vs_fpm: 6000.0,
            ground_gs_kt: 40.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Intruder {
    pub uid: f64,
    pub source: Source,
    pub callsign: String,
    pub aircraft_type: String,
    pub transponder: Option<String>,
    pub lat: f64,
    pub lon: f64,
    pub alt_ft: f64,
    pub track_deg: f64,
    pub gs_kt: f64,
    pub vs_fpm: f64,
    pub on_ground: bool,
    pub quality: Quality,
    pub age_s: f64,
    pub ra_allowed: bool,
    pub obs_lat: f64,
    pub obs_lon: f64,
    pub obs_alt_ft: f64,
    pub obs_s: f64,
}

impl Intruder {
    pub fn raw(&self) -> RawTraffic {
        RawTraffic { uid: self.uid, lat: self.lat, lon: self.lon, alt: self.alt_ft / FT_PER_M, heading: self.track_deg }
    }
}

#[derive(Clone, Debug)]
struct NetTrack {
    report: NetworkReport,
    uid: f64,
    vs_fpm: f64,
    offset: (f64, f64, f64),
    offset_at_s: f64,
}

#[derive(Clone, Debug, Default)]
pub struct TrafficMerger {
    pub config: MergeConfig,
    tracks: HashMap<(Source, String), NetTrack>,
    next_uid: u64,
}

pub fn normalise_callsign(callsign: &str) -> String {
    callsign.chars().filter(|c| c.is_ascii_alphanumeric()).map(|c| c.to_ascii_uppercase()).collect()
}

pub fn dead_reckon(lat: f64, lon: f64, track_deg: f64, gs_kt: f64, dt_s: f64) -> (f64, f64) {
    let d_nm = gs_kt.max(0.0) * dt_s.max(0.0) / 3600.0;
    let t = track_deg.to_radians();
    let coslat = lat.to_radians().cos().max(1e-6);
    let mut lon = lon + d_nm * t.sin() / (60.0 * coslat);
    if lon > 180.0 {
        lon -= 360.0;
    } else if lon < -180.0 {
        lon += 360.0;
    }
    ((lat + d_nm * t.cos() / 60.0).clamp(-90.0, 90.0), lon)
}

fn angle_diff(a: f64, b: f64) -> f64 {
    let d = (a - b).rem_euclid(360.0);
    d.min(360.0 - d)
}

impl TrafficMerger {
    pub fn new(config: MergeConfig) -> Self {
        Self { config, tracks: HashMap::new(), next_uid: 0 }
    }

    pub fn network_len(&self) -> usize {
        self.tracks.len()
    }

    fn on_ground(&self, r: &NetworkReport) -> bool {
        r.on_ground.unwrap_or(r.gs_kt < self.config.ground_gs_kt)
    }

    fn predict(&self, tr: &NetTrack, now_s: f64) -> (f64, f64, f64) {
        let r = &tr.report;
        let h = (now_s - r.observed_s).clamp(0.0, self.config.extrapolate_max_s);
        let (lat, lon, alt) = if self.on_ground(r) {
            (r.lat, r.lon, r.alt_ft)
        } else {
            let (lat, lon) = dead_reckon(r.lat, r.lon, r.track_deg, r.gs_kt, h);
            (lat, lon, (r.alt_ft + tr.vs_fpm * h / 60.0).max(0.0))
        };
        let f = if self.config.blend_s > 0.0 { (1.0 - (now_s - tr.offset_at_s) / self.config.blend_s).clamp(0.0, 1.0) } else { 0.0 };
        (lat + tr.offset.0 * f, lon + tr.offset.1 * f, alt + tr.offset.2 * f)
    }

    pub fn ingest(&mut self, source: Source, reports: Vec<NetworkReport>, now_s: f64) {
        let mut seen: Vec<String> = Vec::with_capacity(reports.len());
        for mut r in reports {
            r.source = source;
            let key_cs = normalise_callsign(&r.callsign);
            if key_cs.is_empty() || !r.lat.is_finite() || !r.lon.is_finite() || !r.alt_ft.is_finite() {
                continue;
            }
            seen.push(key_cs.clone());
            let key = (source, key_cs);
            match self.tracks.get(&key) {
                Some(old) if r.observed_s <= old.report.observed_s => {}
                Some(old) => {
                    let before = self.predict(old, now_s);
                    let dt = r.observed_s - old.report.observed_s;
                    let vs = if dt >= 1.0 {
                        ((r.alt_ft - old.report.alt_ft) / dt * 60.0).clamp(-self.config.max_vs_fpm, self.config.max_vs_fpm)
                    } else {
                        old.vs_fpm
                    };
                    let mut next = NetTrack { report: r, uid: old.uid, vs_fpm: vs, offset: (0.0, 0.0, 0.0), offset_at_s: now_s };
                    let after = self.predict(&next, now_s);
                    let mut dlon = before.1 - after.1;
                    if dlon > 180.0 {
                        dlon -= 360.0;
                    } else if dlon < -180.0 {
                        dlon += 360.0;
                    }
                    next.offset = (before.0 - after.0, dlon, before.2 - after.2);
                    self.tracks.insert(key, next);
                }
                None => {
                    self.next_uid += 1;
                    let uid = NETWORK_UID_BASE + self.next_uid as f64;
                    self.tracks.insert(key, NetTrack { report: r, uid, vs_fpm: 0.0, offset: (0.0, 0.0, 0.0), offset_at_s: now_s });
                }
            }
        }
        self.tracks.retain(|(s, cs), _| *s != source || seen.contains(cs));
    }

    pub fn clear(&mut self, source: Source) {
        self.tracks.retain(|(s, _), _| *s != source);
    }

    fn is_own_echo(&self, own: &OwnShip, cs: &str, lat: f64, lon: f64, alt_ft: f64, r: &NetworkReport) -> bool {
        if let Some(own_cs) = own.callsign.as_deref() {
            let own_cs = normalise_callsign(own_cs);
            if !own_cs.is_empty() && own_cs == cs {
                return true;
            }
        }
        great_circle_distance(own.lat, own.lon, lat, lon) < self.config.own_echo_nm
            && (own.alt_ft - alt_ft).abs() < self.config.own_echo_ft
            && angle_diff(own.track_deg, r.track_deg) < self.config.own_echo_track_deg
            && (own.gs_kt - r.gs_kt).abs() < self.config.own_echo_gs_kt
    }

    fn sim_match(&self, sim: &[SimTarget], cs: &str, lat: f64, lon: f64, alt_ft: f64, r: &NetworkReport) -> Option<usize> {
        let c = &self.config;
        let near = |s: &SimTarget, nm: f64, ft: f64| {
            let s_alt = s.alt_m * FT_PER_M;
            let d = great_circle_distance(s.lat, s.lon, lat, lon).min(great_circle_distance(s.lat, s.lon, r.lat, r.lon));
            let dz = (s_alt - alt_ft).abs().min((s_alt - r.alt_ft).abs());
            d < nm && dz < ft
        };
        sim.iter()
            .position(|s| normalise_callsign(&s.callsign) == cs && near(s, c.dedupe_callsign_nm, c.dedupe_callsign_ft))
            .or_else(|| sim.iter().position(|s| near(s, c.dedupe_position_nm, c.dedupe_position_ft)))
    }

    pub fn merge(&mut self, sim: &[SimTarget], own: Option<&OwnShip>, now_s: f64) -> Vec<Intruder> {
        let drop = self.config.drop_age_s;
        self.tracks.retain(|_, t| now_s - t.report.observed_s < drop);

        let mut out: Vec<Intruder> = sim
            .iter()
            .filter(|s| s.lat.is_finite() && s.lon.is_finite() && s.alt_m.is_finite())
            .map(|s| Intruder {
                uid: s.uid as f64,
                source: Source::XPlane,
                callsign: s.callsign.trim().to_string(),
                aircraft_type: String::new(),
                transponder: None,
                lat: s.lat,
                lon: s.lon,
                alt_ft: s.alt_m * FT_PER_M,
                track_deg: s.heading_deg,
                gs_kt: 0.0,
                vs_fpm: 0.0,
                on_ground: s.on_ground,
                quality: Quality::Live,
                age_s: 0.0,
                ra_allowed: true,
                obs_lat: s.lat,
                obs_lon: s.lon,
                obs_alt_ft: s.alt_m * FT_PER_M,
                obs_s: now_s,
            })
            .collect();
        let sim_live: Vec<SimTarget> = sim.iter().filter(|s| s.lat.is_finite() && s.lon.is_finite() && s.alt_m.is_finite()).cloned().collect();

        let mut keys: Vec<&(Source, String)> = self.tracks.keys().collect();
        keys.sort();
        for key in keys {
            let tr = &self.tracks[key];
            let r = &tr.report;
            let (lat, lon, alt_ft) = self.predict(tr, now_s);
            if let Some(own) = own {
                if self.is_own_echo(own, &key.1, lat, lon, alt_ft, r) {
                    continue;
                }
            }
            if let Some(i) = self.sim_match(&sim_live, &key.1, lat, lon, alt_ft, r) {
                let s = &mut out[i];
                if s.callsign.is_empty() {
                    s.callsign = r.callsign.trim().to_string();
                }
                if s.aircraft_type.is_empty() {
                    s.aircraft_type = r.aircraft_type.clone();
                }
                if s.transponder.is_none() {
                    s.transponder = r.transponder.clone();
                }
                if s.gs_kt == 0.0 {
                    s.gs_kt = r.gs_kt;
                }
                continue;
            }
            let age = (now_s - r.observed_s).max(0.0);
            let quality = if age <= self.config.ra_max_age_s {
                Quality::Fresh
            } else if age <= self.config.extrapolate_max_s {
                Quality::Extrapolated
            } else {
                Quality::Stale
            };
            let on_ground = self.on_ground(r);
            out.push(Intruder {
                uid: tr.uid,
                source: key.0,
                callsign: r.callsign.trim().to_string(),
                aircraft_type: r.aircraft_type.clone(),
                transponder: r.transponder.clone(),
                lat,
                lon,
                alt_ft,
                track_deg: r.track_deg,
                gs_kt: r.gs_kt,
                vs_fpm: if on_ground { 0.0 } else { tr.vs_fpm },
                on_ground,
                quality,
                age_s: age,
                ra_allowed: quality == Quality::Fresh && !on_ground,
                obs_lat: r.lat,
                obs_lon: r.lon,
                obs_alt_ft: r.alt_ft,
                obs_s: r.observed_s,
            });
        }
        out.sort_by(|a, b| a.source.cmp(&b.source).then(a.uid.partial_cmp(&b.uid).unwrap_or(std::cmp::Ordering::Equal)));
        out
    }
}

pub fn raw_traffic(intruders: &[Intruder]) -> Vec<RawTraffic> {
    intruders.iter().map(Intruder::raw).collect()
}

pub fn ta_only_uids(intruders: &[Intruder]) -> Vec<f64> {
    intruders.iter().filter(|i| !i.ra_allowed).map(|i| i.uid).collect()
}

pub fn update_tcas(tcas: &mut Tcas, dt_ms: f64, inputs: &Inputs, intruders: &[Intruder]) -> Update {
    tcas.set_ta_only(&ta_only_uids(intruders));
    tcas.update(dt_ms, inputs, &raw_traffic(intruders))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rep(cs: &str, lat: f64, lon: f64, alt_ft: f64, gs: f64, trk: f64, t: f64) -> NetworkReport {
        NetworkReport {
            source: Source::Vatsim,
            callsign: cs.into(),
            lat,
            lon,
            alt_ft,
            gs_kt: gs,
            track_deg: trk,
            transponder: Some("2000".into()),
            aircraft_type: "B738".into(),
            on_ground: None,
            observed_s: t,
        }
    }

    fn sim(uid: u32, cs: &str, lat: f64, lon: f64, alt_ft: f64) -> SimTarget {
        SimTarget { uid, callsign: cs.into(), lat, lon, alt_m: alt_ft / FT_PER_M, heading_deg: 90.0, on_ground: false }
    }

    #[test]
    fn network_positions_are_dead_reckoned_between_feeds() {
        let mut m = TrafficMerger::default();
        m.ingest(Source::Vatsim, vec![rep("TST1", 50.0, 8.0, 10000.0, 360.0, 0.0, 100.0)], 100.0);
        let a = m.merge(&[], None, 110.0);
        assert_eq!(a.len(), 1);
        assert!((a[0].lat - (50.0 + 1.0 / 60.0)).abs() < 1e-9);
        assert_eq!(a[0].quality, Quality::Fresh);
        assert!(a[0].ra_allowed);
        assert!(a[0].uid > NETWORK_UID_BASE);
        assert_eq!((a[0].obs_lat, a[0].obs_s), (50.0, 100.0));
    }

    #[test]
    fn a_new_report_blends_in_without_a_jump_and_gives_vertical_speed() {
        let mut m = TrafficMerger::default();
        m.ingest(Source::Vatsim, vec![rep("TST1", 50.0, 8.0, 10000.0, 360.0, 0.0, 100.0)], 100.0);
        let before = m.merge(&[], None, 115.0)[0].clone();
        m.ingest(Source::Vatsim, vec![rep("TST1", 50.02, 8.0, 10500.0, 360.0, 0.0, 115.0)], 115.0);
        let at = m.merge(&[], None, 115.0)[0].clone();
        assert!((at.lat - before.lat).abs() < 1e-9 && (at.alt_ft - before.alt_ft).abs() < 1e-6);
        assert_eq!(at.uid, before.uid);
        assert!((at.vs_fpm - 2000.0).abs() < 1e-6);
        let settled = m.merge(&[], None, 115.0 + m.config.blend_s)[0].clone();
        let (lat, _) = dead_reckon(50.02, 8.0, 0.0, 360.0, m.config.blend_s);
        assert!((settled.lat - lat).abs() < 1e-9);
    }

    #[test]
    fn old_data_loses_ra_then_freezes_then_drops() {
        let mut m = TrafficMerger::default();
        m.ingest(Source::Ivao, vec![rep("TST1", 50.0, 8.0, 10000.0, 360.0, 0.0, 0.0)], 0.0);
        let c = m.config.clone();
        let q = |m: &mut TrafficMerger, t: f64| m.merge(&[], None, t).first().map(|i| (i.quality, i.ra_allowed, i.lat));
        assert_eq!(q(&mut m, c.ra_max_age_s).unwrap().0, Quality::Fresh);
        let ext = q(&mut m, c.ra_max_age_s + 1.0).unwrap();
        assert_eq!((ext.0, ext.1), (Quality::Extrapolated, false));
        let s1 = q(&mut m, c.extrapolate_max_s + 1.0).unwrap();
        let s2 = q(&mut m, c.extrapolate_max_s + 30.0).unwrap();
        assert_eq!((s1.0, s1.1), (Quality::Stale, false));
        assert_eq!(s1.2, s2.2);
        assert!(q(&mut m, c.drop_age_s).is_none());
        assert_eq!(m.network_len(), 0);
    }

    #[test]
    fn pilots_missing_from_a_snapshot_are_removed_per_source() {
        let mut m = TrafficMerger::default();
        m.ingest(Source::Vatsim, vec![rep("A1", 50.0, 8.0, 5000.0, 200.0, 0.0, 0.0), rep("A2", 51.0, 8.0, 5000.0, 200.0, 0.0, 0.0)], 0.0);
        m.ingest(Source::Ivao, vec![rep("B1", 52.0, 8.0, 5000.0, 200.0, 0.0, 0.0)], 0.0);
        m.ingest(Source::Vatsim, vec![rep("A2", 51.0, 8.0, 5000.0, 200.0, 0.0, 15.0)], 15.0);
        let list = m.merge(&[], None, 15.0);
        let cs: Vec<(&str, Source)> = list.iter().map(|i| (i.callsign.as_str(), i.source)).collect();
        assert_eq!(cs, vec![("A2", Source::Vatsim), ("B1", Source::Ivao)]);
    }

    #[test]
    fn network_pilot_injected_as_ai_is_shown_once() {
        let mut m = TrafficMerger::default();
        m.ingest(
            Source::Vatsim,
            vec![rep("DLH-12", 50.0, 8.0, 9000.0, 250.0, 90.0, 0.0), rep("NONAME", 50.5, 8.5, 7000.0, 250.0, 90.0, 0.0), rep("TST9", 49.0, 7.0, 7000.0, 250.0, 90.0, 0.0)],
            0.0,
        );
        let (lat, lon) = dead_reckon(50.5, 8.5, 90.0, 250.0, 5.0);
        let targets = [sim(0xABC, "DLH12", 50.01, 8.03, 9100.0), sim(0xDEF, "", lat, lon + 0.001, 7050.0), sim(0x123, "DLH12X", 49.5, 7.5, 3000.0)];
        let list = m.merge(&targets, None, 5.0);
        assert_eq!(list.iter().filter(|i| i.source == Source::XPlane).count(), 3);
        let net: Vec<&str> = list.iter().filter(|i| i.source != Source::XPlane).map(|i| i.callsign.as_str()).collect();
        assert_eq!(net, vec!["TST9"]);
        let unnamed = list.iter().find(|i| i.uid == 0xDEF as f64).unwrap();
        assert_eq!((unnamed.callsign.as_str(), unnamed.aircraft_type.as_str(), unnamed.quality), ("NONAME", "B738", Quality::Live));
        assert!(unnamed.ra_allowed);
    }

    #[test]
    fn same_callsign_far_away_is_not_merged() {
        let mut m = TrafficMerger::default();
        m.ingest(Source::Vatsim, vec![rep("TST5", 50.0, 8.0, 9000.0, 250.0, 90.0, 0.0)], 0.0);
        let list = m.merge(&[sim(1, "TST5", 51.0, 8.0, 9000.0)], None, 0.0);
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn own_aircraft_echo_is_dropped() {
        let mut m = TrafficMerger::default();
        m.ingest(Source::Vatsim, vec![rep("OWN1", 50.0, 8.0, 12000.0, 300.0, 0.0, 0.0), rep("LAG2", 49.9, 8.0, 12000.0, 300.0, 0.0, 0.0)], 0.0);
        let (lat, lon) = dead_reckon(49.9, 8.0, 0.0, 300.0, 12.0);
        let own = OwnShip { lat, lon, alt_ft: 12050.0, track_deg: 2.0, gs_kt: 305.0, callsign: Some("own1".into()) };
        assert!(m.merge(&[], Some(&own), 12.0).is_empty());
        let other = OwnShip { track_deg: 180.0, ..own };
        let list = m.merge(&[], Some(&other), 12.0);
        assert_eq!(list.iter().map(|i| i.callsign.as_str()).collect::<Vec<_>>(), vec!["LAG2"]);
    }

    #[test]
    fn ground_traffic_holds_still_and_never_gets_ra() {
        let mut m = TrafficMerger::default();
        let mut r = rep("GND1", 50.0, 8.0, 360.0, 15.0, 90.0, 0.0);
        r.on_ground = None;
        let mut i = rep("GND2", 50.1, 8.0, 360.0, 120.0, 90.0, 0.0);
        i.on_ground = Some(true);
        m.ingest(Source::Vatsim, vec![r, i], 0.0);
        let list = m.merge(&[], None, 10.0);
        assert!(list.iter().all(|i| i.on_ground && !i.ra_allowed && i.lon == 8.0));
    }

    #[test]
    fn stale_network_traffic_is_capped_at_ta_in_the_computer() {
        use crate::util::{Arinc429, Ssm};
        let nominal = |v: f32| Arinc429::to_raw(v, Ssm::NormalOperation);
        let mut inputs = Inputs { lat: 50.0, lon: 8.0, alt_ft: 10000.0, xpdr_system: 1.0, ..Inputs::default() };
        inputs.adr_altitude = [nominal(10000.0); 3];
        inputs.radio_alt = [nominal(5000.0); 2];
        inputs.alt_above_ground = 9000.0;
        inputs.ir_true_heading = [nominal(0.0); 3];
        inputs.traffic_left_efis = true;
        let run = |age: f64| {
            let mut tcas = Tcas::new();
            let mut m = TrafficMerger::default();
            let mut peak = 0u8;
            for step in 0..70 {
                let now = step as f64;
                let lat = 50.0 + (5.0 - 360.0 * now / 3600.0) / 60.0;
                m.ingest(Source::Vatsim, vec![rep("HEAD", lat, 8.0, 10000.0, 360.0, 180.0, now - age)], now);
                let list = m.merge(&[], None, now);
                let up = update_tcas(&mut tcas, 1000.0, &inputs, &list);
                peak = peak.max(up.tracks.iter().map(|t| t.intrusion_level).max().unwrap_or(0));
            }
            peak
        };
        assert_eq!(run(0.0), crate::consts::INTR_RA);
        assert_eq!(run(30.0), crate::consts::INTR_TA);
    }
}

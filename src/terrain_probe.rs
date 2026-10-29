//! FlyByWire's EfisTawsBridge terrain feed, natively: X-Plane terrain probes into crates/terrain_map,
//! the ND bands and VD profile on `a380.terrain.nd/vd.<L|R>`, the TERR legend L:vars, and the TAWS
//! look-ahead (crates/gpws) on the same grid. Publishes only while `terrain_feed` is set in
//! `L:A380X_NATIVE_SYSHOST_MASK`; otherwise runs in shadow and logs a summary.

use std::cell::RefCell;
use std::sync::Mutex;

use gpws::arinc::Word;
use gpws::taws::{params, Taws, TawsInputs, TawsOutputs, TerrainSampler};
use serde_json::Value;
use terrain_map::host::{Efis, FmsPath, SideOutput, Status, TerrainHost, PROBE_BUDGET};
use terrain_map::{AlertArea, AlertLevel, Probe, M_TO_FT};

use crate::mapdata::terrain::terronnd::LVAR_THRESHOLDS;
use crate::native_systems_host_patches::BIT_TERRAIN_FEED;
use crate::{SimulatorReaderWriter, Vars, NAMED};

const MASK_VAR: &str = "A380X_NATIVE_SYSHOST_MASK";
const SIDES: [char; 2] = ['L', 'R'];
const SUMMARY_PERIOD_S: f64 = 10.;
const FMS_VECTORS_KEY: &str = "A32NX_EFIS_VECTORS_L_ACTIVE";
/// GENERIC: below this radio height the aircraft counts as on the ground for the look-ahead.
const ON_GROUND_RA_FT: f64 = 5.;
const MAX_STEP_S: f64 = 0.25;

type Vector = ((f64, f64), (f64, f64));

#[derive(Default)]
struct FmsIn {
    vectors: Vec<Vector>,
    track_change_nm: Option<f64>,
    dirty: bool,
}

static FMS: Mutex<Option<FmsIn>> = Mutex::new(None);

fn with_fms(f: impl FnOnce(&mut FmsIn)) {
    let mut g = FMS.lock().unwrap_or_else(|e| e.into_inner());
    let fms = g.get_or_insert_with(FmsIn::default);
    f(fms);
    fms.dirty = true;
}

fn point(v: &Value) -> Option<(f64, f64)> {
    Some((v.get("lat")?.as_f64()?, v.get("long")?.as_f64()?))
}

/// `PathVector`s as FlyByWire's EfisVectors transmits them; debug points are skipped.
pub fn parse_vectors(v: &Value) -> Vec<Vector> {
    v.as_array()
        .into_iter()
        .flatten()
        .filter(|p| p.get("type").and_then(Value::as_i64) != Some(3))
        .filter_map(|p| Some((point(p.get("startPoint")?)?, point(p.get("endPoint")?)?)))
        .collect()
}

/// The active flight plan's path vectors (captain side), from crates/fms_host or the JS FMS.
pub fn fms_vectors(v: &Value) {
    let vectors = parse_vectors(v);
    with_fms(|f| f.vectors = vectors);
}

/// `a32nx_fms_vd_track_change_distance`.
pub fn fms_track_change(nm: Option<f64>) {
    with_fms(|f| f.track_change_nm = nm.filter(|d| d.is_finite() && *d >= 0.));
}

/// GenericData seen by the bridge: picks up the JS FMS's active vectors.
pub fn tap_generic_data(key: &str, data: &str) {
    if key == FMS_VECTORS_KEY {
        if let Ok(v) = serde_json::from_str::<Value>(data) {
            fms_vectors(&v);
        }
    }
}

fn word(read: &impl Fn(&str) -> f64, name: &str) -> Word {
    Word::from_raw(read(name))
}

fn first_normal(read: &impl Fn(&str) -> f64, pattern: &str) -> Option<f64> {
    (1..=3).map(|n| word(read, &pattern.replace('#', &n.to_string()))).find(|w| w.is_normal_operation()).map(|w| w.value as f64)
}

fn on(read: &impl Fn(&str) -> f64, name: &str) -> bool {
    read(name) != 0.
}

/// FBW `shouldShowTrackLine`.
pub fn track_line_shown(lateral_mode: i32, armed: u32) -> bool {
    matches!(lateral_mode, 0 | 10 | 11 | 40 | 41 | 50) && armed & 1 == 0
}

/// The aircraft status and EFIS data FBW's EfisTawsBridge assembles, from the same variables.
pub fn read_status(read: &impl Fn(&str) -> f64) -> Status {
    let [lat, lon, alt, hdg, vs] = ["LAT", "LONG", "ALTITUDE", "HEADING", "VERTICAL_SPEED"].map(|n| word(read, &format!("A32NX_EGPWC_PRESENT_{n}")));
    let valid = [lat, lon, alt, hdg, vs].iter().all(|w| w.is_normal_operation());
    let dest = [word(read, "A32NX_EGPWC_DEST_LAT"), word(read, "A32NX_EGPWC_DEST_LONG")];
    let dest = dest.iter().all(|w| w.is_normal_operation()).then(|| (dest[0].value as f64, dest[1].value as f64));
    let terr_failed = match read("A32NX_WXR_TAWS_SYS_SELECTED") as i32 {
        1 => on(read, "A32NX_TERR_1_FAILED"),
        2 => on(read, "A32NX_TERR_2_FAILED"),
        _ => true,
    };
    let swtg = read("A32NX_ATT_HDG_SWITCHING_KNOB") as i32;
    let ir_ok = |n: usize, what: &str| word(read, &format!("A32NX_ADIRS_IR_{n}_{what}")).is_normal_operation();
    let power = [
        on(read, "A32NX_ELEC_108PH_BUS_IS_POWERED") || on(read, "A32NX_ELEC_DC_1_BUS_IS_POWERED"),
        on(read, "A32NX_ELEC_DC_1_BUS_IS_POWERED") || on(read, "A32NX_ELEC_DC_2_BUS_IS_POWERED"),
    ];
    let efis = std::array::from_fn(|side| {
        let s = SIDES[side];
        let ir = if swtg == 1 { side + 1 } else { 3 };
        Efis {
            range_index: read(&format!("A32NX_EFIS_{s}_ND_RANGE")).max(0.) as usize,
            mode: read(&format!("A32NX_EFIS_{s}_ND_MODE")) as i32,
            overlay: read(&format!("A380X_EFIS_{s}_ACTIVE_OVERLAY")) as i32,
            vd_lower_ft: read(&format!("A32NX_VD_{}_RANGE_LOWER", side + 1)),
            vd_upper_ft: read(&format!("A32NX_VD_{}_RANGE_UPPER", side + 1)),
            nd_avail: ir_ok(ir, "HEADING") && ir_ok(ir, "LATITUDE") && power[side],
            vd_avail: ir_ok(ir, "HEADING") && ir_ok(ir, "TRACK") && power[side],
        }
    });
    let heading = hdg.value as f64;
    Status {
        valid,
        lat: lat.value as f64,
        lon: lon.value as f64,
        alt_ft: alt.value as f64,
        heading_deg: heading,
        track_deg: first_normal(read, "A32NX_ADIRS_IR_#_TRUE_TRACK").unwrap_or(heading),
        vs_fpm: vs.value as f64,
        gear_down: on(read, "A32NX_EGPWC_GEAR_IS_DOWN"),
        dest,
        terr_failed,
        track_line: track_line_shown(read("A32NX_FMA_LATERAL_MODE") as i32, read("A32NX_FMA_LATERAL_ARMED") as u32),
        efis,
        taws_rnp_nm: 0.,
    }
}

fn dest_nm(s: &Status) -> Option<f64> {
    s.dest.map(|(la, lo)| {
        let (n, e) = terrain_map::GeoGrid::ne_nm(s.lat, s.lon, la, lo);
        n.hypot(e)
    })
}

fn clearance_ft(dest_nm: Option<f64>) -> f64 {
    let p = params::CLEARANCE_PROFILE;
    let Some(d) = dest_nm else { return p[p.len() - 1].1 };
    if d <= p[0].0 {
        return p[0].1;
    }
    for w in p.windows(2) {
        if d <= w[1].0 {
            return w[0].1 + (w[1].1 - w[0].1) * (d - w[0].0) / (w[1].0 - w[0].0);
        }
    }
    p[p.len() - 1].1
}

/// The solid caution/warning area for the active look-ahead alert (FCOM DSC-34-20-20-10).
pub fn alert_area(s: &Status, gs_kt: f64, flta: gpws::taws::AlertLevel) -> AlertArea {
    let level = match flta {
        gpws::taws::AlertLevel::None => AlertLevel::None,
        gpws::taws::AlertLevel::Caution => AlertLevel::Caution,
        gpws::taws::AlertLevel::Warning => AlertLevel::Warning,
    };
    let d = dest_nm(s);
    let terminal = d.is_some_and(|d| d <= params::TERMINAL_RADIUS_NM);
    AlertArea {
        level,
        lat: s.lat,
        lon: s.lon,
        alt_ft: s.alt_ft,
        vs_fpm: s.vs_fpm,
        gs_kt,
        track_deg: s.track_deg,
        caution_time_s: if terminal { params::TERMINAL_CAUTION_TIME_S } else { params::CAUTION_TIME_S },
        warning_time_s: if terminal { params::TERMINAL_WARNING_TIME_S } else { params::WARNING_TIME_S },
        half_angle_deg: params::CONE_HALF_ANGLE_DEG,
        start_width_nm: params::CONE_START_WIDTH_NM,
        clearance_ft: clearance_ft(d),
        max_lookahead_nm: params::MAX_LOOKAHEAD_NM,
    }
}

/// `xp-ui.bus` batch for the native displays' input tap.
pub fn bus_batch(side: usize, out: &SideOutput) -> String {
    let s = SIDES[side];
    format!(
        "{{\"events\":[{{\"topic\":\"{}\",\"data\":{}}},{{\"topic\":\"{}\",\"data\":{}}}]}}",
        terrain_map::publish::nd_topic(s),
        out.nd,
        terrain_map::publish::vd_topic(s),
        out.vd
    )
}

#[derive(Default)]
struct Summary {
    probes: usize,
    hits: usize,
    published: [Option<SideOutput>; 2],
}

struct Running {
    host: TerrainHost,
    taws: Taws,
    last_time: Option<f64>,
    last_track: Option<f64>,
    authoritative: bool,
    summary_at: f64,
    summary: Summary,
}

thread_local! {
    static RUNNING: RefCell<Option<Running>> = const { RefCell::new(None) };
}

fn read_var(vars: &Vars, name: &str) -> f64 {
    vars.ids.get(name).map_or(0., |id| vars.slots[id.identifier_type()][id.identifier_index()].value)
}

fn write_var(vars: &mut Vars, name: &str, value: f64) {
    let id = vars.add(name.to_string(), NAMED);
    vars.write(&id, value);
}

/// Probe-grid elevation in feet; the TAWS look-ahead's terrain. Main thread only.
pub fn elevation_ft(lat: f64, lon: f64) -> Option<f64> {
    RUNNING.with(|c| c.borrow().as_ref().and_then(|r| r.host.elevation_ft(lat, lon)))
}

pub fn sampler() -> impl TerrainSampler {
    elevation_ft
}

pub fn taws_outputs() -> Option<TawsOutputs> {
    RUNNING.with(|c| c.borrow().as_ref().map(|r| r.taws.outputs()))
}

fn probe(lat: f64, lon: f64) -> Probe {
    match crate::xp::probe_terrain_at(lat, lon) {
        Some((m, water)) => Probe::Hit { elevation_ft: m * M_TO_FT, water },
        None => Probe::Miss,
    }
}

/// Each frame, after the systems and the mapdata update.
pub fn tick(vars: &mut Vars, time: f64) {
    let mut lines = Vec::new();
    RUNNING.with(|cell| {
        let mut guard = cell.borrow_mut();
        let r = guard.get_or_insert_with(|| Running {
            host: TerrainHost::new(),
            taws: Taws::new(),
            last_time: None,
            last_track: None,
            authoritative: false,
            summary_at: time,
            summary: Summary::default(),
        });
        let dt = r.last_time.map_or(0., |t| (time - t).clamp(0., MAX_STEP_S));
        r.last_time = Some(time);
        let authoritative = (read_var(vars, MASK_VAR) as u32) & BIT_TERRAIN_FEED != 0;
        if authoritative != r.authoritative {
            r.authoritative = authoritative;
            lines.push(format!("terrain feed: {}", if authoritative { "authoritative" } else { "shadow" }));
            for side in 0..2 {
                crate::mapdata::plugin::set_native_terrain(side, authoritative);
            }
        }
        if let Some(f) = FMS.lock().unwrap_or_else(|e| e.into_inner()).as_mut().filter(|f| f.dirty) {
            f.dirty = false;
            r.host.set_fms_path(FmsPath::from_vectors(&f.vectors, f.track_change_nm));
        }

        let read = |n: &str| read_var(vars, n);
        let s = read_status(&read);
        let reqs = r.host.plan(&s, PROBE_BUDGET);
        r.summary.probes += reqs.len();
        for q in &reqs {
            let p = probe(q.lat, q.lon);
            r.summary.hits += matches!(p, Probe::Hit { .. }) as usize;
            r.host.store(q, p);
        }

        let gs_kt = first_normal(&read, "A32NX_ADIRS_IR_#_GROUND_SPEED").unwrap_or(0.);
        let ra = word(&read, "A32NX_RA_1_RADIO_ALTITUDE");
        let ra_ft = ra.is_normal_operation().then_some(ra.value as f64);
        let track_rate = match r.last_track {
            Some(t) if dt > 0. => gpws::taws::angle_diff(s.track_deg, t) / dt,
            _ => 0.,
        };
        r.last_track = Some(s.track_deg);
        let inputs = TawsInputs {
            lat: s.lat,
            lon: s.lon,
            alt_ft: s.alt_ft,
            radio_alt_ft: ra_ft,
            gs_kt,
            track_deg: s.track_deg,
            track_rate_dps: track_rate,
            vs_fpm: s.vs_fpm,
            on_ground: ra_ft.is_some_and(|h| h < ON_GROUND_RA_FT),
            landing_config: s.gear_down,
            glideslope_tracking: false,
            terr_off: on(&read, "A32NX_GPWS_TERR_OFF"),
            system_off: on(&read, "A32NX_GPWS_SYS_OFF"),
            failed: s.terr_failed || !s.valid,
            runways: &[],
        };
        let host = &r.host;
        r.taws.update(dt * 1000., &inputs, &|lat: f64, lon: f64| host.elevation_ft(lat, lon), None);
        let alert = alert_area(&s, gs_kt, r.taws.outputs().flta);

        for (side, out) in r.host.update(time * 1000., &s, &alert) {
            if r.authoritative {
                for (name, value) in LVAR_THRESHOLDS[side].iter().zip(out.legend) {
                    write_var(vars, name, value);
                }
                #[cfg(feature = "native-displays")]
                crate::native_displays::inputs::tap_generic_data("xp-ui.bus", &bus_batch(side, &out));
            }
            r.summary.published[side] = Some(out);
        }

        if time - r.summary_at >= SUMMARY_PERIOD_S || time < r.summary_at {
            r.summary_at = time;
            let filled = (0..r.host.map.levels.len()).map(|l| format!("{:.0}", r.host.map.filled(l) * 100.)).collect::<Vec<_>>().join("/");
            let mut line = format!(
                "terrain feed: {} probes={} hits={} filled%={filled} flta={:?}",
                if r.authoritative { "authoritative" } else { "shadow" },
                r.summary.probes,
                r.summary.hits,
                r.taws.outputs().flta
            );
            for side in 0..2 {
                if let Some(o) = &r.summary.published[side] {
                    let fbw: Vec<f64> = LVAR_THRESHOLDS[side].iter().map(|n| read_var(vars, n)).collect();
                    line.push_str(&format!(" {}: nd={} vd={} legend={:?} fbw={:?}", SIDES[side], o.nd_on, o.vd_on, o.legend, fbw));
                }
            }
            lines.push(line);
            r.summary.probes = 0;
            r.summary.hits = 0;
        }
    });
    for line in lines {
        crate::log(&line);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn normal(v: f64) -> f64 {
        Word::normal(v as f32).to_raw()
    }

    fn vars() -> HashMap<String, f64> {
        let mut m: HashMap<String, f64> = [
            ("A32NX_EGPWC_PRESENT_LAT", normal(47.)),
            ("A32NX_EGPWC_PRESENT_LONG", normal(11.)),
            ("A32NX_EGPWC_PRESENT_ALTITUDE", normal(9000.)),
            ("A32NX_EGPWC_PRESENT_HEADING", normal(90.)),
            ("A32NX_EGPWC_PRESENT_VERTICAL_SPEED", normal(-500.)),
            ("A32NX_WXR_TAWS_SYS_SELECTED", 1.),
            ("A32NX_ATT_HDG_SWITCHING_KNOB", 1.),
            ("A32NX_ELEC_DC_1_BUS_IS_POWERED", 1.),
            ("A32NX_EFIS_L_ND_RANGE", 3.),
            ("A32NX_EFIS_L_ND_MODE", 3.),
            ("A380X_EFIS_L_ACTIVE_OVERLAY", 2.),
            ("A32NX_FMA_LATERAL_MODE", 20.),
            ("A32NX_ADIRS_IR_2_TRUE_TRACK", normal(95.)),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();
        for n in 1..=3 {
            for w in ["HEADING", "LATITUDE", "TRACK"] {
                m.insert(format!("A32NX_ADIRS_IR_{n}_{w}"), normal(1.));
            }
        }
        m
    }

    fn status(m: &HashMap<String, f64>) -> Status {
        read_status(&|n: &str| m.get(n).copied().unwrap_or(0.))
    }

    #[test]
    fn status_matches_the_bridge() {
        let s = status(&vars());
        assert!(s.valid);
        assert_eq!((s.lat, s.lon, s.alt_ft, s.heading_deg, s.vs_fpm), (47., 11., 9000., 90., -500.));
        assert_eq!(s.track_deg, 95.);
        assert!(!s.terr_failed && !s.track_line && s.dest.is_none());
        assert!(s.efis[0].terr_on_nd(false) && s.efis[0].terr_on_vd(false));
        assert_eq!(s.efis[0].range_nm(), 40.);
        assert!(s.efis[1].nd_avail, "DC 1 powers both sides");
        assert!(!s.efis[1].terr_on_nd(false));
    }

    #[test]
    fn failures_power_and_validity() {
        let mut m = vars();
        m.insert("A32NX_TERR_1_FAILED".into(), 1.);
        assert!(status(&m).terr_failed);
        m.insert("A32NX_WXR_TAWS_SYS_SELECTED".into(), 2.);
        assert!(!status(&m).terr_failed);
        m.insert("A32NX_WXR_TAWS_SYS_SELECTED".into(), 0.);
        assert!(status(&m).terr_failed);
        let mut m = vars();
        m.insert("A32NX_ELEC_DC_1_BUS_IS_POWERED".into(), 0.);
        m.insert("A32NX_ELEC_108PH_BUS_IS_POWERED".into(), 1.);
        let s = status(&m);
        assert!(s.efis[0].nd_avail && !s.efis[1].nd_avail);
        m.insert("A32NX_EGPWC_PRESENT_LAT".into(), Word::failed().to_raw());
        assert!(!status(&m).valid);
        let mut m = vars();
        m.insert("A32NX_ATT_HDG_SWITCHING_KNOB".into(), 0.);
        m.insert("A32NX_ADIRS_IR_3_HEADING".into(), Word::failed().to_raw());
        assert!(!status(&m).efis[0].vd_avail);
    }

    #[test]
    fn track_line_follows_the_lateral_modes() {
        assert!(track_line_shown(10, 0));
        assert!(track_line_shown(0, 0));
        assert!(!track_line_shown(10, 1));
        assert!(!track_line_shown(20, 0));
        assert!(track_line_shown(41, 2));
    }

    #[test]
    fn vectors_parse_from_the_fms_json() {
        let v: Value = serde_json::from_str(
            r#"[{"type":0,"startPoint":{"lat":1,"long":2},"endPoint":{"lat":3,"long":4}},{"type":3,"startPoint":{"lat":9,"long":9}},{"type":1,"startPoint":{"lat":3,"long":4},"centrePoint":{"lat":0,"long":0},"endPoint":{"lat":5,"long":6},"sweepAngle":10}]"#,
        )
        .unwrap();
        assert_eq!(parse_vectors(&v), vec![((1., 2.), (3., 4.)), ((3., 4.), (5., 6.))]);
        assert!(parse_vectors(&Value::Null).is_empty());
    }

    #[test]
    fn alert_area_uses_terminal_times_near_the_destination() {
        let mut m = vars();
        m.insert("A32NX_EGPWC_DEST_LAT".into(), normal(47.1));
        m.insert("A32NX_EGPWC_DEST_LONG".into(), normal(11.));
        let s = status(&m);
        let a = alert_area(&s, 250., gpws::taws::AlertLevel::Caution);
        assert_eq!(a.level, AlertLevel::Caution);
        assert_eq!(a.caution_time_s, params::TERMINAL_CAUTION_TIME_S);
        assert!(a.clearance_ft > 100. && a.clearance_ft < 500.);
        let s = status(&vars());
        let a = alert_area(&s, 250., gpws::taws::AlertLevel::None);
        assert_eq!(a.caution_time_s, params::CAUTION_TIME_S);
        assert_eq!(a.clearance_ft, 700.);
    }

    #[test]
    fn bus_batch_carries_both_topics() {
        let out = SideOutput { nd: "{\"off\":true}".into(), vd: "{\"gen\":1}".into(), legend: [0.; 4], nd_on: false, vd_on: true };
        let b: Value = serde_json::from_str(&bus_batch(1, &out)).unwrap();
        assert_eq!(b["events"][0]["topic"], "a380.terrain.nd.R");
        assert_eq!(b["events"][1]["data"]["gen"], 1);
    }
}

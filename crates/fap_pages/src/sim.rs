use mfd_tree::{Node, Shape};
use std::sync::OnceLock;

pub const DOORS: [(&str, f64, f64, char); 18] = [
    ("M1L", -3.05, -28.85, 'M'),
    ("M1R", 3.05, -28.85, 'M'),
    ("M2L", -3.58, -18.65, 'M'),
    ("M2R", 3.58, -18.65, 'M'),
    ("M3L", -3.71, -2.44, 'M'),
    ("M3R", 3.71, -2.44, 'M'),
    ("M4L", -3.57, 9.66, 'M'),
    ("M4R", 3.57, 9.66, 'M'),
    ("M5L", -3.35, 18.6, 'M'),
    ("M5R", 3.35, 18.6, 'M'),
    ("U1L", -3.38, -14.2, 'U'),
    ("U1R", 3.38, -14.2, 'U'),
    ("U2L", -3.36, 5.21, 'U'),
    ("U2R", 3.36, 5.21, 'U'),
    ("U3L", -3.37, 14.1, 'U'),
    ("U3R", 3.37, 14.1, 'U'),
    ("FWD CARGO", 3.4, -23.45, 'C'),
    ("AFT CARGO", 3.37, 13.03, 'C'),
];
pub const ZONES: [&str; 3] = ["FWD", "MID", "AFT"];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DoorMark {
    #[default]
    Na,
    Open,
    Moving,
    Armed,
    Disarmed,
}

impl DoorMark {
    pub fn colour(self) -> &'static str {
        match self {
            DoorMark::Armed => "#1f3dff",
            DoorMark::Disarmed => "#25cf3a",
            DoorMark::Moving => "#ff9a1a",
            DoorMark::Open => "#8c0d0d",
            DoorMark::Na => "#777",
        }
    }

    pub fn from_view(d: &cids::doors::DoorView) -> Self {
        use cids::doors::{DoorState, SlideState};
        match (d.state, d.slide) {
            (DoorState::Open, _) => DoorMark::Open,
            (DoorState::Moving, _) => DoorMark::Moving,
            (DoorState::Closed, SlideState::Armed) => DoorMark::Armed,
            _ => DoorMark::Disarmed,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FapSim {
    pub powered: bool,
    pub doors: [DoorMark; 18],
    pub main_temp: [Option<f64>; 8],
    pub upper_temp: [Option<f64>; 7],
    pub ckpt_temp: Option<f64>,
    pub cargo_fwd_temp: Option<f64>,
    pub cargo_bulk_temp: Option<f64>,
    pub purs_sel: Option<f64>,
    pub purser_sel_fault: bool,
    pub waste_pct: [Option<f64>; 3],
    pub waste_full: [bool; 3],
    pub water_pct: Option<f64>,
    pub water_kg: Option<f64>,
    pub water_press: Option<f64>,
    pub compressor: bool,
    pub leak: bool,
    pub generator: bool,
    pub mast_blocked: bool,
    pub lav_inop: [bool; 3],
    pub heater_temp: [Option<f64>; 3],
    pub heaters_off: bool,
    pub calls: f64,
    pub slide_low: bool,
    pub cabin_ready: bool,
    pub evac: bool,
    pub seat_power_cmd: bool,
    pub ann_enabled: bool,
    pub cargo_smoke: [Option<f64>; 2],
    pub ife_smoke: [Option<f64>; 3],
    pub galley_smoke: [Option<f64>; 3],
    pub ife_tripped: [bool; 3],
    pub ife_hot: [bool; 3],
    pub ife_temp: [Option<f64>; 3],
    pub seat_power_off: bool,
    pub pax_sys_off: bool,
    pub emer_light: bool,
    pub cids_fault: [bool; 3],
    pub ife_server_fail: [bool; 2],
    pub extract_fault: bool,
    pub fans_failed: f64,
    pub ife_bay_fault: bool,
    pub galley_off: [bool; 3],
    pub galley_bus_fault: [bool; 3],
    pub oven: [Option<f64>; 3],
    pub chiller: [Option<f64>; 3],
    pub boiler: [Option<f64>; 3],
    pub scene_raw: f64,
    pub level_raw: f64,
}

impl FapSim {
    pub fn read(get: impl Fn(&str) -> Option<f64>) -> Self {
        let l = |n: &str| get(&format!("L:{n}"));
        let on = |n: &str| l(n).unwrap_or(0.) >= 0.5;
        let z3 = |f: &dyn Fn(usize) -> Option<f64>| [f(1), f(2), f(3)];
        let b3 = |f: &dyn Fn(usize) -> bool| [f(1), f(2), f(3)];
        let mut doors = [DoorMark::Na; 18];
        for (i, d) in DOORS.iter().enumerate() {
            doors[i] = match get(&format!("A:INTERACTIVE POINT OPEN:{i}")) {
                None => DoorMark::Na,
                Some(p) if p > 98. => DoorMark::Open,
                Some(p) if p > 2. => DoorMark::Moving,
                Some(_) if i >= 16 => DoorMark::Disarmed,
                Some(_) => {
                    if l(&format!("ANIM_DOOR_{}_DISARMED", d.0)).unwrap_or(1.) < 0.5 {
                        DoorMark::Armed
                    } else {
                        DoorMark::Disarmed
                    }
                }
            };
        }
        let mut main_temp = [None; 8];
        for (i, t) in main_temp.iter_mut().enumerate() {
            *t = l(&format!("A32NX_COND_MAIN_DECK_{}_TEMP", i + 1));
        }
        let mut upper_temp = [None; 7];
        for (i, t) in upper_temp.iter_mut().enumerate() {
            *t = l(&format!("A32NX_COND_UPPER_DECK_{}_TEMP", i + 1));
        }
        FapSim {
            powered: (1..=4).any(|n| on(&format!("A32NX_ELEC_AC_{n}_BUS_IS_POWERED"))),
            doors,
            main_temp,
            upper_temp,
            ckpt_temp: l("A32NX_COND_CKPT_TEMP"),
            cargo_fwd_temp: l("A32NX_COND_CARGO_FWD_TEMP"),
            cargo_bulk_temp: l("A32NX_COND_CARGO_BULK_TEMP"),
            purs_sel: l("A32NX_COND_PURS_SEL_TEMPERATURE"),
            purser_sel_fault: on("A32NX_DEEP_CABIN_PURSER_TEMP_SEL_FAULT"),
            waste_pct: z3(&|i| l(&format!("A32NX_CABIN_WASTE_LEVEL_PERCENT:{i}"))),
            waste_full: b3(&|i| on(&format!("A32NX_CABIN_WASTE_TANK_FULL:{i}"))),
            water_pct: l("A32NX_CABIN_WATER_QTY_PERCENT"),
            water_kg: l("A32NX_CABIN_WATER_QTY_KG"),
            water_press: l("A32NX_CABIN_WATER_PRESS_PSI"),
            compressor: on("A32NX_CABIN_WATER_COMPRESSOR_CMD"),
            leak: l("A32NX_CABIN_WATER_LEAK_L_S").unwrap_or(0.) > 0.001,
            generator: on("A32NX_CABIN_WASTE_GENERATOR_RUNNING"),
            mast_blocked: on("A32NX_CABIN_MAST_BLOCKED:1") || on("A32NX_CABIN_MAST_BLOCKED:2"),
            lav_inop: b3(&|i| on(&format!("A32NX_CABIN_LAV_INOPERATIVE:{i}"))),
            heater_temp: z3(&|i| l(&format!("A32NX_CABIN_WATER_HEATER_TEMP_C:{i}"))),
            heaters_off: on("A380X_FAP_WATER_HEATERS_OFF"),
            calls: l("A32NX_CABIN_CREW_CALL_NEW_COUNT").unwrap_or(0.),
            slide_low: on("A32NX_CABIN_SLIDE_PRESSURE_LOW:1"),
            cabin_ready: on("A32NX_CABIN_READY"),
            evac: on("A32NX_EVAC_COMMAND_TOGGLE"),
            seat_power_cmd: on("A32NX_CABIN_SEAT_POWER_CMD"),
            ann_enabled: on("A32NX_SOUND_ANNOUNCEMENTS_ENABLED"),
            cargo_smoke: [l("A32NX_CARGO_FWD_SMOKE_DETECTED"), l("A32NX_CARGO_AFT_SMOKE_DETECTED")],
            ife_smoke: z3(&|i| l(&format!("A32NX_CABIN_IFE_ZONE_SMOKE:{i}"))),
            galley_smoke: z3(&|i| l(&format!("A32NX_CABIN_GALLEY_OVEN_SMOKE:{i}"))),
            ife_tripped: b3(&|i| on(&format!("A32NX_CABIN_IFE_ZONE_TRIPPED:{i}"))),
            ife_hot: b3(&|i| on(&format!("A32NX_CABIN_IFE_ZONE_OVERHEAT:{i}"))),
            ife_temp: z3(&|i| l(&format!("A32NX_CABIN_IFE_ZONE_TEMP_C:{i}"))),
            seat_power_off: on("A380X_FAP_SEAT_POWER_OFF"),
            pax_sys_off: on("A380X_FAP_PAX_SYS_OFF"),
            emer_light: on("A380X_FAP_EMER_LIGHT"),
            cids_fault: b3(&|i| on(&format!("A32NX_DEEP_COM_CIDS_{i}_FAULT"))),
            ife_server_fail: [on("A32NX_CABIN_IFE_SERVER_FAIL:1"), on("A32NX_CABIN_IFE_SERVER_FAIL:2")],
            extract_fault: on("A32NX_DEEP_CABIN_LAV_GALLEY_EXTRACT_FAULT"),
            fans_failed: l("A32NX_DEEP_CABIN_SECONDARY_FANS_FAILED_COUNT").unwrap_or(0.),
            ife_bay_fault: on("A32NX_DEEP_CABIN_IFE_BAY_VENT_FAULT") || on("A32NX_DEEP_CABIN_IFE_BAY_ISOL_FAULT"),
            galley_off: b3(&|i| on(&format!("A380X_FAP_GALLEY_OFF_{i}"))),
            galley_bus_fault: b3(&|i| on(&format!("A32NX_CABIN_GALLEY_BUS_FAULT:{i}"))),
            oven: z3(&|i| l(&format!("A32NX_CABIN_GALLEY_OVEN_TEMP_C:{i}"))),
            chiller: z3(&|i| l(&format!("A32NX_CABIN_GALLEY_CHILLER_TEMP_C:{i}"))),
            boiler: z3(&|i| l(&format!("A32NX_CABIN_GALLEY_BOILER_TEMP_C:{i}"))),
            scene_raw: l("A380X_FAP_CABIN_SCENE").unwrap_or(0.).round(),
            level_raw: l("A380X_FAP_CABIN_LEVEL").unwrap_or(0.).round(),
        }
    }

    pub fn smoke_on(v: Option<f64>) -> bool {
        v.unwrap_or(0.) >= 0.5
    }
}

pub(crate) fn shape(t: &str, attrs: &[(&str, &str)], depth: usize) -> Shape {
    let mut s = Shape { t: t.to_string(), ..Default::default() };
    for (k, v) in attrs {
        s.a.insert(k.to_string(), v.to_string());
    }
    s.a.insert("_d".into(), depth.to_string());
    s
}

fn silhouette() -> &'static [(f64, f64)] {
    static SIL: OnceLock<Vec<(f64, f64)>> = OnceLock::new();
    SIL.get_or_init(|| {
        let src = include_str!("../../../html_ui/Pages/VCockpit/Instruments/A380X/FAP/sil.js");
        let json = src.split_once('=').map(|x| x.1).unwrap_or("{}").trim().trim_end_matches(';');
        let v: serde_json::Value = serde_json::from_str(json).unwrap_or_default();
        v["top"]
            .as_array()
            .map(|a| a.iter().filter_map(|p| Some((p.get(0)?.as_f64()?, p.get(1)?.as_f64()?))).collect())
            .unwrap_or_default()
    })
}

pub struct Plan {
    pub s: f64,
    cx: f64,
    top: f64,
}

impl Plan {
    pub fn p(&self, x: f64, z: f64) -> (f64, f64) {
        (self.cx + x * self.s, self.top + z * self.s)
    }
}

pub(crate) fn plan(w: f64, h: f64, marks: impl FnOnce(&Plan) -> Vec<Shape>) -> Node {
    let s = ((w - 20.) / 80.).min((h - 16.) / 73.);
    let pl = Plan { s, cx: w / 2., top: (h - 73. * s) / 2. + 35.2 * s };
    let mut d = String::new();
    for (i, (x, z)) in silhouette().iter().enumerate() {
        let (px, py) = pl.p(*x, *z);
        d.push_str(&format!("{}{px:.1},{py:.1}", if i == 0 { 'M' } else { 'L' }));
    }
    d.push('Z');
    let mut svg = Node::new("svg").style("position:absolute;left:0;top:0");
    svg.svgattr.insert("width".into(), format!("{w}"));
    svg.svgattr.insert("height".into(), format!("{h}"));
    svg.svgattr.insert("viewBox".into(), format!("0 0 {w} {h}"));
    svg.shapes.push(shape("path", &[("d", &d), ("fill", "#45474b"), ("stroke", "#d6d9dc"), ("stroke-width", "1.1"), ("stroke-linejoin", "round")], 0));
    svg.shapes.extend(marks(&pl));
    svg
}

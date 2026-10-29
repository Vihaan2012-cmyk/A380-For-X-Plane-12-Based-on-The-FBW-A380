use crate::constants::*;
use crate::data;
use crate::isa::mach_to_kcas;
use crate::table::{interp_pairs, Table1, Table2};
use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ApproachConf {
    Conf1 = 1,
    Conf1F = 2,
    Conf2 = 3,
    Conf3 = 4,
    ConfFull = 5,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum FlightPhase {
    Preflight = 0,
    Takeoff = 1,
    Climb = 2,
    Cruise = 3,
    Descent = 4,
    Approach = 5,
    GoAround = 6,
    Done = 7,
}

impl FlightPhase {
    pub fn from_index(i: i32) -> Option<FlightPhase> {
        Some(match i {
            0 => FlightPhase::Preflight,
            1 => FlightPhase::Takeoff,
            2 => FlightPhase::Climb,
            3 => FlightPhase::Cruise,
            4 => FlightPhase::Descent,
            5 => FlightPhase::Approach,
            6 => FlightPhase::GoAround,
            7 => FlightPhase::Done,
            _ => return None,
        })
    }
}

struct Tables {
    appr: [Table2; 5],
    s: Table1,
    f2: Table2,
    f3: Table2,
    green_dot: Table2,
    conf0: Table2,
}

fn tables() -> &'static Tables {
    static T: OnceLock<Tables> = OnceLock::new();
    T.get_or_init(|| Tables {
        appr: [
            Table2::new(data::VLS_CONF_1),
            Table2::new(data::VLS_CONF_1F),
            Table2::new(data::VLS_CONF_2),
            Table2::new(data::VLS_CONF_3),
            Table2::new(data::VLS_CONF_FULL),
        ],
        s: Table1::new(data::S_SPEED),
        f2: Table2::new(data::F2_SPEED),
        f3: Table2::new(data::F3_SPEED),
        green_dot: Table2::new(data::GREEN_DOT),
        conf0: Table2::new(data::VLS_CONF_0),
    })
}

pub struct SpeedsLookupTables;

impl SpeedsLookupTables {
    pub fn vls_conf0(altitude_ft: f64, weight_kg: f64) -> f64 {
        tables().conf0.get(altitude_ft, weight_kg)
    }

    pub fn approach_vls(conf: u8, cg: f64, weight_kg: f64) -> f64 {
        if conf == 0 {
            return Self::vls_conf0(0.0, weight_kg);
        }
        assert!(conf <= 5, "invalid flap config for approach VLS computation");
        tables().appr[conf as usize - 1].get(cg, weight_kg)
    }

    pub fn s_speed(weight_kg: f64) -> f64 {
        tables().s.get(weight_kg)
    }

    pub fn f2_speed(cg: f64, weight_kg: f64) -> f64 {
        tables().f2.get(cg, weight_kg)
    }

    pub fn f3_speed(cg: f64, weight_kg: f64) -> f64 {
        tables().f3.get(cg, weight_kg)
    }

    pub fn green_dot(altitude_ft: f64, weight_kg: f64) -> f64 {
        tables().green_dot.get(altitude_ft, weight_kg)
    }
}

const VMCA: &[(f64, f64)] = &[
    (-2000.0, 115.0),
    (0.0, 114.0),
    (2000.0, 114.0),
    (4000.0, 113.0),
    (6000.0, 112.0),
    (8000.0, 109.0),
    (10000.0, 106.0),
    (12000.0, 103.0),
    (14100.0, 99.0),
    (15100.0, 97.0),
];

const VMCG: &[(f64, f64)] = &[
    (-2000.0, 117.0),
    (0.0, 116.0),
    (2000.0, 116.0),
    (4000.0, 115.0),
    (6000.0, 114.0),
    (8000.0, 112.0),
    (10000.0, 109.0),
    (12000.0, 106.0),
    (14100.0, 102.0),
    (15100.0, 101.0),
];

const VFE_FS: [f64; 6] = [VMO, VFE_F1, VFE_F1F, VFE_F2, VFE_F3, VFE_FF];

pub fn add_wind_component(vw: f64) -> f64 {
    vw.min(15.0).max(5.0)
}

fn diff_angle(a: f64, b: f64) -> f64 {
    180.0 - ((a - b).abs() - 180.0).abs()
}

pub fn js_round(x: f64) -> f64 {
    (x + 0.5).floor()
}

pub fn vmo_knots(static_pressure_hpa: Option<f64>) -> f64 {
    let p = static_pressure_hpa.unwrap_or(1013.25);
    VMO.min(mach_to_kcas(MMO, p))
}

#[derive(Clone, Copy, Debug)]
pub struct OperatingSpeedsInput {
    pub mass_kg: f64,
    pub cg_percent_mac: f64,
    pub cas_kt: f64,
    pub flap_position: i32,
    pub phase: FlightPhase,
    pub v2_kt: f64,
    pub altitude_ft: f64,
    pub wind_kt: f64,
    pub ignore_spoilers: bool,
    pub left_spoiler_1_position: f64,
    pub static_pressure_hpa: Option<f64>,
}

impl Default for OperatingSpeedsInput {
    fn default() -> Self {
        OperatingSpeedsInput {
            mass_kg: 0.0,
            cg_percent_mac: 35.0,
            cas_kt: 0.0,
            flap_position: 0,
            phase: FlightPhase::Preflight,
            v2_kt: 150.0,
            altitude_ft: 0.0,
            wind_kt: 0.0,
            ignore_spoilers: false,
            left_spoiler_1_position: 0.0,
            static_pressure_hpa: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct OperatingSpeeds {
    pub vs1g: f64,
    pub vls: f64,
    pub vapp: f64,
    pub vref: f64,
    pub f2: f64,
    pub f3: f64,
    pub s: f64,
    pub gd: f64,
    pub vmax: f64,
    pub vfe_next: f64,
}

impl OperatingSpeeds {
    pub fn compute(i: &OperatingSpeedsInput) -> OperatingSpeeds {
        let m = i.mass_kg;
        let cg = i.cg_percent_mac;
        let f_pos = i.flap_position;
        let alt = i.altitude_ft;
        let mut o = OperatingSpeeds::default();

        if f_pos == 0 {
            o.vls = SpeedsLookupTables::vls_conf0(alt, m);
            o.vmax = vmo_knots(i.static_pressure_hpa);
            o.vfe_next = VFE_FS[1];
        } else if f_pos == 1 && i.cas_kt > 212.0 {
            o.vls = SpeedsLookupTables::approach_vls(ApproachConf::Conf1 as u8, cg, m);
            o.vmax = VFE_FS[1];
            o.vfe_next = VFE_FS[2];
        } else {
            o.vls = SpeedsLookupTables::approach_vls((f_pos + 1) as u8, cg, m);
            o.vmax = VFE_FS[(f_pos + 1) as usize];
            o.vfe_next = if f_pos == 4 { 0.0 } else { VFE_FS[(f_pos + 2) as usize] };
        }
        o.vapp = o.vls + add_wind_component(js_round(i.wind_kt / 3.0));
        o.vref = SpeedsLookupTables::approach_vls(ApproachConf::ConfFull as u8, cg, m);
        o.gd = SpeedsLookupTables::green_dot(alt, m);

        o.vs1g = o.vls / 1.23;
        o.vls = o.vls.max(VMCL);
        if i.phase <= FlightPhase::Takeoff {
            o.vls = (1.15 * o.vs1g).max(1.05 * i.v2_kt.min(VMCL));
        } else if f_pos == 1 && i.cas_kt > 212.0 {
            o.vls = (1.18 * o.vs1g).max(VMCL);
        }

        let spoilers = i.left_spoiler_1_position;
        let max_spoiler_extension = [20.0, 20.0, 12.0, 9.0, 8.0, 6.0];
        let spoiler_vls_increase = [25.0, 25.0, 7.0, 10.0, 10.0, 8.0];
        if spoilers > 0.0 && !i.ignore_spoilers {
            let conf = match f_pos {
                1 => {
                    if i.cas_kt > 212.0 {
                        1
                    } else {
                        2
                    }
                }
                0 => 0,
                p => (p + 1) as usize,
            };
            let conf = conf as usize;
            o.vls += spoiler_vls_increase[conf] * (spoilers / max_spoiler_extension[conf]);
        }

        let vs1g_conf0 = SpeedsLookupTables::vls_conf0(alt, m) / 1.23;
        let vs1g_conf1f = SpeedsLookupTables::approach_vls(ApproachConf::Conf1 as u8, cg, m) / 1.23;
        let early = i.phase <= FlightPhase::Takeoff;
        o.f2 = if early {
            (1.18 * vs1g_conf1f).max(VMCL + 5.0)
        } else {
            SpeedsLookupTables::f2_speed(cg, m)
        };
        o.f3 = if early {
            (1.18 * vs1g_conf1f).max(VMCL + 5.0)
        } else {
            SpeedsLookupTables::f3_speed(cg, m)
        };
        o.s = if early {
            1.21 * vs1g_conf0
        } else {
            SpeedsLookupTables::s_speed(m)
        };
        o
    }
}

pub struct SpeedsUtils;

impl SpeedsUtils {
    pub fn headwind(wind_speed: f64, wind_direction: f64, runway_bearing: f64) -> f64 {
        wind_speed * (diff_angle(wind_direction, runway_bearing) * (std::f64::consts::PI / 180.0)).cos()
    }

    pub fn headwind_diff(tower_headwind: f64, current_headwind: f64) -> f64 {
        js_round((1.0 / 3.0) * (current_headwind - tower_headwind))
    }

    pub fn vtarget_gs_mini(vapp: f64, wind_diff: f64, flaps_handle_index: i32, vmax: f64, vfe_next: f64) -> f64 {
        let limit = if flaps_handle_index == 4 { vmax - 5.0 } else { vfe_next };
        vapp.max(js_round(vapp + wind_diff).min(js_round(limit)))
    }

    pub fn vmca(altitude_ft: f64) -> f64 {
        interp_table(VMCA, altitude_ft)
    }

    pub fn vmcg(altitude_ft: f64) -> f64 {
        interp_table(VMCG, altitude_ft)
    }

    pub fn vs1g(mass_kg: f64, cg: f64, conf: u8, takeoff: bool) -> f64 {
        if takeoff {
            return SpeedsLookupTables::approach_vls(conf, cg, mass_kg) / 1.15;
        }
        if conf == 5 {
            return (SpeedsLookupTables::approach_vls(conf, cg, mass_kg) / 1.18).max(VMCL);
        }
        (SpeedsLookupTables::approach_vls(conf, cg, mass_kg) / 1.23).max(VMCL)
    }
}

fn interp_table(table: &[(f64, f64)], alt: f64) -> f64 {
    if alt <= table[0].0 {
        return VMCA[0].1;
    }
    interp_pairs(table, alt)
}

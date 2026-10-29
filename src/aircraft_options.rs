use systems::simulation::{SimulatorReaderWriter, VariableIdentifier, VariableRegistry};

use crate::xp::{DataRef, Xplm};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WeightVariant {
    pub name: &'static str,
    pub mrw_kg: f64,
    pub mtow_kg: f64,
    pub mlw_kg: f64,
    pub mzfw_kg: f64,
}

const fn wv(name: &'static str, mrw: f64, mtow: f64, mlw: f64, mzfw: f64) -> WeightVariant {
    WeightVariant { name, mrw_kg: mrw * 1000.0, mtow_kg: mtow * 1000.0, mlw_kg: mlw * 1000.0, mzfw_kg: mzfw * 1000.0 }
}

pub const WEIGHT_VARIANTS: [WeightVariant; 15] = [
    wv("WV000", 562.0, 560.0, 386.0, 361.0),
    wv("WV001", 512.0, 510.0, 394.0, 372.0),
    wv("WV002", 571.0, 569.0, 391.0, 366.0),
    wv("WV003", 512.0, 510.0, 395.0, 373.0),
    wv("WV004", 562.0, 560.0, 391.0, 366.0),
    wv("WV005", 562.0, 560.0, 386.0, 366.0),
    wv("WV006", 575.0, 573.0, 393.0, 368.0),
    wv("WV007", 492.0, 490.0, 395.0, 373.0),
    wv("WV008", 577.0, 575.0, 394.0, 369.0),
    wv("WV009", 512.0, 510.0, 386.0, 361.0),
    wv("WV010", 482.0, 480.0, 386.0, 361.0),
    wv("WV011", 577.0, 575.0, 395.0, 369.0),
    wv("WV012", 571.0, 569.0, 395.0, 366.0),
    wv("WV013", 494.0, 492.0, 386.0, 361.0),
    wv("WV014", 574.0, 572.0, 391.0, 366.0),
];

pub const DEFAULT_WEIGHT_VARIANT: u8 = 3;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EngineRating {
    pub name: &'static str,
    pub takeoff_kn: f64,
    pub takeoff_n1_scale: f64,
    pub fuel_flow_factor: f64,
    pub gp: bool,
    pub flat_shift_c: f64,
}

pub const ENGINE_RATINGS: [EngineRating; 11] = [
    EngineRating { name: "970", takeoff_kn: 334.29, takeoff_n1_scale: 0.9692, fuel_flow_factor: 1.0, gp: false, flat_shift_c: 0.0 },
    EngineRating { name: "972", takeoff_kn: 341.41, takeoff_n1_scale: 0.9790, fuel_flow_factor: 1.0, gp: false, flat_shift_c: 0.0 },
    EngineRating { name: "972E", takeoff_kn: 341.41, takeoff_n1_scale: 0.9790, fuel_flow_factor: 0.99, gp: false, flat_shift_c: 0.0 },
    EngineRating { name: "970B", takeoff_kn: 348.31, takeoff_n1_scale: 0.9885, fuel_flow_factor: 1.0, gp: false, flat_shift_c: 0.0 },
    EngineRating { name: "972B", takeoff_kn: 356.81, takeoff_n1_scale: 1.0, fuel_flow_factor: 1.0, gp: false, flat_shift_c: 0.0 },
    EngineRating { name: "977", takeoff_kn: 359.33, takeoff_n1_scale: 1.0034, fuel_flow_factor: 1.0, gp: false, flat_shift_c: 0.0 },
    EngineRating { name: "977B", takeoff_kn: 372.92, takeoff_n1_scale: 1.0212, fuel_flow_factor: 1.0, gp: false, flat_shift_c: 0.0 },
    EngineRating { name: "980", takeoff_kn: 374.09, takeoff_n1_scale: 1.0228, fuel_flow_factor: 1.0, gp: false, flat_shift_c: 0.0 },
    EngineRating { name: "GP7270", takeoff_kn: 332.44, takeoff_n1_scale: 0.9667, fuel_flow_factor: 1.0, gp: true, flat_shift_c: 0.0 },
    EngineRating { name: "GP7270E", takeoff_kn: 332.44, takeoff_n1_scale: 0.9667, fuel_flow_factor: 1.0, gp: true, flat_shift_c: 6.0 },
    EngineRating { name: "GP7277", takeoff_kn: 357.15, takeoff_n1_scale: 1.0005, fuel_flow_factor: 1.0, gp: true, flat_shift_c: 0.0 },
];

pub const DEFAULT_ENGINE_RATING: u8 = 4;

pub fn derate_n1_scale(percent: f64) -> f64 {
    (1.0 - percent.clamp(0.0, 40.0) / 100.0).powf(1.0 / 2.04)
}

pub fn takeoff_derate_scale(power_setting: f64, level: f64, phase: f64) -> f64 {
    let s = crate::app_settings::current();
    if !s.derates_enabled || power_setting.round() != 2.0 || phase > 1.5 {
        return 1.0;
    }
    derate_n1_scale(s.takeoff_derates[(level.round().max(0.0) as usize).min(4)])
}

pub fn climb_derate_scale(level: f64, phase: f64) -> f64 {
    let s = crate::app_settings::current();
    let k = level.round();
    if !s.derates_enabled || !(1.0..=5.0).contains(&k) || !(1.5..2.5).contains(&phase) {
        return 1.0;
    }
    derate_n1_scale(s.climb_derates[k as usize - 1])
}

const MFD: &str = "/Pages/VCockpit/Instruments/A380X/MFD/mfd.js";

fn mirror(cell: &str, initial: &str, var: &str) -> crate::source_patch::SourcePatch {
    crate::source_patch::SourcePatch {
        path: MFD.to_string(),
        find: format!("{cell}=Signal.of({initial})"),
        replace: format!("{cell}=(c=>(c.watch(v=>Vars.write(\"L:{var}\",\"number\",v==null?0:v),true),c))(Signal.of({initial}))"),
        reason: format!("the FMS {cell} reaches the FADEC as {var} (derates)"),
    }
}

pub fn source_patches() -> Vec<crate::source_patch::SourcePatch> {
    let gp = &crate::physics::engine::engine_data::GP7200;
    let trent = &crate::physics::engine::engine_data::TRENT_900;
    vec![
        mirror("takeoffPowerSetting", "0", "A32NX_FM_TO_POWER_SETTING"),
        mirror("takeoffDeratedSetting", "0", "A32NX_FM_TO_DERATE_LEVEL"),
        mirror("climbDerated", "0", "A32NX_FM_CLB_DERATE_LEVEL"),
        sd_ewd("/Pages/VCockpit/Instruments/A380X/EWD/ewd.js", "[\"egt\",{name:\"L:A32NX_ENGINE_EGT:#index#\"", "[\"egt\",{name:\"L:A32NX_ENGINE_EGT_DISPLAY:#index#\"", "EGT on the EWD on the selected engine's scale (GP7200)"),
        sd_ewd("/Pages/VCockpit/Instruments/A380X/EWD/ewd.js", "this.min=0;this.max=1e3;this.warningEGTColor", "this.min=0;this.max=(Vars.read(\"L:A32NX_A380X_ENGINE_TYPE\",\"number\")===1?1100:1e3);this.warningEGTColor", "GP7200 EGT scale"),
        sd_ewd("/Pages/VCockpit/Instruments/A380X/EWD/ewd.js", "if(EGTemperature>=900)", &format!("if(EGTemperature>=(Vars.read(\"L:A32NX_A380X_ENGINE_TYPE\",\"number\")===1?{}:{}))", gp.egt_takeoff_c, trent.egt_takeoff_c), "GP7200 EGT red line 1002 C (EASA TCDS IM.E.026)"),
        sd_ewd("/Pages/VCockpit/Instruments/A380X/EWD/ewd.js", "if(EGTemperature>850&&throttleMode<3)", &format!("if(EGTemperature>(Vars.read(\"L:A32NX_A380X_ENGINE_TYPE\",\"number\")===1?{}:{})&&throttleMode<3)", gp.egt_max_continuous_c, trent.egt_max_continuous_c), "GP7200 max continuous EGT 970 C"),
        sd_ewd("/Pages/VCockpit/Instruments/A380X/EWD/ewd.js", "GaugeMaxEGTComponent,{value:Signal.of(850)", &format!("GaugeMaxEGTComponent,{{value:Signal.of(Vars.read(\"L:A32NX_A380X_ENGINE_TYPE\",\"number\")===1?{}:{})", gp.egt_max_continuous_c, trent.egt_max_continuous_c), "GP7200 max EGT marker"),
        sd_ewd("/Pages/VCockpit/Instruments/A380X/SD/sd.js", "value:N2,active:fadecPowered}", "value:(Vars.read(\"L:A32NX_A380X_ENGINE_TYPE\",\"number\")===1?N3:N2),active:fadecPowered}", "GP7200 is two-spool: its N2 is the HP spool"),
        sd_ewd("/Pages/VCockpit/Instruments/A380X/SD/sd.js", "className:`LightGreyBox ${starting?\"Show\":\"Hide\"}`", "className:`LightGreyBox ${starting&&!(Vars.read(\"L:A32NX_A380X_ENGINE_TYPE\",\"number\")===1)?\"Show\":\"Hide\"}`", "GP7200 has no IP spool"),
        sd_ewd("/Pages/VCockpit/Instruments/A380X/SD/sd.js", "import_react21.default.createElement(DecimalValues_default,{x,y:y+38,value:N3,active:fadecPowered}),import_react21.default.createElement(\"path\",{className:\"White SW2\",d:`M${engine>2?x-96:x+64},${y+28} l 26, 0`})", "!(Vars.read(\"L:A32NX_A380X_ENGINE_TYPE\",\"number\")===1)&&import_react21.default.createElement(DecimalValues_default,{x,y:y+38,value:N3,active:fadecPowered}),!(Vars.read(\"L:A32NX_A380X_ENGINE_TYPE\",\"number\")===1)&&import_react21.default.createElement(\"path\",{className:\"White SW2\",d:`M${engine>2?x-96:x+64},${y+28} l 26, 0`})", "GP7200 has no N3"),
        sd_ewd("/Pages/VCockpit/Instruments/A380X/SD/sd.js", "import_react21.default.createElement(DecimalValues_default,{x,y:y+450,value:n3Vibration,active:fadecPowered,shift:-14}),import_react21.default.createElement(\"path\",{className:\"White SW2\",d:`M${engine>2?x-96:x+64},${y+440} l 26, 0`})", "!(Vars.read(\"L:A32NX_A380X_ENGINE_TYPE\",\"number\")===1)&&import_react21.default.createElement(DecimalValues_default,{x,y:y+450,value:n3Vibration,active:fadecPowered,shift:-14}),!(Vars.read(\"L:A32NX_A380X_ENGINE_TYPE\",\"number\")===1)&&import_react21.default.createElement(\"path\",{className:\"White SW2\",d:`M${engine>2?x-96:x+64},${y+440} l 26, 0`})", "GP7200 has no N3 vibration"),
        sd_ewd("/Pages/VCockpit/Instruments/A380X/SD/sd.js", "import_react22.default.createElement(\"text\",{x:388,y:108,className:\"F25 EndAlign White\"},\"N3\"),import_react22.default.createElement(\"text\",{x:414,y:108,className:\"F25 EndAlign Cyan\"},\"%\")", "!(Vars.read(\"L:A32NX_A380X_ENGINE_TYPE\",\"number\")===1)&&import_react22.default.createElement(\"text\",{x:388,y:108,className:\"F25 EndAlign White\"},\"N3\"),!(Vars.read(\"L:A32NX_A380X_ENGINE_TYPE\",\"number\")===1)&&import_react22.default.createElement(\"text\",{x:414,y:108,className:\"F25 EndAlign Cyan\"},\"%\")", "GP7200 has no N3"),
        sd_ewd("/Pages/VCockpit/Instruments/A380X/SD/sd.js", "import_react22.default.createElement(\"text\",{x:428,y:520,className:\"F25 EndAlign White\"},\"N3\")", "!(Vars.read(\"L:A32NX_A380X_ENGINE_TYPE\",\"number\")===1)&&import_react22.default.createElement(\"text\",{x:428,y:520,className:\"F25 EndAlign White\"},\"N3\")", "GP7200 has no N3 vibration"),
    ]
}

pub fn engine_label(r: &EngineRating) -> String {
    if r.gp { r.name.to_string() } else { format!("Trent {}-84", r.name) }
}

pub fn display_egt(egt: f64) -> f64 {
    crate::physics::engine::engine_data::EngineData::current().display_egt_c(egt)
}

fn sd_ewd(path: &str, find: &str, replace: &str, reason: &str) -> crate::source_patch::SourcePatch {
    crate::source_patch::SourcePatch { path: path.to_string(), find: find.to_string(), replace: replace.to_string(), reason: reason.to_string() }
}

pub fn weight_variant() -> WeightVariant {
    let k = crate::app_settings::current().weight_variant as usize;
    WEIGHT_VARIANTS[k.min(WEIGHT_VARIANTS.len() - 1)]
}

pub fn engine_rating() -> EngineRating {
    let k = crate::app_settings::current().engine_rating as usize;
    ENGINE_RATINGS[k.min(ENGINE_RATINGS.len() - 1)]
}

pub fn weight_variant_index(text: &str) -> Option<u8> {
    let t = text.trim().to_ascii_uppercase();
    WEIGHT_VARIANTS.iter().position(|w| w.name == t).map(|k| k as u8)
}

pub fn rating_for_type(engine_type: Option<&str>, rating: u8) -> u8 {
    let Some(t) = engine_type else { return rating };
    let gp = t.trim().eq_ignore_ascii_case("GP7200");
    if ENGINE_RATINGS.get(rating as usize).is_some_and(|r| r.gp == gp) {
        return rating;
    }
    let default = if gp { "GP7270" } else { "972B" };
    ENGINE_RATINGS.iter().position(|r| r.name == default).map_or(rating, |k| k as u8)
}

pub fn engine_rating_index(text: &str) -> Option<u8> {
    let t = text.trim().to_ascii_uppercase().replace("TRENT", "").replace("-84", "").replace(' ', "").replace("GP-", "GP");
    ENGINE_RATINGS.iter().position(|r| r.name == t).map(|k| k as u8)
}

pub struct AircraftOptions {
    mrw: VariableIdentifier,
    mtow: VariableIdentifier,
    mlw: VariableIdentifier,
    mzfw: VariableIdentifier,
    wv: VariableIdentifier,
    rating_kn: VariableIdentifier,
    engine_type: VariableIdentifier,
    _published: crate::published::Published,
    gp_model: crate::published::Value,
    max_weight: Option<DataRef>,
    applied: Option<(WeightVariant, EngineRating)>,
}

impl AircraftOptions {
    pub fn new<V: VariableRegistry>(vars: &mut V, xplm: &Xplm) -> Self {
        let mut published = crate::published::Published::default();
        Self {
            mrw: vars.get("A380X_WV_MRW_KG".into()),
            mtow: vars.get("A380X_WV_MTOW_KG".into()),
            mlw: vars.get("A380X_WV_MLW_KG".into()),
            mzfw: vars.get("A380X_WV_MZFW_KG".into()),
            wv: vars.get("A380X_WV_INDEX".into()),
            rating_kn: vars.get("A380X_ENGINE_RATING_TO_KN".into()),
            engine_type: vars.get("A380X_ENGINE_TYPE".into()),
            gp_model: published.number("fbw/engine/gp7200", 0., false),
            _published: published,
            max_weight: xplm.find("sim/aircraft/weight/acf_m_max"),
            applied: None,
        }
    }

    pub fn update<V: SimulatorReaderWriter>(&mut self, vars: &mut V, xplm: &Xplm) {
        let w = weight_variant();
        let r = engine_rating();
        vars.write(&self.mrw, w.mrw_kg);
        vars.write(&self.mtow, w.mtow_kg);
        vars.write(&self.mlw, w.mlw_kg);
        vars.write(&self.mzfw, w.mzfw_kg);
        vars.write(&self.wv, WEIGHT_VARIANTS.iter().position(|x| x.name == w.name).unwrap_or(0) as f64);
        vars.write(&self.rating_kn, r.takeoff_kn);
        vars.write(&self.engine_type, if r.gp { 1. } else { 0. });
        crate::published::set(self.gp_model, if r.gp { 1. } else { 0. });
        if self.applied != Some((w, r)) {
            if let Some(d) = self.max_weight {
                xplm.set_f(d, w.mtow_kg as f32);
            }
            crate::log(&format!("aircraft options: {} (MTOW {:.0} t, MLW {:.0} t, MZFW {:.0} t), {} {:.2} kN", w.name, w.mtow_kg / 1000.0, w.mlw_kg / 1000.0, w.mzfw_kg / 1000.0, engine_label(&r), r.takeoff_kn));
            self.applied = Some((w, r));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_variant_is_internally_consistent() {
        for w in WEIGHT_VARIANTS {
            assert!(w.mrw_kg - w.mtow_kg == 2000.0, "{}: ramp is take-off plus 2 t taxi", w.name);
            assert!(w.mlw_kg < w.mtow_kg && w.mzfw_kg < w.mlw_kg, "{}", w.name);
        }
    }

    #[test]
    fn the_default_variant_is_the_one_the_port_was_built_on() {
        let w = WEIGHT_VARIANTS[DEFAULT_WEIGHT_VARIANT as usize];
        assert_eq!((w.name, w.mtow_kg, w.mlw_kg, w.mzfw_kg), ("WV003", 510_000.0, 395_000.0, 373_000.0));
        assert_eq!(ENGINE_RATINGS[DEFAULT_ENGINE_RATING as usize].name, "972B");
    }

    #[test]
    fn names_parse_the_way_the_efb_and_users_write_them() {
        assert_eq!(weight_variant_index("wv008"), Some(8));
        assert_eq!(weight_variant_index("WV015"), None);
        assert_eq!(engine_rating_index("Trent 970B-84"), Some(3));
        assert_eq!(engine_rating_index("972"), Some(1));
        assert_eq!(engine_rating_index("977B"), Some(6));
        assert_eq!(engine_rating_index("985"), None);
        assert_eq!(engine_rating_index("GP7277"), Some(10));
        assert_eq!(engine_rating_index("gp7270e"), Some(9));
    }

    #[test]
    fn the_engine_type_wins_over_a_rating_of_the_other_engine() {
        let gp7270 = engine_rating_index("GP7270").unwrap();
        let t972b = engine_rating_index("972B").unwrap();
        assert_eq!(rating_for_type(Some("TRENT900"), gp7270), t972b);
        assert_eq!(rating_for_type(Some("GP7200"), t972b), gp7270);
        assert_eq!(rating_for_type(Some("GP7200"), engine_rating_index("GP7277").unwrap()), engine_rating_index("GP7277").unwrap());
        assert_eq!(rating_for_type(Some("TRENT900"), engine_rating_index("980").unwrap()), engine_rating_index("980").unwrap());
        assert_eq!(rating_for_type(None, gp7270), gp7270);
    }

    #[test]
    fn each_rating_scale_gives_its_certified_thrust_on_the_engine_model() {
        use crate::physics::engine::{Engine, EngineInputs};
        let thrust_at = |n1: f64| {
            let mut e = Engine::new();
            let inputs = EngineInputs { target_n1_corrected_pct: n1, ..crate::physics::engine::tests::isa_sea_level() };
            crate::physics::engine::tests::run_to_steady_state(&mut e, inputs, 40.0).net_thrust_n
        };
        let to_n1 = crate::physics::engine::params::TAKEOFF_N1_PCT;
        let base = thrust_at(to_n1);
        let top = ENGINE_RATINGS[DEFAULT_ENGINE_RATING as usize].takeoff_kn;
        for r in ENGINE_RATINGS {
            let ratio = thrust_at(to_n1 * r.takeoff_n1_scale) / base;
            let want = r.takeoff_kn / top;
            assert!((ratio - want).abs() < 0.005, "{}: {ratio:.4} vs {want:.4}", r.name);
        }
    }

    #[test]
    fn gp7200_egt_maps_the_trent_red_line_onto_its_own() {
        let scale = |egt: f64| crate::physics::engine::engine_data::GP7200.display_egt_c(egt);
        assert!((scale(900.0) - 1002.0).abs() < 1e-9);
        assert!((scale(15.0) - 15.0).abs() < 1e-9);
    }

    #[test]
    fn every_patch_anchor_is_unique_in_the_installed_bundles() {
        let base = std::path::Path::new("D:/Steam Games/steamapps/common/X-Plane 12/Aircraft/A380 for X-Plane 12/html_ui");
        if !base.is_dir() {
            return;
        }
        for p in source_patches() {
            let text = std::fs::read_to_string(base.join(p.path.trim_start_matches('/'))).unwrap();
            assert_eq!(crate::source_patch::count(&text, &p.find), 1, "{}: {}", p.path, p.find);
        }
    }
}

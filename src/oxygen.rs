//! Oxygen bridge. The one oxygen model is `deep::oxygen` (crew bottle per
//! FCOM DSC-35-20-10, cabin system, first-aid bottle, faults, persistence).
//! This module only:
//! - posts the cockpit inputs to it (crew mask on, MASK MAN ON, CREW SUPPLY);
//! - republishes its outputs under the older `OXYGEN_*` names;
//! - drives X-Plane's hypoxia effect from the CPCS cabin altitude (W209);
//! - forwards servicing requests (Study panel, GSE, aircraft reset).
//!
//! Cabin visual: `OXYGEN_PAX_MASKS_DEPLOYED` (0/1) and
//! `DEEP_OXY_PAX_{MAIN,UPPER}_DECK_PRESENTED_FRACTION` are the variables a
//! dropped-mask cabin OBJ animates on.

use systems::simulation::{SimulatorReaderWriter, VariableIdentifier, VariableRegistry};

use crate::deep::oxygen::live as deep;
use crate::physics::gas;
use crate::xp::{DataRef, Xplm};
use crate::Vars;

/// Density of oxygen at 0 C and 1 atm, kg/L.
const O2_KG_PER_L: f64 = 1.429e-3;

/// (W209) Felt altitude for X-Plane's hypoxia effect: the cabin altitude,
/// reduced by the diluter-demand protection when a mask is donned.
fn felt_altitude_ft(cabin_altitude_ft: f64, masks_on: bool) -> f64 {
    if masks_on {
        cabin_altitude_ft * (1. - gas::diluter_demand_protection_fraction(cabin_altitude_ft))
    } else {
        cabin_altitude_ft
    }
}

/// `PRESS_CABIN_ALTITUDE_B1` is an ARINC 429 word (FBW `to_arinc429`); only
/// a word in normal operation carries a cabin altitude.
fn cabin_altitude_ft(packed: f64) -> Option<f64> {
    const SSM_NORMAL_OPERATION: u64 = 3;
    let bits = packed as u64;
    ((bits >> 32) & 0b11 == SSM_NORMAL_OPERATION).then(|| f32::from_bits(bits as u32) as f64)
}

struct Ids {
    crew_mask_on: VariableIdentifier,
    /// FBW A380 cockpit behaviour: MASK MAN ON guarded pb.
    mask_man_on: VariableIdentifier,
    /// FBW A380 cockpit behaviour: CREW SUPPLY pb-sw, 1 = OFF (as A32NX).
    crew_supply_off: VariableIdentifier,
    crew_quantity_percent: VariableIdentifier,
    crew_pressure_psi: VariableIdentifier,
    crew_low_pressure: VariableIdentifier,
    crew_flow_kg_s: VariableIdentifier,
    crew_dilution_fraction: VariableIdentifier,
    pax_quantity_percent: VariableIdentifier,
    pax_masks_deployed: VariableIdentifier,
    pax_flow_lpm: VariableIdentifier,
    cabin_altitude_ft: VariableIdentifier,
    deep_crew_fraction: VariableIdentifier,
    deep_crew_psi: VariableIdentifier,
    deep_crew_low: VariableIdentifier,
    deep_crew_flow: VariableIdentifier,
    deep_crew_mass: VariableIdentifier,
    deep_crew_delivered: VariableIdentifier,
    deep_pax_remaining: VariableIdentifier,
    deep_pax_deployed: VariableIdentifier,
    deep_pax_flow: VariableIdentifier,
}

impl Ids {
    fn new(vars: &mut impl VariableRegistry) -> Self {
        Self {
            crew_mask_on: vars.get("OXYGEN CREW MASK ON".into()),
            mask_man_on: vars.get("A32NX_OXYGEN_MASKS_DEPLOYED".into()),
            crew_supply_off: vars.get("PUSH_OVHD_OXYGEN_CREW".into()),
            crew_quantity_percent: vars.get("OXYGEN_CREW_QUANTITY_PERCENT".into()),
            crew_pressure_psi: vars.get("OXYGEN_CREW_PRESSURE_PSI".into()),
            crew_low_pressure: vars.get("OXYGEN_CREW_LOW_PRESSURE".into()),
            crew_flow_kg_s: vars.get("OXYGEN_CREW_FLOW_KG_S".into()),
            crew_dilution_fraction: vars.get("OXYGEN_CREW_DILUTION_FRACTION".into()),
            pax_quantity_percent: vars.get("OXYGEN_PAX_QUANTITY_PERCENT".into()),
            pax_masks_deployed: vars.get("OXYGEN_PAX_MASKS_DEPLOYED".into()),
            pax_flow_lpm: vars.get("OXYGEN_PAX_FLOW_LPM".into()),
            // a380_systems air_conditioning/cpiom_b.rs:861.
            cabin_altitude_ft: vars.get("PRESS_CABIN_ALTITUDE_B1".into()),
            deep_crew_fraction: vars.get("DEEP_OXY_CREW_QUANTITY_FRACTION".into()),
            deep_crew_psi: vars.get("DEEP_OXY_CREW_BOTTLE_CORRECTED_PSI".into()),
            deep_crew_low: vars.get("DEEP_OXY_CREW_LOW_PRESSURE".into()),
            deep_crew_flow: vars.get("DEEP_OXY_CREW_TOTAL_FLOW_KG_S".into()),
            deep_crew_mass: vars.get("DEEP_OXY_CREW_MASS_KG".into()),
            deep_crew_delivered: vars.get("DEEP_OXY_CREW_DELIVERED_O2_FRACTION:1".into()),
            deep_pax_remaining: vars.get("DEEP_OXY_PAX_SUPPLY_REMAINING_FRACTION".into()),
            deep_pax_deployed: vars.get("DEEP_OXY_PAX_MASKS_DEPLOYED".into()),
            deep_pax_flow: vars.get("DEEP_OXY_PAX_TOTAL_FLOW_KG_S".into()),
        }
    }
}

struct Refs {
    override_oxygen_system: Option<DataRef>,
    pilot_felt_altitude_ft: Option<DataRef>,
}

/// Servicing asked for from the Study panel or the oxygen GSE.
pub fn request_service() {
    deep::request_service();
}

pub struct Oxygen {
    ids: Ids,
    refs: Refs,
    last: (f64, f64, bool),
}

impl Oxygen {
    /// `(crew bottle kg, pax percent, pax deployed)`, as last published by
    /// `deep::oxygen`. The deep area's own saved state is what restores.
    pub fn saved_state(&self) -> (f64, f64, bool) {
        self.last
    }

    pub fn restore_state(&mut self, state: (f64, f64, bool)) {
        self.last = state;
    }

    pub fn new(vars: &mut Vars, xplm: &Xplm) -> Self {
        let refs = Refs {
            override_oxygen_system: xplm.find("sim/operation/override/override_oxygen_system"),
            pilot_felt_altitude_ft: xplm.find("sim/cockpit2/oxygen/indicators/pilot_felt_altitude_ft"),
        };
        Self { ids: Ids::new(vars), refs, last: (0., 100., false) }
    }

    pub fn update<W: SimulatorReaderWriter>(&mut self, vars: &mut W, xplm: &Xplm, _delta: f64) {
        self.step(vars);
        self.mirror_hypoxia_trigger(vars, xplm);
    }

    fn mirror_hypoxia_trigger<W: SimulatorReaderWriter>(&self, vars: &mut W, xplm: &Xplm) {
        let Some(felt) = self.refs.pilot_felt_altitude_ft else { return };
        let Some(cabin_alt) = cabin_altitude_ft(vars.read(&self.ids.cabin_altitude_ft)) else {
            if let Some(d) = self.refs.override_oxygen_system {
                xplm.set_i(d, 0);
            }
            return;
        };
        if let Some(d) = self.refs.override_oxygen_system {
            xplm.set_i(d, 1);
        }
        let delivered = vars.read(&self.ids.deep_crew_delivered);
        let masks_on = vars.read(&self.ids.crew_mask_on) != 0. && delivered > crate::deep::oxygen::gas::AIR_O2_MOLE_FRACTION;
        xplm.set_f(felt, felt_altitude_ft(cabin_alt, masks_on) as f32);
    }

    /// Hands X-Plane's own hypoxia/oxygen system back.
    pub fn release(&self, xplm: &Xplm) {
        if let Some(d) = self.refs.override_oxygen_system {
            xplm.set_i(d, 0);
        }
    }

    fn step<W: SimulatorReaderWriter>(&mut self, vars: &mut W) {
        let masks_on = vars.read(&self.ids.crew_mask_on) != 0.;
        deep::post_commands(deep::Commands {
            masks_donned: [masks_on, masks_on, false, false],
            manual_deploy: vars.read(&self.ids.mask_man_on) != 0.,
            crew_supply_off: vars.read(&self.ids.crew_supply_off) != 0.,
            ..Default::default()
        });

        let crew_fraction = vars.read(&self.ids.deep_crew_fraction);
        let pax_remaining = vars.read(&self.ids.deep_pax_remaining);
        let deployed = vars.read(&self.ids.deep_pax_deployed) != 0.;
        vars.write(&self.ids.crew_quantity_percent, 100. * crew_fraction);
        let v = vars.read(&self.ids.deep_crew_psi);
        vars.write(&self.ids.crew_pressure_psi, v);
        let v = vars.read(&self.ids.deep_crew_low);
        vars.write(&self.ids.crew_low_pressure, v);
        let v = vars.read(&self.ids.deep_crew_flow);
        vars.write(&self.ids.crew_flow_kg_s, v);
        let v = vars.read(&self.ids.deep_crew_delivered);
        vars.write(&self.ids.crew_dilution_fraction, v);
        vars.write(&self.ids.pax_quantity_percent, 100. * pax_remaining);
        vars.write(&self.ids.pax_masks_deployed, deployed as i32 as f64);
        let v = vars.read(&self.ids.deep_pax_flow) * 60. / O2_KG_PER_L;
        vars.write(&self.ids.pax_flow_lpm, v);
        self.last = (vars.read(&self.ids.deep_crew_mass), 100. * pax_remaining, deployed);
    }

    /// Ground servicing: recharge the bottles, fit new generators, stow the
    /// masks.
    pub fn service(&mut self) {
        deep::request_service();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aspects::test_vars::TestVars;

    fn packed(ft: f64) -> f64 {
        ((3u64 << 32) | (ft as f32).to_bits() as u64) as f64
    }

    #[test]
    fn the_cabin_altitude_word_is_decoded_and_only_trusted_in_normal_operation() {
        let live = cabin_altitude_ft(14_046_068_736.).expect("normal-operation word");
        assert!((2_800. ..3_000.).contains(&live), "decoded {live}");
        assert_eq!(cabin_altitude_ft(packed(8000.)), Some(8000.));
        assert_eq!(cabin_altitude_ft((8000_f32).to_bits() as f64), None);
        assert_eq!(cabin_altitude_ft(((1u64 << 32) | (8000_f32).to_bits() as u64) as f64), None);
    }

    #[test]
    fn the_cabin_altitude_variable_this_module_reads_is_one_the_systems_register() {
        #[derive(Default)]
        struct Names(Vec<String>);
        impl VariableRegistry for Names {
            fn get(&mut self, name: String) -> VariableIdentifier {
                self.0.push(name);
                VariableIdentifier::new(0usize)
            }
            fn get_unprefixed(&mut self, name: String) -> VariableIdentifier {
                self.0.push(name);
                VariableIdentifier::new(0usize)
            }
        }
        let mut names = Names::default();
        let _ = systems::simulation::Simulation::new(systems::simulation::StartState::Apron, a380_systems::A380::new, &mut names);
        assert!(names.0.iter().any(|n| n == "PRESS_CABIN_ALTITUDE_B1"));
    }

    #[test]
    fn deep_outputs_are_republished_under_the_old_names() {
        let mut vars = TestVars::default();
        let mut o = Oxygen { ids: Ids::new(&mut vars), refs: Refs { override_oxygen_system: None, pilot_felt_altitude_ft: None }, last: (0., 100., false) };
        vars.write(&o.ids.deep_crew_fraction, 0.5);
        vars.write(&o.ids.deep_crew_psi, 925.);
        vars.write(&o.ids.deep_pax_remaining, 0.25);
        vars.write(&o.ids.deep_pax_deployed, 1.);
        vars.write(&o.ids.deep_pax_flow, O2_KG_PER_L);
        o.step(&mut vars);
        assert_eq!(vars.read(&o.ids.crew_quantity_percent), 50.);
        assert_eq!(vars.read(&o.ids.crew_pressure_psi), 925.);
        assert_eq!(vars.read(&o.ids.pax_quantity_percent), 25.);
        assert_eq!(vars.read(&o.ids.pax_masks_deployed), 1.);
        assert!((vars.read(&o.ids.pax_flow_lpm) - 60.).abs() < 1e-9);
        assert_eq!(o.saved_state().2, true);
    }

    #[test]
    fn felt_altitude_tracks_real_cabin_altitude_with_no_mask_donned() {
        assert_eq!(felt_altitude_ft(35_000., false), 35_000.);
        assert_eq!(felt_altitude_ft(8_000., false), 8_000.);
    }

    #[test]
    fn felt_altitude_drops_toward_sea_level_once_masked_at_full_protection_altitude() {
        assert!(felt_altitude_ft(34_000., true) < 1.);
    }

    #[test]
    fn felt_altitude_is_unchanged_by_masking_at_sea_level() {
        assert_eq!(felt_altitude_ft(0., true), 0.);
    }
}

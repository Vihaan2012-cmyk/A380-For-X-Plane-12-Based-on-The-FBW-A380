//! Registers every hydraulics component, failure and ECAM alert through
//! `crate::deep::api`'s `Registry` (see `docs/deep/BRIEF.md`, "Registering
//! failures, components and ECAM alerts"). One failure per genuinely
//! distinct physical fault this directory's models support, expanded per
//! instance (per pump, per circuit) via the loops below rather than
//! hand-duplicated blocks.
//!
//! Health-parameter vs. failure split: a pump's `wear` (`pump::PumpFaults`)
//! is a slowly accumulated, persisted health parameter (a `ComponentDef`
//! `ParamDef`), not an instructor-triggered event, so it is registered as a
//! component parameter; every other fault field in this directory's
//! `...Faults` structs (displacement loss, seizure, stuck valves, precharge
//! loss, leaks, clog) is an instructor-settable 0..1 fault, so each becomes
//! its own `FailureDef`, one per instance.
//!
//! New simulator variables this directory's models need published (none
//! exist in the plugin yet -- noted here and in `PROGRESS.md` for whoever
//! wires the glue): `HYD_{GREEN,YELLOW}_MANIFOLD_PRESSURE_PSI`,
//! `HYD_{GREEN,YELLOW}_RESERVOIR_LEVEL_IS_LOW`,
//! `HYD_{GREEN,YELLOW}_RESERVOIR_AIR_PRESSURE_IS_LOW`,
//! `HYD_{GREEN,YELLOW}_RESERVOIR_OVHT` (these four names match FlyByWire's
//! own `Reservoir::new` identifier convention,
//! `fbw-common/hydraulic/mod.rs` lines 2300-2306, reused verbatim rather
//! than inventing new ones for the same physical indications).

use crate::deep::api::*;

const ATA: u16 = 29; // Hydraulic Power.

struct CircuitSpec {
    color: &'static str,
    engines: [u16; 2],
}
const CIRCUITS: [CircuitSpec; 2] = [CircuitSpec { color: "green", engines: [1, 2] }, CircuitSpec { color: "yellow", engines: [3, 4] }];

pub fn register(r: &mut Registry) {
    let mut n: u16 = 0;
    let mut next_id = |r: &mut Registry, name: &str, component: &str, model_field: &str, magnitude: &str, effect: &str| -> u64 {
        n += 1;
        r.failure(FailureDef { id: failure_id(Area::Hydraulics, ATA, n), area: Area::Hydraulics, ata: ATA, name: name.into(), component: component.into(), model_field: model_field.into(), magnitude: magnitude.into(), effect: effect.into() })
    };

    for circuit in CIRCUITS {
        let color = circuit.color;
        let mut edp_low_pressure_failures: Vec<u64> = Vec::new();

        // ---- Engine-driven pumps: two per engine (a/b), each with its own
        // check valve and firewall shutoff valve, folded into one component
        // per pump (the smallest independently trackable LRU on the real
        // aircraft).
        for engine in circuit.engines {
            for half in ['a', 'b'] {
                let comp_id = format!("29_hyd.{color}_edp_{engine}{half}");
                let disp_loss = next_id(
                    r,
                    &format!("{color} EDP {engine}{half} displacement loss"),
                    &comp_id,
                    "pump::EngineDrivenPump (via topology::EdpFaults.pump.displacement_loss)",
                    "swash/valve-plate damage: 0 healthy .. 1 zero displacement at any pressure",
                    "pump delivers proportionally less flow at every pressure on its own compensator curve; circuit pressure sags if the other pumps cannot cover the demand",
                );
                let seizure = next_id(
                    r,
                    &format!("{color} EDP {engine}{half} seizure"),
                    &comp_id,
                    "pump::EngineDrivenPump (via topology::EdpFaults.pump.seizure)",
                    "0 free .. 1 fully seized shaft",
                    "zero flow and zero case drain from this pump; a fully seized EDP is also a mechanical drag on the engine accessory gearbox (another area's concern)",
                );
                let check_open = next_id(
                    r,
                    &format!("{color} EDP {engine}{half} check valve stuck open"),
                    &comp_id,
                    "network::CheckValve (via topology::EdpFaults.check_valve.stuck_open)",
                    "0 healthy .. 1 fully jammed off its seat",
                    "loses its reverse-block function: if this pump stops, manifold pressure can bleed backward through it to the return manifold instead of being held by the other running pumps",
                );
                let check_shut = next_id(
                    r,
                    &format!("{color} EDP {engine}{half} check valve stuck shut"),
                    &comp_id,
                    "network::CheckValve (via topology::EdpFaults.check_valve.stuck_shut)",
                    "0 healthy .. 1 fully jammed on its seat",
                    "throttles or blocks this pump's own delivery even while it is otherwise healthy and turning",
                );
                let fire_sov = next_id(
                    r,
                    &format!("{color} EDP {engine}{half} fire shutoff valve stuck"),
                    &comp_id,
                    "network::FireShutoffValve (via topology::EdpFaults.fire_sov_stuck)",
                    "0 healthy .. 1 seized at its last commanded position",
                    "the firewall FIRE handle for this engine no longer isolates (stuck open, fuel/hydraulic fire risk persists) or the pump is already isolated and cannot be restored (stuck shut)",
                );
                r.component(ComponentDef {
                    id: comp_id,
                    area: Area::Hydraulics,
                    ata: ATA,
                    name: format!("{color} system engine {engine}{half} engine-driven pump"),
                    params: vec![ParamDef { name: "wear".into(), meaning: "internal clearance wear: 0 healthy .. 1 fully worn (volumetric efficiency lost to case drain)".into(), healthy: 0.0 }],
                    failures: vec![disp_loss, seizure, check_open, check_shut, fire_sov],
                });
                edp_low_pressure_failures.extend([disp_loss, seizure, check_open, check_shut, fire_sov]);
            }
        }

        // ---- Reservoir.
        let rsvr_id = format!("29_hyd.{color}_reservoir");
        let rsvr_leak = next_id(r, &format!("{color} reservoir leak"), &rsvr_id, "reservoir::Reservoir (via topology::CircuitFaults.reservoir.leak_area_m2)", "leak orifice area, 0..20 mm^2", "reservoir fluid quantity falls over time; low level eventually unports the pump inlets (progressive cavitation, not a scripted cutoff)");
        let rsvr_press = next_id(r, &format!("{color} reservoir pressurisation loss"), &rsvr_id, "reservoir::Reservoir (via topology::CircuitFaults.reservoir.pressurization_loss)", "0 healthy .. 1 no bootstrap air pressure at all", "pump inlet gauge pressure collapses toward zero, cavitating every pump on this circuit even with a full reservoir");
        let air_ingestion = next_id(r, &format!("{color} system air ingestion"), &rsvr_id, "network::Node.air_fraction_at_1atm (via topology::CircuitFaults.air_ingestion)", "0 healthy .. 1 at 5% free air by volume at 1 atm (GENERIC ceiling)", "entrained air softens the fluid's effective bulk modulus (`fluid::effective_bulk_modulus_pa`), making the whole circuit spongy/slow to pressurise and desensitising flight control response, worst at low pressure where the air has not yet been compressed away");
        r.component(ComponentDef { id: rsvr_id, area: Area::Hydraulics, ata: ATA, name: format!("{color} system hydraulic reservoir"), params: vec![], failures: vec![rsvr_leak, rsvr_press, air_ingestion] });

        // ---- Accumulator.
        let acc_id = format!("29_hyd.{color}_accumulator");
        let acc_precharge = next_id(r, &format!("{color} accumulator precharge loss"), &acc_id, "accumulator::Accumulator (via topology::CircuitFaults.accumulator.precharge_loss)", "0 full rated precharge .. 1 nitrogen fully bled to ambient", "the accumulator still fills with fluid but delivers almost no stored pressure back out on a transient demand (gear extension, brake application) or an engine-out/all-pump-loss scenario");
        r.component(ComponentDef { id: acc_id, area: Area::Hydraulics, ata: ATA, name: format!("{color} system hydraulic accumulator"), params: vec![], failures: vec![acc_precharge] });

        // ---- Priority valve.
        let pv_id = format!("29_hyd.{color}_priority_valve");
        let pv_stuck = next_id(r, &format!("{color} priority valve stuck"), &pv_id, "network::PriorityValve (via topology::CircuitFaults.priority_valve_stuck)", "0 healthy .. 1 seized at its last position", "stuck shut starves the whole non-essential branch (gear, brakes, steering, cargo doors, reversers) even with full manifold pressure; stuck open removes the flight controls' priority the moment total demand exceeds pump capacity");
        r.component(ComponentDef { id: pv_id, area: Area::Hydraulics, ata: ATA, name: format!("{color} system priority valve"), params: vec![], failures: vec![pv_stuck] });

        // ---- Relief valve.
        let rv_id = format!("29_hyd.{color}_relief_valve");
        let rv_crack = next_id(r, &format!("{color} relief valve cracking low"), &rv_id, "network::ReliefValve (via topology::CircuitFaults.relief_valve_crack_low)", "0 healthy 5400 psi crack .. 1 cracks as low as half that", "the circuit cannot reach its normal regulated pressure band; the relief valve dumps supply to return well before the pumps would otherwise destroke, capping pressure low and starving every consumer proportionally");
        r.component(ComponentDef { id: rv_id, area: Area::Hydraulics, ata: ATA, name: format!("{color} system pressure relief valve"), params: vec![], failures: vec![rv_crack] });

        // ---- Return filter.
        let filter_id = format!("29_hyd.{color}_return_filter");
        let filter_clog = next_id(r, &format!("{color} return filter contamination"), &filter_id, "network::Filter (via topology::CircuitFaults.filter_clog)", "0 clean .. 1 fully blocked", "return-side pressure drop rises until the bypass valve cracks and admits unfiltered flow; a full loss of filtration protects flow at the cost of downstream contamination");
        r.component(ComponentDef { id: filter_id, area: Area::Hydraulics, ata: ATA, name: format!("{color} system return filter"), params: vec![], failures: vec![filter_clog] });

        // ---- Branch line leaks (gear, brakes, steering, cargo doors, reversers).
        let mut branch_leak_failures: Vec<u64> = Vec::new();
        for branch in ["gear", "brakes", "steering", "cargo_doors", "reversers"] {
            let line_id = format!("29_hyd.{color}_line_{branch}");
            let leak = next_id(r, &format!("{color} {branch} line leak"), &line_id, "network::Line (via topology::CircuitFaults.line_leak_area_m2)", "leak orifice area, 0..20 mm^2", "fluid lost from this branch straight to the bay; a large enough leak drags the whole circuit's reservoir down and, if the priority valve has already shed this branch, gets no relief at all");
            r.component(ComponentDef { id: line_id, area: Area::Hydraulics, ata: ATA, name: format!("{color} system {branch} supply line"), params: vec![], failures: vec![leak] });
            branch_leak_failures.push(leak);
        }

        // ---- ECAM completeness pass (E-FIRE §G/§H): the fuel/hydraulic
        // heat exchanger's own valve, its air-side leak-detection switch,
        // and the dual overheat-detection channels -- each a pure
        // monitored-circuit discrete (`thermal::{HxValveFaults,
        // AirLeakSwitchFaults, OverheatChannelFaults}`'s own doc), wired
        // directly in `deep::ecam::fbw::ata29` off the published names
        // `HydraulicsLive::publish` adds. Not physics: none of these gate
        // `topology::Circuit`'s own heat balance (§I's manifold state is
        // the one genuine physics addition, unconditional and un-faulted).
        let hx_valve_id = format!("29_hyd.{color}_fuel_hx_valve");
        let hx_valve_stuck = next_id(r, &format!("{color} fuel/hydraulic HX valve stuck"), &hx_valve_id, "hydraulics::thermal::HxValveFaults.stuck", "0 healthy .. 1 (any nonzero) the valve's own monitored position/circuit reports faulted", "the HX valve's own monitored circuit reports faulted, independent of whether the fluid is actually hot");
        r.component(ComponentDef { id: hx_valve_id, area: Area::Hydraulics, ata: ATA, name: format!("{color} system fuel/hydraulic heat exchanger valve"), params: vec![], failures: vec![hx_valve_stuck] });

        let air_leak_id = format!("29_hyd.{color}_fuel_hx_air_leak_switch");
        let air_leak_leak = next_id(r, &format!("{color} fuel/hydraulic HX air-side leak"), &air_leak_id, "hydraulics::thermal::AirLeakSwitchFaults.leak", "0 healthy .. 1 (any nonzero) the leak-detection switch reports a leak", "the heat exchanger's air-side leak-detection switch reports a leak present");
        let air_leak_circuit_fault = next_id(r, &format!("{color} fuel/hydraulic HX air-leak switch circuit fault"), &air_leak_id, "hydraulics::thermal::AirLeakSwitchFaults.circuit_fault", "0 healthy .. 1 (any nonzero) the switch's own monitoring circuit reports faulted", "the leak-detection switch's own monitoring circuit is faulted, independent of whether a leak is actually present");
        r.component(ComponentDef { id: air_leak_id, area: Area::Hydraulics, ata: ATA, name: format!("{color} system fuel/hydraulic heat exchanger air-leak detection switch"), params: vec![], failures: vec![air_leak_leak, air_leak_circuit_fault] });

        for channel in ["a", "b"] {
            let chan_id = format!("29_hyd.{color}_ovht_chan_{channel}");
            let chan_fault = next_id(r, &format!("{color} system overheat-detection channel {}", channel.to_uppercase()), &chan_id, "hydraulics::thermal::OverheatChannelFaults.circuit_fault", "0 healthy .. 1 (any nonzero) this channel's own monitoring circuit reports faulted", "this overheat-detection channel's own monitoring circuit is faulted, independent of the circuit's actual fluid temperature");
            r.component(ComponentDef { id: chan_id, area: Area::Hydraulics, ata: ATA, name: format!("{color} system overheat-detection channel {}", channel.to_uppercase()), params: vec![], failures: vec![chan_fault] });
        }
    }

    // ---- Electric pumps: two per circuit, each fed from its own AC bus,
    // so losing one bus still leaves that circuit a powered pump (green
    // A/B on AC 1/2, yellow A/B on AC 3/4 -- `topology.rs`'s module doc).
    for (colour, letter, bus) in [("green", "a", 1), ("green", "b", 2), ("yellow", "a", 3), ("yellow", "b", 4)] {
        let index = usize::from(letter == "b");
        let name = format!("{colour} system electric motor pump {}", letter.to_uppercase());
        let ep_id = format!("29_hyd.{colour}_electric_pump_{letter}");
        let ep_disp = next_id(
            r,
            &format!("{name} displacement loss"),
            &ep_id,
            &format!("pump::ElectricPump (via topology::CircuitFaults.electric_pump[{index}].displacement_loss)"),
            "0 healthy .. 1 zero displacement at any pressure",
            "electric pump delivers proportionally less flow at every pressure",
        );
        let ep_seize = next_id(
            r,
            &format!("{name} seizure"),
            &ep_id,
            &format!("pump::ElectricPump (via topology::CircuitFaults.electric_pump[{index}].seizure)"),
            "0 free .. 1 fully seized",
            "zero flow and zero case drain; the motor still spins (electrically healthy) against a jammed pump end",
        );
        r.component(ComponentDef {
            id: ep_id,
            area: Area::Hydraulics,
            ata: ATA,
            name: format!("{name}, motor supply AC {bus}"),
            params: vec![ParamDef { name: "wear".into(), meaning: "internal clearance wear: 0 healthy .. 1 fully worn".into(), healthy: 0.0 }],
            failures: vec![ep_disp, ep_seize],
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registers_without_validation_errors() {
        let mut r = Registry::default();
        register(&mut r);
        let errors = r.validate_area();
        assert!(errors.is_empty(), "{errors:?}");
    }

    #[test]
    fn registers_every_edp_across_both_circuits() {
        let mut r = Registry::default();
        register(&mut r);
        let edp_components: Vec<_> = r.components.iter().filter(|c| c.id.contains("_edp_")).collect();
        assert_eq!(edp_components.len(), 8, "4 per circuit x 2 circuits");
        for id in ["29_hyd.green_edp_1a", "29_hyd.green_edp_1b", "29_hyd.green_edp_2a", "29_hyd.green_edp_2b", "29_hyd.yellow_edp_3a", "29_hyd.yellow_edp_3b", "29_hyd.yellow_edp_4a", "29_hyd.yellow_edp_4b"] {
            assert!(r.components.iter().any(|c| c.id == id), "missing {id}");
        }
    }

    /// Both circuits carry two electric pumps, split across two AC buses.
    /// This test used to assert the opposite -- that no green electric
    /// pump existed -- which was wrong about the aircraft: FlyByWire's own
    /// `A380Hydraulic` constructs all four
    /// (`a380_systems/src/hydraulic/mod.rs:1934-1985`).
    #[test]
    fn both_circuits_register_two_electric_pumps_on_separate_ac_buses() {
        let mut r = Registry::default();
        register(&mut r);
        for id in ["29_hyd.green_electric_pump_a", "29_hyd.green_electric_pump_b", "29_hyd.yellow_electric_pump_a", "29_hyd.yellow_electric_pump_b"] {
            assert!(r.components.iter().any(|c| c.id == id), "missing {id}");
        }
        let pumps: Vec<_> = r.components.iter().filter(|c| c.id.contains("_electric_pump_")).collect();
        assert_eq!(pumps.len(), 4, "2 per circuit x 2 circuits");
        // Each names the bus it is fed from, and the four are all different
        // -- a circuit whose pumps shared a bus would lose both at once.
        for bus in ["AC 1", "AC 2", "AC 3", "AC 4"] {
            assert_eq!(pumps.iter().filter(|c| c.name.contains(bus)).count(), 1, "exactly one pump on {bus}");
        }
    }

    #[test]
    fn every_failure_id_carries_the_hydraulics_area_and_ata_29() {
        let mut r = Registry::default();
        register(&mut r);
        assert!(!r.failures.is_empty());
        for f in &r.failures {
            assert_eq!(f.ata, 29);
            assert!(f.id / 1_000_000 == Area::Hydraulics as u64);
        }
    }

    #[test]
    fn hydraulic_cautions_are_flybywires_own() {
        let mut r = Registry::default();
        register(&mut r);
        assert!(r.alerts.is_empty(), "FlyByWire raises HYD RSVR LEVEL LO / AIR PRESS LO / SYS OVHT / SYS LO PRESS from the variables this area publishes");
    }
}

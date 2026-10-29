use fms_vnav::config::{AircraftConfig, FlapConf};
use fms_vnav::profile::*;
use fms_vnav::speed_profile::McduSpeedProfile;
use fms_vnav::strategy::ClimbThrustClimbStrategy;
use fms_vnav::{climb::*, cruise::*, takeoff::*};
use serde_json::Value;

const ORACLE: &str = "E:/fbw-build/fms-oracle";

fn load(name: &str) -> Option<Value> {
    let text = std::fs::read_to_string(format!("{ORACLE}/{name}.json")).ok()?;
    serde_json::from_str(&text).ok()
}

fn num(v: &Value, key: &str) -> f64 {
    v.get(key).and_then(Value::as_f64).unwrap_or(f64::NAN)
}

fn params_from_golden(state: &Value) -> ComputationParameters {
    let p = &state["guidance"]["computationParametersObserver"]["parameters"];
    let perf = &state["plans"]["active"]["performance"];
    ComputationParameters {
        present_altitude: num(p, "departureElevation"),
        fcu_altitude: num(p, "fcuAltitude"),
        fcu_speed: num(p, "fcuSpeed"),
        managed_climb_speed: num(p, "managedClimbSpeed"),
        managed_climb_speed_mach: num(p, "managedClimbSpeedMach"),
        managed_cruise_speed: num(p, "managedCruiseSpeed"),
        managed_cruise_speed_mach: num(p, "managedCruiseSpeedMach"),
        managed_descent_speed: num(p, "managedDescentSpeed"),
        managed_descent_speed_mach: num(p, "managedDescentSpeedMach"),
        zero_fuel_weight: num(p, "zeroFuelWeight"),
        fuel_on_board: num(p, "fuelOnBoard"),
        v2_speed: num(p, "v2Speed"),
        tropopause: num(p, "tropoPause"),
        perf_factor: num(p, "perfFactor"),
        departure_elevation: num(p, "departureElevation"),
        destination_elevation: num(p, "destinationElevation"),
        acceleration_altitude: num(p, "accelerationAltitude"),
        thrust_reduction_altitude: num(p, "thrustReductionAltitude"),
        cruise_altitude: num(perf, "cruiseFlightLevel") * 100.0,
        climb_speed_limit: Some(SpeedLimit { speed: num(perf, "climbSpeedLimitSpeed"), under_altitude: num(perf, "climbSpeedLimitAltitude") }),
        descent_speed_limit: Some(SpeedLimit { speed: num(perf, "descentSpeedLimitSpeed"), under_altitude: num(perf, "descentSpeedLimitAltitude") }),
        flight_phase: FlightPhase::Preflight,
        preselected_clb_speed: num(p, "preselectedClbSpeed"),
        preselected_cruise_speed: num(p, "preselectedCruiseSpeed"),
        takeoff_flaps: FlapConf::Conf1,
        takeoff_toga_n1: 90.0,
        isa_deviation: 0.0,
        approach_speed: num(p, "approachSpeed"),
        flap_retraction_speed: num(p, "flapRetractionSpeed"),
        slat_retraction_speed: num(p, "slatRetractionSpeed"),
        clean_speed: num(p, "cleanSpeed"),
        gross_weight_valid: false,
    }
}

fn cell_is_blank(text: &str) -> bool {
    text.starts_with("--:--") || text.starts_with("---.-") || text.starts_with("---")
}

#[test]
fn recorded_goldens_have_no_predictions_and_the_profile_is_not_computable() {
    for name in ["uplink-egll-kjfk", "city-pair-procedures", "edits-tmpy", "secondary"] {
        let Some(golden) = load(name) else { continue };
        for snap in golden["snapshots"].as_array().unwrap() {
            let params = params_from_golden(&snap["state"]);
            assert!(!params.can_compute_profile(), "{name}/{}", snap["label"]);
            for cell in snap["mfd"].as_array().unwrap() {
                let text = cell.as_str().unwrap();
                let (value, _) = text.split_once('@').unwrap();
                if value.contains(':') && value.len() == 5 && !value.starts_with("--") {
                    panic!("{name}/{}: recorded a time prediction {text}", snap["label"]);
                }
            }
        }
    }
}

#[test]
fn uplink_fpln_page_prediction_cells_are_all_blank() {
    let Some(golden) = load("uplink-egll-kjfk") else { return };
    let snap = golden["snapshots"].as_array().unwrap().iter().find(|s| s["label"] == "fpln-page").unwrap();
    let cells: Vec<&str> = snap["mfd"].as_array().unwrap().iter().map(|c| c.as_str().unwrap()).collect();
    let times: Vec<&&str> = cells.iter().filter(|c| c.contains(":") && c.contains('@')).collect();
    assert_eq!(times.len(), 9);
    assert!(times.iter().all(|t| cell_is_blank(t)));
    assert!(cells.iter().any(|c| c.starts_with("3580@")));
    let total = num(&snap["state"]["guidance"]["constraintReader"], "totalFlightPlanDistance");
    assert_eq!(total.round() as i64, 3580);
}

#[test]
fn uplink_plan_with_weights_gives_a_plausible_climb() {
    let Some(golden) = load("uplink-egll-kjfk") else { return };
    let snap = golden["snapshots"].as_array().unwrap().iter().find(|s| s["label"] == "fpln-page").unwrap();
    let mut params = params_from_golden(&snap["state"]);
    params.zero_fuel_weight = 350_000.0 * 2.20462;
    params.fuel_on_board = 120_000.0 * 2.20462;
    params.gross_weight_valid = true;
    params.flight_phase = FlightPhase::Takeoff;
    assert!(params.can_compute_profile());

    let cfg = AircraftConfig::a380();
    let mut profile = Profile::new(&cfg, Box::new(ConstantWinds::default()));
    build_takeoff_path(&mut profile, &params);
    let sp = McduSpeedProfile::new(&params, 0.0, vec![], vec![]);
    let strategy = ClimbThrustClimbStrategy { params: &params, atm: params.atmosphere(), config: &cfg };
    ClimbPathBuilder::new(&params).compute_climb_path(&mut profile, &strategy, &sp, params.cruise_altitude, FcuClimbHint::default());
    let toc = *profile.last();
    let total = num(&snap["state"]["guidance"]["constraintReader"], "totalFlightPlanDistance");
    let cruise_end = (total - 100.0).max(toc.distance_from_start);
    let cruise = CruisePathBuilder::new(&params).compute_cruise_path(&mut profile, toc, cruise_end, &sp);
    let end = cruise.last().unwrap();

    println!(
        "EGLL-KJFK ZFW 350 t, fuel 120 t, CI30 FL360: TOC {:.1} NM, {:.0} s, {:.0} kg; cruise end {:.1} NM, {:.0} s, fuel burned {:.0} kg",
        toc.distance_from_start,
        toc.seconds_from_present,
        (params.fuel_on_board - toc.remaining_fuel_on_board) / 2.20462,
        end.distance_from_start,
        end.seconds_from_present,
        (params.fuel_on_board - end.remaining_fuel_on_board) / 2.20462
    );
    assert!(toc.distance_from_start > 100.0 && toc.distance_from_start < 350.0);
    assert!(end.seconds_from_present > 18000.0 && end.seconds_from_present < 30000.0);
}

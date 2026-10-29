use fms_data::command::{Command, Event};
use fms_data::entry::{Entry, EntryContext};
use fms_data::service::PerfDataStore;
use fms_data::{CostIndexMode, PerformanceData, TakeoffPowerSetting};
use serde_json::Value;

const GOLDEN: &str = "E:/fbw-build/fms-oracle/perf-data-egll-kjfk.json";

fn dump(golden: &Value, label: &str) -> Value {
    for e in golden["events"].as_array().unwrap() {
        let Some(json) = e.get("json").and_then(Value::as_str) else { continue };
        if !json.contains("oracle_perf_dump") {
            continue;
        }
        let parsed: Value = serde_json::from_str(json).unwrap();
        for ev in parsed["events"].as_array().unwrap() {
            if ev["topic"] == "oracle_perf_dump" && ev["data"]["label"] == label {
                return ev["data"].clone();
            }
        }
    }
    panic!("no oracle_perf_dump {label}");
}

fn check(d: &Value, pd: &PerformanceData) -> Vec<String> {
    let n = |k: &str| d[k].as_f64();
    let mut bad = Vec::new();
    let mut eq = |k: &str, ours: Option<f64>| {
        if n(k) != ours {
            bad.push(format!("{k}: fbw {} ours {ours:?}", d[k]));
        }
    };
    eq("thrustReductionAltitude", pd.thrust_reduction_altitude());
    eq("accelerationAltitude", pd.acceleration_altitude());
    eq("engineOutAccelerationAltitude", pd.engine_out_acceleration_altitude());
    eq("missedThrustReductionAltitude", pd.missed_thrust_reduction_altitude());
    eq("missedAccelerationAltitude", pd.missed_acceleration_altitude());
    eq("transitionAltitude", pd.transition_altitude());
    eq("transitionLevel", pd.transition_level());
    eq("zeroFuelWeight", pd.zero_fuel_weight);
    eq("zeroFuelWeightCenterOfGravity", pd.zero_fuel_weight_center_of_gravity);
    eq("blockFuel", pd.block_fuel);
    eq("costIndex", pd.cost_index);
    eq("cruiseFlightLevel", pd.cruise_flight_level);
    eq("v1", pd.v1);
    eq("vr", pd.vr);
    eq("v2", pd.v2);
    eq("takeoffFlaps", pd.takeoff_flaps.map(f64::from));
    eq("flexTakeoffTemperature", pd.flex_takeoff_temperature);
    eq("takeoffPowerSetting", Some(pd.takeoff_power_setting as u8 as f64));
    eq("costIndexMode", pd.cost_index_mode.map(|m| m as u8 as f64));
    eq("approachQnh", pd.approach_qnh);
    eq("approachTemperature", pd.approach_temperature);
    eq("approachWindDirection", pd.approach_wind_direction);
    eq("approachWindMagnitude", pd.approach_wind_magnitude);
    eq("approachBaroMinimum", pd.approach_baro_minimum);
    eq("approachRadioMinimum", pd.approach_radio_minimum);
    eq("tropopause", Some(pd.tropopause()));
    eq("taxiFuel", Some(pd.taxi_fuel()));
    eq("routeReserveFuelPercentage", Some(pd.route_reserve_fuel_percentage()));
    eq("finalHoldingTime", Some(pd.final_holding_time()));
    if d["isApproachWindPilotEntered"].as_bool() != Some(pd.is_approach_wind_pilot_entered) {
        bad.push("isApproachWindPilotEntered".into());
    }
    if d["approachFlapsThreeSelected"].as_bool() != Some(pd.approach_flaps_three_selected) {
        bad.push("approachFlapsThreeSelected".into());
    }
    bad
}

#[test]
fn perf_data_matches_fbw() {
    let Ok(text) = std::fs::read_to_string(GOLDEN) else {
        eprintln!("perf-data-egll-kjfk: no golden at {GOLDEN}");
        return;
    };
    let golden: Value = serde_json::from_str(&text).unwrap();
    let up = dump(&golden, "UPLINK");
    let origin_elev = up["defaultThrustReductionAltitude"].as_f64().map(|v| v - 1500.0);
    let dest_elev = up["defaultMissedThrustReductionAltitude"].as_f64().map(|v| v - 1500.0);
    let ctx = EntryContext { active: true, active_or_copy: true, has_destination: true, destination_elevation_ft: dest_elev, ..Default::default() };
    let mut s = PerfDataStore::default();
    s.execute(&ctx, Command::OriginChanged { plan: 0, elevation_ft: origin_elev, transition_altitude_ft: up["databaseTransitionAltitude"].as_f64() }).unwrap();
    s.execute(&ctx, Command::DestinationChanged { plan: 0, elevation_ft: dest_elev, transition_level: up["databaseTransitionLevel"].as_f64() }).unwrap();
    s.ensure(0).cost_index = up["costIndex"].as_f64();
    s.ensure(0).cruise_flight_level = up["cruiseFlightLevel"].as_f64();
    s.ensure(0).cost_index_mode = up["costIndexMode"].as_f64().map(|m| if m == 0.0 { CostIndexMode::Lrc } else { CostIndexMode::Econ });
    let bad = check(&up, s.get(0).unwrap());
    assert!(bad.is_empty(), "UPLINK: {bad:?}");

    for e in [
        Entry::ZeroFuelWeightKg(Some(330_000.0)),
        Entry::ZeroFuelWeightCg(Some(38.5)),
        Entry::BlockFuelKg(Some(95_000.0)),
        Entry::CostIndex(Some(30.0)),
        Entry::V1(Some(140.0)),
        Entry::Vr(Some(150.0)),
        Entry::V2(Some(158.0)),
        Entry::TakeoffFlaps(Some(2)),
        Entry::FlexTemperature(Some(45.0)),
        Entry::TakeoffPowerSetting(TakeoffPowerSetting::Flex),
        Entry::ThrustReductionAltitude(Some(1585.0)),
        Entry::ApproachQnh(Some(1013.0)),
        Entry::ApproachTemperature(Some(15.0)),
        Entry::ApproachBaroMinimum(Some(300.0)),
        Entry::ApproachRadioMinimum(Some(0.0)),
        Entry::ApproachWindDirection(Some(220.0)),
        Entry::ApproachWindMagnitude(Some(12.0)),
        Entry::CruiseFlightLevel(360.0),
    ] {
        s.execute(&ctx, Command::Enter { plan: 0, entry: e }).unwrap();
    }
    let bad = check(&dump(&golden, "ENTERED"), s.get(0).unwrap());
    assert!(bad.is_empty(), "ENTERED: {bad:?}");

    let ev = s.execute(&ctx, Command::Enter { plan: 0, entry: Entry::ApproachWindDirection(None) }).unwrap();
    assert!(ev.iter().any(|e| matches!(e, Event::Changed("isApproachWindPilotEntered"))));
    let bad = check(&dump(&golden, "WIND_DELETED"), s.get(0).unwrap());
    assert!(bad.is_empty(), "WIND_DELETED: {bad:?}");
}

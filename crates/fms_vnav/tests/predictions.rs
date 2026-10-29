use fms_vnav::config::{AircraftConfig, FlapConf};
use fms_vnav::profile::*;
use fms_vnav::constraints::{ConstraintLeg, ConstraintPlan, ConstraintReader, ConstraintType};
use fms_vnav::coordinator::CruiseToDescentCoordinator;
use fms_vnav::manager::{compute_tactical_mcdu_path, DescentProfileRelation, PresentState, TacticalInputs};
use fms_vnav::waypoints::{compute_predictions_at_waypoints, WaypointLeg};
use fms_vnav::wind::{WindEntry, WindFixes, WindLeg, WindPlan, WindProfile, WindVector};
use serde_json::Value;

const ORACLE: &str = "E:/fbw-build/fms-oracle";
const LB_PER_KG: f64 = 2.20462;

fn load() -> Option<Value> {
    load_named("predictions-egll-kjfk")
}

fn load_named(name: &str) -> Option<Value> {
    let dir = std::env::var("FMS_ORACLE_DIR").unwrap_or_else(|_| ORACLE.to_string());
    let text = std::fs::read_to_string(format!("{dir}/{name}.json")).ok()?;
    serde_json::from_str(&text).ok()
}

fn num(v: &Value, key: &str) -> f64 {
    v.get(key).and_then(Value::as_f64).unwrap_or(f64::NAN)
}

fn dump(golden: &Value, label: &str) -> Value {
    for e in golden["events"].as_array().unwrap() {
        let Some(json) = e.get("json").and_then(Value::as_str) else { continue };
        if !json.contains("oracle_vnav_dump") {
            continue;
        }
        let parsed: Value = serde_json::from_str(json).unwrap();
        for ev in parsed["events"].as_array().unwrap() {
            if ev["topic"] == "oracle_vnav_dump" && ev["data"]["label"] == label {
                return ev["data"].clone();
            }
        }
    }
    panic!("no oracle_vnav_dump {label}");
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
        // The headless oracle leaves L:A32NX_AUTOTHRUST_THRUST_LIMIT_TOGA at 0.
        takeoff_toga_n1: 0.0,
        isa_deviation: 0.0,
        approach_speed: num(p, "approachSpeed"),
        flap_retraction_speed: num(p, "flapRetractionSpeed"),
        slat_retraction_speed: num(p, "slatRetractionSpeed"),
        clean_speed: num(p, "cleanSpeed"),
        gross_weight_valid: true,
    }
}

#[test]
fn recorded_waypoint_predictions_are_ordered_and_consistent() {
    let Some(golden) = load() else { return };
    for label in ["NOWIND", "WIND"] {
        let d = dump(&golden, label);
        let wps = d["waypointPredictions"].as_array().unwrap();
        assert!(wps.len() >= 10);
        for w in wps.windows(2) {
            assert!(num(&w[1], "distanceFromStart") >= num(&w[0], "distanceFromStart"));
            assert!(num(&w[1], "secondsFromPresent") >= num(&w[0], "secondsFromPresent"));
            assert!(num(&w[1], "estimatedFuelOnBoard") <= num(&w[0], "estimatedFuelOnBoard"));
        }
    }
}

fn wind_entries(v: &Value) -> Vec<WindEntry> {
    v.as_array()
        .map(|a| {
            a.iter()
                .map(|e| WindEntry {
                    vector: WindVector { magnitude: e["vector"]["magnitude"].as_f64(), direction: e["vector"]["direction"].as_f64() },
                    altitude: e["altitude"].as_f64(),
                })
                .collect()
        })
        .unwrap_or_default()
}

fn wind_plan(state: &Value) -> WindPlan {
    let active = &state["plans"]["active"];
    let perf = &active["performance"];
    let p = &state["guidance"]["computationParametersObserver"]["parameters"];
    let mut cumulative = 0.0;
    let mut prev_cumulative_direct = 0.0;
    let mut after_disco = false;
    let mut elements = Vec::new();
    let legs = active["legs"].as_array().unwrap();
    for leg in legs {
        if leg.get("disco").is_some() {
            elements.push(None);
            after_disco = true;
            continue;
        }
        let calc = &leg["calc"];
        let dwt = num(calc, "distanceWithTransitions");
        let direct = num(calc, "cumulativeDistance");
        cumulative += if after_disco { direct - prev_cumulative_direct } else { dwt };
        prev_cumulative_direct = direct;
        after_disco = false;
        elements.push(Some(WindLeg {
            cumulative_distance_with_transitions: cumulative,
            distance_with_transitions: dwt,
            true_track: calc["trueTrack"].as_f64(),
            calculated: calc.is_object(),
            cruise_wind_entries: wind_entries(&leg["cruiseWindEntries"]),
        }));
    }
    let scenario = |m: f64, d: f64, alt: f64| WindEntry { vector: WindVector::from_degrees(m, d), altitude: Some(alt) };
    let mut climb = wind_entries(&perf["climbWindEntries"]);
    let mut descent = wind_entries(&perf["descentWindEntries"]);
    // The snapshot serialises the wind entries as null; these are the values the winds scenario enters.
    if climb.iter().all(|e| e.altitude.is_none()) {
        climb = vec![scenario(70.0, 290.0, 30000.0), scenario(35.0, 280.0, 10000.0), scenario(15.0, 270.0, 0.0)];
    }
    if descent.iter().all(|e| e.altitude.is_none()) {
        descent = vec![scenario(60.0, 300.0, 30000.0), scenario(30.0, 290.0, 10000.0), scenario(12.0, 280.0, 0.0)];
    }
    WindPlan {
        first_missed_approach_leg_index: elements.len(),
        elements,
        max_cruise_wind_levels: 4,
        climb_wind_entries: climb,
        descent_wind_entries: descent,
        pilot_trip_wind: perf["pilotTripWind"].as_f64(),
        origin_altitude: legs.first().and_then(|l| l["def"]["waypoint"]["location"]["alt"].as_f64()).unwrap_or(num(p, "departureElevation")),
        destination_altitude: legs.last().and_then(|l| l["def"]["waypoint"]["location"]["alt"].as_f64()).unwrap_or(num(p, "destinationElevation")),
    }
}

struct Built<'a> {
    profile: Profile<'a>,
    tod: Checkpoint,
    coordinator: CruiseToDescentCoordinator,
}

fn constraint_plan(state: &Value) -> ConstraintPlan {
    let wp = wind_plan(state);
    let total = wp.elements.iter().flatten().last().map(|l| l.cumulative_distance_with_transitions).unwrap_or(0.0);
    let legs = state["plans"]["active"]["legs"].as_array().unwrap();
    ConstraintPlan {
        elements: wp
            .elements
            .iter()
            .map(|e| {
                e.as_ref().map(|l| ConstraintLeg {
                    cumulative_distance_with_transitions: Some(l.cumulative_distance_with_transitions),
                    cumulative_distance_to_end_with_transitions: Some(total - l.cumulative_distance_with_transitions),
                    constraint_type: ConstraintType::Unknown,
                    altitude_constraint: None,
                    speed_constraint: None,
                    cruise_step: None,
                    vertical_angle: None,
                    is_final_approach_fix: false,
                })
            })
            .collect(),
        active_leg_index: state["plans"]["active"]["activeLegIndex"].as_u64().unwrap() as usize,
        first_missed_approach_leg_index: wp.elements.len(),
        destination_leg_index: Some(legs.len() - 1),
        missed_approach_point_altitude: None,
        destination_runway_threshold_altitude: None,
        destination_airport_altitude: legs.last().and_then(|l| l["def"]["waypoint"]["location"]["alt"].as_f64()),
    }
}

/// The whole profile from the crate: takeoff, climb, then `CruiseToDescentCoordinator` (approach, managed descent, cruise to the TOD).
/// The destination estimate starts from the reset value and is carried over `passes` recomputations, as FBW keeps it between updates.
fn build_full<'a>(
    cfg: &'a AircraftConfig,
    params: &ComputationParameters,
    state: &Value,
    dump: &Value,
    winds: &dyn Fn() -> Box<dyn Winds + 'a>,
    mut coordinator: CruiseToDescentCoordinator,
    passes: usize,
) -> Built<'a> {
    let mut reader = ConstraintReader::default();
    let plan = constraint_plan(state);
    let total = num(dump, "totalFlightPlanDistance");
    reader.update_flight_plan(&plan, total - num(dump, "distanceToPresentPosition"));
    let mut last = None;
    for _ in 0..passes {
        last = Some(compute_tactical_mcdu_path(params, cfg, &reader, winds(), &mut coordinator, None, &TacticalInputs::default()).unwrap().profile);
    }
    let profile = last.unwrap();
    let tod = *profile.checkpoints.iter().find(|c| c.reason == Reason::TopOfDescent).unwrap();
    Built { profile, tod, coordinator }
}

fn compare_waypoints(name: &str, snapshot: &str, label: &str, winds: bool) -> Option<()> {
    let golden = load_named(name)?;
    let snap = golden["snapshots"].as_array().unwrap().iter().find(|s| s["label"] == snapshot).unwrap();
    let params = params_from_golden(&snap["state"]);
    let d = dump(&golden, label);
    let gcp = d["checkpoints"].as_array().unwrap().clone();
    let cfg = AircraftConfig::a380();
    let state = &snap["state"];
    let make = || -> Box<dyn Winds> {
        if winds {
            let mut plan = wind_plan(state);
            // The snapshot does not serialise cruise winds; winds2 inserts this entry at leg 3 (IPNOX) and confirms the draft.
            if name.ends_with("winds2") {
                plan.elements[3].as_mut().unwrap().cruise_wind_entries = vec![WindEntry { vector: WindVector::from_degrees(90.0, 300.0), altitude: Some(36000.0) }];
            }
            Box::new(WindProfile::new(plan, None, WindFixes::FBW))
        } else {
            Box::new(ConstantWinds::default())
        }
    };
    // The oracle computed the no-wind profile twice before the NOWIND dump, then once more after the winds went in.
    let passes: usize = std::env::var("FMS_VNAV_PASSES").ok().and_then(|v| v.parse().ok()).unwrap_or(if winds { 1 } else { 2 });
    let seed_passes: usize = std::env::var("FMS_VNAV_SEED_PASSES").ok().and_then(|v| v.parse().ok()).unwrap_or(2);
    let mut coordinator = CruiseToDescentCoordinator::default();
    coordinator.reset_estimations(None, &cfg);
    if winds && seed_passes > 0 {
        let s0 = golden["snapshots"].as_array().unwrap().iter().find(|s| s["label"] == "predictions-no-wind").unwrap();
        let p0 = params_from_golden(&s0["state"]);
        let calm = || -> Box<dyn Winds> { Box::new(ConstantWinds::default()) };
        coordinator = build_full(&cfg, &p0, &s0["state"], &dump(&golden, "NOWIND"), &calm, coordinator, seed_passes).coordinator;
    }
    let built = build_full(&cfg, &params, state, &d, &make, coordinator, passes);
    compare_checkpoints(name, &gcp, &built);
    if std::env::var("FMS_VNAV_DUMP").is_ok() {
        for (g, m) in gcp.iter().zip(&built.profile.checkpoints).take(32) {
            println!(
                "{:<26} golden {:>8.3} NM {:>7.1} s {:>6.0} ft | here {:<26} {:>8.3} NM {:>7.1} s {:>6.0} ft",
                g["reason"].as_str().unwrap(),
                num(g, "distanceFromStart"),
                num(g, "secondsFromPresent"),
                num(g, "altitude"),
                format!("{:?}", m.reason),
                m.distance_from_start,
                m.seconds_from_present,
                m.altitude
            );
        }
    }
    let gwps = d["waypointPredictions"].as_array().unwrap();
    let legs: Vec<WaypointLeg> = gwps
        .iter()
        .map(|w| WaypointLeg {
            index: w["leg"].as_u64().unwrap() as usize,
            cumulative_distance_with_transitions: num(w, "distanceFromStart"),
            altitude_constraint: None,
            speed_constraint: None,
        })
        .collect();
    let atm = params.atmosphere();
    let mine = compute_predictions_at_waypoints(&built.profile, &atm, &legs, num(&d, "distanceToPresentPosition"), false);
    assert_eq!(mine.len(), 13, "{name}: 13 F-PLN waypoint predictions");
    let mut worst = [0.0_f64; 4];
    for (g, m) in gwps.iter().zip(&mine) {
        let dt = m.seconds_from_present - num(g, "secondsFromPresent");
        let dfuel = (m.estimated_fuel_on_board - num(g, "estimatedFuelOnBoard")) / LB_PER_KG;
        let dspd = m.speed - num(g, "speed");
        let dalt = m.altitude - num(g, "altitude");
        println!("{name} leg {:>2} {:>8.2} NM dt {:+.3} s dfuel {:+.3} kg dspd {:+.5} dalt {:+.3} ft", m.leg, m.distance_from_start, dt, dfuel, dspd, dalt);
        worst[0] = worst[0].max(dt.abs());
        worst[1] = worst[1].max(dfuel.abs());
        worst[2] = worst[2].max(if m.speed < 2.0 { dspd.abs() * 600.0 } else { dspd.abs() });
        worst[3] = worst[3].max(dalt.abs());
    }
    let at = |r: &str| num(&gcp[gcp.iter().position(|c| c["reason"] == r).unwrap()], "distanceFromStart");
    println!(
        "{name}: TOD golden {:.3} NM here {:.3} NM; decel {:.3} NM, landing {:.3} NM; destination estimate {:.1} lb {:.1} s; worst over all waypoints: time {:.3} s, fuel {:.3} kg, speed {:.3} kt (Mach x600), altitude {:.3} ft",
        at("TopOfDescent"),
        built.tod.distance_from_start,
        at("Decel"),
        at("Landing"),
        built.coordinator.last_estimated_fuel_at_destination,
        built.coordinator.last_estimated_time_at_destination,
        worst[0],
        worst[1],
        worst[2],
        worst[3]
    );
    assert!(worst[0] <= 1.0, "{name} time");
    assert!(worst[1] <= 10.0, "{name} fuel");
    assert!(worst[2] <= 0.5, "{name} speed");
    assert!(worst[3] <= 1.0, "{name} altitude");
    assert!((built.tod.distance_from_start - at("TopOfDescent")).abs() <= 0.01, "{name} TOD");
    Some(())
}

fn compare_checkpoints(name: &str, gcp: &[Value], built: &Built) {
    let mine = &built.profile.checkpoints;
    if std::env::var("FMS_VNAV_DUMP").is_ok() {
        for (i, m) in mine.iter().enumerate() {
            let g = gcp.get(i);
            println!(
                "{i:>3} {:<30} {:>9.3} NM {:>8.1} s {:>7.1} ft {:>6.1} kt | golden {:<30} {:>9.3} NM {:>8.1} s {:>7.1} ft {:>6.1} kt",
                format!("{:?}", m.reason),
                m.distance_from_start,
                m.seconds_from_present,
                m.altitude,
                m.speed,
                g.map(|g| g["reason"].as_str().unwrap()).unwrap_or("-"),
                g.map(|g| num(g, "distanceFromStart")).unwrap_or(f64::NAN),
                g.map(|g| num(g, "secondsFromPresent")).unwrap_or(f64::NAN),
                g.map(|g| num(g, "altitude")).unwrap_or(f64::NAN),
                g.map(|g| num(g, "speed")).unwrap_or(f64::NAN)
            );
        }
    }
    assert_eq!(mine.len(), gcp.len(), "{name}: checkpoint count");
    let mut worst = [0.0_f64; 5];
    for (i, (g, m)) in gcp.iter().zip(mine).enumerate() {
        assert_eq!(g["reason"].as_str().unwrap(), format!("{:?}", m.reason), "{name}: checkpoint {i}");
        let d = [
            m.distance_from_start - num(g, "distanceFromStart"),
            m.seconds_from_present - num(g, "secondsFromPresent"),
            m.altitude - num(g, "altitude"),
            m.speed - num(g, "speed"),
            (m.remaining_fuel_on_board - num(g, "remainingFuelOnBoard")) / LB_PER_KG,
        ];
        for k in 0..5 {
            worst[k] = worst[k].max(d[k].abs());
        }
    }
    println!(
        "{name}: all {} checkpoints, worst distance {:.4} NM, time {:.3} s, altitude {:.3} ft, speed {:.3} kt, fuel {:.3} kg",
        mine.len(),
        worst[0],
        worst[1],
        worst[2],
        worst[3],
        worst[4]
    );
    assert!(worst[0] <= 0.01, "{name} distance");
    assert!(worst[1] <= 1.0, "{name} time");
    assert!(worst[2] <= 1.0, "{name} altitude");
    assert!(worst[3] <= 0.5, "{name} speed");
    assert!(worst[4] <= 10.0, "{name} fuel");
}

#[test]
fn f_pln_waypoint_predictions_no_wind() {
    compare_waypoints("predictions-egll-kjfk", "predictions-no-wind", "NOWIND", false);
}

#[test]
fn f_pln_waypoint_predictions_winds() {
    compare_waypoints("predictions-egll-kjfk-winds", "predictions-winds", "WIND", true);
}

#[test]
fn f_pln_waypoint_predictions_winds2_no_wind() {
    compare_waypoints("predictions-egll-kjfk-winds2", "predictions-no-wind", "NOWIND", false);
}

#[test]
fn f_pln_waypoint_predictions_winds2_cruise_wind() {
    compare_waypoints("predictions-egll-kjfk-winds2", "predictions-winds", "WIND", true);
}

const INFLIGHT: &str = "predictions-egll-kjfk-inflight";

fn phase(n: f64) -> FlightPhase {
    [
        FlightPhase::Preflight,
        FlightPhase::Takeoff,
        FlightPhase::Climb,
        FlightPhase::Cruise,
        FlightPhase::Descent,
        FlightPhase::Approach,
        FlightPhase::GoAround,
        FlightPhase::Done,
    ][n as usize]
}

/// Parameters from the end-of-run snapshot, with the values that change in flight taken from the dump.
fn inflight_snapshot<'g>(golden: &'g Value, d: &Value) -> &'g Value {
    let label = if d["label"] == "PREFLIGHT" { "weights" } else { "in-flight" };
    golden["snapshots"].as_array().unwrap().iter().find(|s| s["label"] == label).unwrap()
}

fn inflight_params(golden: &Value, d: &Value) -> ComputationParameters {
    let snap = inflight_snapshot(golden, d);
    let mut p = params_from_golden(&snap["state"]);
    let dp = &d["params"];
    p.flight_phase = phase(num(dp, "flightPhase"));
    p.present_altitude = num(&dp["presentPosition"], "alt");
    p.fuel_on_board = num(dp, "fuelOnBoard");
    p.approach_speed = num(dp, "approachSpeed");
    p.cruise_altitude = num(dp, "cruiseAltitude");
    p.fcu_altitude = num(dp, "fcuAltitude");
    p.isa_deviation = num(&snap["state"]["guidance"]["atmosphericConditions"], "computedIsaDeviation");
    p
}

/// A plan with no constraints in the given distance frame (the canned route has none); KJFK gives the final altitude.
fn frame_reader(total: f64, distance_to_present_position: f64) -> ConstraintReader {
    let leg = ConstraintLeg {
        cumulative_distance_with_transitions: Some(0.0),
        cumulative_distance_to_end_with_transitions: Some(total),
        constraint_type: ConstraintType::Unknown,
        altitude_constraint: None,
        speed_constraint: None,
        cruise_step: None,
        vertical_angle: None,
        is_final_approach_fix: false,
    };
    let plan = ConstraintPlan {
        elements: vec![Some(leg)],
        active_leg_index: 1,
        first_missed_approach_leg_index: 1,
        destination_leg_index: None,
        missed_approach_point_altitude: None,
        destination_runway_threshold_altitude: None,
        destination_airport_altitude: Some(12.625),
    };
    let mut reader = ConstraintReader::default();
    reader.update_flight_plan(&plan, total - distance_to_present_position);
    reader
}

/// The wind plan for an in-flight dump. With `Some(total)` each leg ends `cumulativeDistanceToEnd` before `total` (the frame
/// of a profile computed before the snapshot; ENE and KJFK have the same distances to the end at every moment). With `None`
/// the legs keep the snapshot's `cumulativeDistance`, which for legs already flown is stale geometry from before sequencing:
/// FBW's `FlightPlanTrackProfile` searches from leg 0, so after sequencing it takes the track of a flown leg.
fn inflight_winds(golden: &Value, d: &Value, total: Option<f64>) -> WindPlan {
    let snap = inflight_snapshot(golden, d);
    let mut plan = wind_plan(&snap["state"]);
    let legs = snap["state"]["plans"]["active"]["legs"].as_array().unwrap();
    for (e, leg) in plan.elements.iter_mut().zip(legs) {
        if let Some(e) = e.as_mut() {
            e.cumulative_distance_with_transitions = match total {
                Some(total) => total - num(&leg["calc"], "cumulativeDistanceToEnd"),
                None => num(&leg["calc"], "cumulativeDistance"),
            };
        }
    }
    plan.elements[3].as_mut().unwrap().cruise_wind_entries = vec![WindEntry { vector: WindVector::from_degrees(90.0, 300.0), altitude: Some(36000.0) }];
    plan
}

fn checkpoints_from(v: &Value) -> Vec<Checkpoint> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|c| Checkpoint {
            reason: reason_from(c["reason"].as_str().unwrap()),
            distance_from_start: num(c, "distanceFromStart"),
            seconds_from_present: num(c, "secondsFromPresent"),
            altitude: num(c, "altitude"),
            remaining_fuel_on_board: num(c, "remainingFuelOnBoard"),
            speed: num(c, "speed"),
            mach: num(c, "mach"),
            profile_phase: [ProfilePhase::Climb, ProfilePhase::Cruise, ProfilePhase::Descent][num(c, "profilePhase") as usize],
            target_speed: None,
        })
        .collect()
}

fn reason_from(name: &str) -> Reason {
    use Reason::*;
    let all = [
        Liftoff, ThrustReductionAltitude, AccelerationAltitude, TopOfClimb, AtmosphericConditions, PresentPosition, LevelOffForClimbConstraint,
        AltitudeConstraint, ContinueClimb, CrossingClimbSpeedLimit, SpeedConstraint, CrossingFcuAltitudeClimb, StepClimb, TopOfStepClimb,
        StepDescent, BottomOfStepDescent, CrossingFcuAltitudeDescent, InterceptDescentProfileManaged, InterceptDescentProfileSelected,
        LevelOffForDescentConstraint, ContinueDescent, ContinueDescentArmed, TopOfDescent, CrossingDescentSpeedLimit,
        IdlePathAtmosphericConditions, IdlePathEnd, GeometricPathStart, GeometricPathConstraint, GeometricPathTooSteep, GeometricPathEnd,
        StartDecelerationToConstraint, StartDecelerationToLimit, Decel, Flaps1, Flaps2, Flaps3, FlapsFull, Landing,
    ];
    *all.iter().find(|r| format!("{r:?}") == name).unwrap_or_else(|| panic!("unknown reason {name}"))
}

/// The descent profile FBW keeps for guidance, in the frame it was computed in (its Landing checkpoint is the plan total then).
fn inflight_descent_profile<'a>(cfg: &'a AircraftConfig, golden: &Value, label: &str) -> (Profile<'a>, Vec<Value>) {
    let d = dump(golden, label);
    let recorded = d["descentCheckpoints"].as_array().unwrap().clone();
    let total = num(recorded.last().unwrap(), "distanceFromStart");
    let p = inflight_params(golden, &d);
    let reader = frame_reader(total, 0.0);
    let plan = inflight_winds(golden, &d, Some(total));
    let path = fms_vnav::manager::compute_descent_path(&p, cfg, &reader, Box::new(WindProfile::new(plan, None, WindFixes::FBW)), None);
    (path.profile, recorded)
}

#[test]
fn inflight_guidance_descent_profile() {
    let Some(golden) = load_named(INFLIGHT) else { return };
    let cfg = AircraftConfig::a380();
    // CRUISE and DESCENT keep a descent profile computed in cruise, with approach speeds the oracle did not snapshot.
    for label in ["PREFLIGHT"] {
        let (profile, recorded) = inflight_descent_profile(&cfg, &golden, label);
        let built = Built { tod: profile.checkpoints[0], profile, coordinator: CruiseToDescentCoordinator::default() };
        compare_checkpoints(&format!("{INFLIGHT} {label} guidance"), &recorded, &built);
    }
}

#[test]
fn inflight_descent_phase_tactical_profile() {
    let Some(golden) = load_named(INFLIGHT) else { return };
    let cfg = AircraftConfig::a380();
    let d = dump(&golden, "DESCENT");
    let p = inflight_params(&golden, &d);
    let gcp = d["checkpoints"].as_array().unwrap().clone();
    let pp = &gcp[0];
    let total = num(&d, "totalFlightPlanDistance");
    let reader = frame_reader(total, num(pp, "distanceFromStart"));
    let mut descent = Profile::new(&cfg, Box::new(ConstantWinds::default()));
    descent.checkpoints = checkpoints_from(&d["descentCheckpoints"]);
    let tod = descent.checkpoints.iter().find(|c| c.reason == Reason::TopOfDescent).unwrap().distance_from_start;
    let rel = &d["relation"];
    let distance_from_start = tod - num(rel, "distanceToTopOfDescent");
    let gps = descent.checkpoints.iter().find(|c| c.reason == Reason::GeometricPathStart).unwrap().distance_from_start;
    let relation = DescentProfileRelation {
        is_valid: rel["isValid"].as_bool().unwrap(),
        distance_from_start,
        past_top_of_descent: rel["pastTod"].as_bool().unwrap(),
        linear_deviation: p.present_altitude - descent.interpolate_altitude_at_distance(distance_from_start),
        on_geometric_path: distance_from_start > gps,
        current_target_path_angle: descent.interpolate_path_angle_at_distance(distance_from_start),
        above_speed_limit_altitude: p.descent_speed_limit.is_some_and(|l| p.present_altitude > l.under_altitude),
        close_to_airfield_elevation: p.present_altitude < p.destination_elevation + 5000.0,
    };
    let tactical = TacticalInputs {
        modes: Default::default(),
        present: PresentState { indicated_airspeed: num(pp, "speed"), vman: 0.0 },
        relation,
        descent_profile: Some(&descent),
    };
    let plan = inflight_winds(&golden, &d, None);
    let mut coordinator = CruiseToDescentCoordinator::default();
    let path = compute_tactical_mcdu_path(&p, &cfg, &reader, Box::new(WindProfile::new(plan, None, WindFixes::FBW)), &mut coordinator, None, &tactical).unwrap();
    let built = Built { tod: path.profile.checkpoints[0], profile: path.profile, coordinator };
    compare_checkpoints(&format!("{INFLIGHT} DESCENT tactical"), &gcp, &built);
}

/// `VnavDriver.recompute` on the preflight golden: two recomputations (the oracle's count before the NOWIND dump) give the
/// recorded MCDU profile; the guidance descent profile, relation and ND profile come along.
#[test]
fn vnav_driver_preflight_matches_golden() {
    use fms_vnav::driver::{RecomputeInputs, VnavDriver, WindsFor};
    let Some(golden) = load() else { return };
    let snap = golden["snapshots"].as_array().unwrap().iter().find(|s| s["label"] == "predictions-no-wind").unwrap();
    let state = &snap["state"];
    let params = params_from_golden(state);
    let d = dump(&golden, "NOWIND");
    let cfg = AircraftConfig::a380();
    let plan = constraint_plan(state);
    let total = num(&d, "totalFlightPlanDistance");
    let legs: Vec<(usize, bool)> = (0..plan.elements.len()).map(|i| (i, plan.elements[i].is_some())).collect();
    let winds = |_: WindsFor| -> Box<dyn Winds> { Box::new(ConstantWinds::default()) };
    let inputs = RecomputeInputs {
        modes: Default::default(),
        plan: &plan,
        distance_to_end: total - num(&d, "distanceToPresentPosition"),
        legs: &legs,
        active_leg_index: plan.active_leg_index,
        has_temporary: false,
        present: Default::default(),
        green_dot_speed: 0.0,
        approach_qnh: 1013.25,
        approach_temperature: 0.0,
        estimated_destination_fuel: None,
        winds: &winds,
    };
    let mut driver = VnavDriver::new(&cfg);
    driver.recompute(&params, &inputs);
    driver.recompute(&params, &inputs);
    assert_eq!(driver.version, 2);
    assert!(driver.out.fm_vertical_profile_avail);
    assert!(driver.relation.is_valid);
    let tod = driver.perf_crz_to_prediction().unwrap();
    assert_eq!(tod.reason, Reason::TopOfDescent);
    let nd = driver.nd_profile.as_ref().unwrap();
    assert!(nd.is_ready_to_display);
    assert_eq!(nd.checkpoints.len(), 1, "no climb or descent mode engaged on the ground");
    let guidance_tod = driver.descent_profile.as_ref().unwrap().find_vertical_checkpoint(&[Reason::TopOfDescent]).unwrap().distance_from_start;
    assert!((driver.relation.distance_to_top_of_descent().unwrap() - (guidance_tod - num(&d, "distanceToPresentPosition"))).abs() < 1e-6);
    let gcp = d["checkpoints"].as_array().unwrap().clone();
    let profile = driver.mcdu_profile.take().unwrap();
    let built = Built { tod: *profile.find_vertical_checkpoint(&[Reason::TopOfDescent]).unwrap(), profile, coordinator: CruiseToDescentCoordinator::default() };
    compare_checkpoints("predictions-egll-kjfk driver", &gcp, &built);
}

use fms_vnav::config::{AircraftConfig, FlapConf};
use fms_vnav::constraints::ConstraintReader;
use fms_vnav::manager::compute_descent_path;
use fms_vnav::profile::*;

const KG: f64 = 2.20462;

fn params() -> ComputationParameters {
    ComputationParameters {
        present_altitude: 34000.0,
        fcu_altitude: 34000.0,
        fcu_speed: -1.0,
        managed_climb_speed: 300.0,
        managed_climb_speed_mach: 0.84,
        managed_cruise_speed: 310.0,
        managed_cruise_speed_mach: 0.85,
        managed_descent_speed: 289.0,
        managed_descent_speed_mach: 0.84,
        zero_fuel_weight: 300_000.0 * KG,
        fuel_on_board: 36_000.0 * KG,
        v2_speed: 150.0,
        tropopause: 36090.0,
        perf_factor: 0.0,
        departure_elevation: 20.0,
        destination_elevation: 62.0,
        acceleration_altitude: 1580.0,
        thrust_reduction_altitude: 1580.0,
        cruise_altitude: 34000.0,
        climb_speed_limit: Some(SpeedLimit { speed: 250.0, under_altitude: 10000.0 }),
        descent_speed_limit: Some(SpeedLimit { speed: 250.0, under_altitude: 10000.0 }),
        flight_phase: FlightPhase::Descent,
        preselected_clb_speed: -1.0,
        preselected_cruise_speed: -1.0,
        takeoff_flaps: FlapConf::Conf1,
        takeoff_toga_n1: 90.0,
        isa_deviation: 0.0,
        approach_speed: 135.0,
        flap_retraction_speed: 165.0,
        slat_retraction_speed: 185.0,
        clean_speed: 210.0,
        gross_weight_valid: true,
    }
}

struct StarTable {
    total: f64,
    imped: f64,
    db520: f64,
    db518: f64,
    vuton: f64,
    db515: f64,
    db514: f64,
    db508: f64,
}

const SHORT_TABLE: StarTable = StarTable { total: 147.3, imped: 90.9, db520: 83.1, db518: 74.5, vuton: 66.5, db515: 62.5, db514: 58.1, db508: 36.7 };
const INFLATED_TABLE: StarTable = StarTable { total: 203.0, imped: 145.5, db520: 137.7, db518: 129.1, vuton: 121.1, db515: 117.1, db514: 117.1, db508: 65.0 };

fn at_or_below(total: f64, shift: f64, to_end: f64, altitude: f64, leg: usize) -> DescentAltitudeConstraint {
    DescentAltitudeConstraint {
        distance_from_start: total - to_end + shift,
        constraint: AltitudeConstraint { descriptor: AltitudeDescriptor::AtOrBelowAlt1, altitude1: altitude, altitude2: 0.0 },
        leg,
    }
}

fn at_or_above(total: f64, shift: f64, to_end: f64, altitude: f64, leg: usize) -> DescentAltitudeConstraint {
    DescentAltitudeConstraint {
        distance_from_start: total - to_end + shift,
        constraint: AltitudeConstraint { descriptor: AltitudeDescriptor::AtOrAboveAlt1, altitude1: altitude, altitude2: 0.0 },
        leg,
    }
}

fn speed(total: f64, shift: f64, to_end: f64, max_speed: f64) -> MaxSpeedConstraint {
    MaxSpeedConstraint { distance_from_start: total - to_end + shift, max_speed }
}

fn reader(t: &StarTable, shift: f64, with_speeds: bool) -> ConstraintReader {
    let mut r = ConstraintReader::default();
    r.total_flight_plan_distance = t.total + shift;
    r.final_altitude = 110.0;
    r.faf_distance_to_end = 6.0;
    let total = t.total;
    r.descent_altitude_constraints = vec![
        at_or_below(total, shift, t.imped, 12000.0, 1),
        at_or_below(total, shift, t.db520, 11000.0, 2),
        at_or_below(total, shift, t.db518, 10000.0, 3),
        at_or_below(total, shift, t.vuton, 8000.0, 4),
        at_or_below(total, shift, t.db514, 6000.0, 5),
        at_or_above(total, shift, 15.6, 2000.0, 6),
        at_or_above(total, shift, 11.0, 2000.0, 7),
    ];
    if with_speeds {
        r.descent_speed_constraints = vec![
            speed(total, shift, t.imped, 230.0),
            speed(total, shift, t.db520, 230.0),
            speed(total, shift, t.db515, 210.0),
            speed(total, shift, t.db508, 185.0),
            speed(total, shift, 15.6, 185.0),
        ];
    }
    r
}

struct Outcome {
    reader: ConstraintReader,
    checkpoints: Vec<Checkpoint>,
    too_steep: Vec<usize>,
}

fn run(p: &ComputationParameters, r: ConstraintReader) -> Outcome {
    let cfg = AircraftConfig::a380();
    let path = compute_descent_path(p, &cfg, &r, Box::new(ConstantWinds::default()), None);
    Outcome { checkpoints: path.profile.checkpoints.clone(), too_steep: path.too_steep_path_legs, reader: r }
}

fn dte(res: &Outcome, c: &Checkpoint) -> f64 {
    res.reader.total_flight_plan_distance - c.distance_from_start
}

fn altitude_at_dte(res: &Outcome, d: f64) -> f64 {
    let cps = &res.checkpoints;
    let target = res.reader.total_flight_plan_distance - d;
    for w in cps.windows(2) {
        if target > w[0].distance_from_start && target <= w[1].distance_from_start {
            let f = (target - w[0].distance_from_start) / (w[1].distance_from_start - w[0].distance_from_start);
            return w[0].altitude + f * (w[1].altitude - w[0].altitude);
        }
    }
    cps.last().unwrap().altitude
}

fn assert_physical(res: &Outcome, label: &str) {
    assert!(res.too_steep.is_empty(), "{label}: too steep legs {:?}", res.too_steep);
    let cps = &res.checkpoints;
    assert!(cps.len() > 10, "{label}: {} checkpoints", cps.len());
    let mut max_gradient: f64 = 0.0;
    for w in cps.windows(2) {
        let dd = w[1].distance_from_start - w[0].distance_from_start;
        let da = w[1].altitude - w[0].altitude;
        assert!(dd >= -1e-9, "{label}: checkpoints out of order at dte {:.2}", dte(res, &w[0]));
        assert!(da <= 1.0, "{label}: climbs {:.0} ft between dte {:.2} and {:.2}", da, dte(res, &w[0]), dte(res, &w[1]));
        if dd < 0.05 {
            assert!(da.abs() <= 1.0, "{label}: vertical step of {:.0} ft at dte {:.2}", da, dte(res, &w[0]));
        } else {
            max_gradient = max_gradient.max(-da / dd);
        }
    }
    assert!(max_gradient < 600.0, "{label}: steepest segment {max_gradient:.0} ft/nm");

    for c in &res.reader.descent_altitude_constraints {
        let d = res.reader.total_flight_plan_distance - c.distance_from_start;
        let alt = altitude_at_dte(res, d);
        match c.constraint.descriptor {
            AltitudeDescriptor::AtOrBelowAlt1 => assert!(alt <= c.constraint.altitude1 + 10.0, "{label}: {alt:.0} ft at dte {d:.1} breaks <= {}", c.constraint.altitude1),
            AltitudeDescriptor::AtOrAboveAlt1 => assert!(alt >= c.constraint.altitude1 - 10.0, "{label}: {alt:.0} ft at dte {d:.1} breaks >= {}", c.constraint.altitude1),
            _ => {}
        }
    }

    let tod = cps.iter().find(|c| c.reason == Reason::TopOfDescent).expect("top of descent");
    assert!((tod.altitude - 34000.0).abs() < 5.0, "{label}: TOD at {:.0} ft", tod.altitude);
    let mut previous = (dte(res, tod), tod.altitude);
    let mut first_leg_gradient = 0.0_f64;
    for c in cps.iter().rev().skip(1) {
        let d = dte(res, c);
        if previous.0 - d > 7.5 {
            break;
        }
        if previous.0 - d > 0.05 {
            first_leg_gradient = first_leg_gradient.max((previous.1 - c.altitude) / (previous.0 - d));
        }
        previous = (d, c.altitude);
    }
    assert!(first_leg_gradient < 420.0, "{label}: {first_leg_gradient:.0} ft/nm in the first 8 nm after TOD");
}

#[test]
fn star_descent_path_is_continuous_and_gradual() {
    for (label, table) in [("short table", &SHORT_TABLE), ("inflated table", &INFLATED_TABLE)] {
        for with_speeds in [false, true] {
            let res = run(&params(), reader(table, 0.0, with_speeds));
            assert_physical(&res, &format!("{label}, speeds {with_speeds}"));
        }
    }
}

#[test]
fn star_descent_path_is_continuous_across_weights_and_approach_speeds() {
    for zfw in [250.0, 300.0, 350.0] {
        for (vapp, f, s, gd) in [(130.0, 160.0, 180.0, 200.0), (135.0, 165.0, 185.0, 210.0), (150.0, 175.0, 200.0, 230.0)] {
            let mut p = params();
            p.zero_fuel_weight = zfw * 1000.0 * KG;
            p.approach_speed = vapp;
            p.flap_retraction_speed = f;
            p.slat_retraction_speed = s;
            p.clean_speed = gd;
            let res = run(&p, reader(&SHORT_TABLE, 0.0, true));
            assert_physical(&res, &format!("zfw {zfw}, speeds {vapp}/{f}/{s}/{gd}"));
        }
    }
}

#[test]
fn star_descent_path_only_depends_on_distance_to_end() {
    let a = run(&params(), reader(&SHORT_TABLE, 0.0, true));
    let b = run(&params(), reader(&SHORT_TABLE, 1234.5, true));
    assert_eq!(a.checkpoints.len(), b.checkpoints.len());
    for (x, y) in a.checkpoints.iter().zip(&b.checkpoints) {
        assert!((dte(&a, x) - dte(&b, y)).abs() < 1e-6, "{} vs {}", dte(&a, x), dte(&b, y));
        assert!((x.altitude - y.altitude).abs() < 1e-6);
        assert_eq!(x.reason, y.reason);
    }
}

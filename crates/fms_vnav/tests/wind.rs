use fms_vnav::profile::Winds;
use fms_vnav::wind::*;

fn entry(alt: f64, mag: f64, dir_deg: f64) -> WindEntry {
    WindEntry { vector: WindVector::from_degrees(mag, dir_deg), altitude: Some(alt) }
}

fn plan() -> WindPlan {
    WindPlan {
        elements: vec![
            Some(WindLeg { cumulative_distance_with_transitions: 0.0, distance_with_transitions: 0.0, true_track: None, calculated: true, cruise_wind_entries: vec![] }),
            Some(WindLeg { cumulative_distance_with_transitions: 100.0, distance_with_transitions: 100.0, true_track: Some(90.0), calculated: true, cruise_wind_entries: vec![] }),
            Some(WindLeg { cumulative_distance_with_transitions: 300.0, distance_with_transitions: 200.0, true_track: Some(270.0), calculated: true, cruise_wind_entries: vec![entry(36000.0, 90.0, 300.0)] }),
        ],
        first_missed_approach_leg_index: 3,
        max_cruise_wind_levels: 4,
        climb_wind_entries: vec![entry(30000.0, 70.0, 290.0), entry(10000.0, 35.0, 280.0), entry(0.0, 15.0, 270.0)],
        descent_wind_entries: vec![entry(30000.0, 60.0, 300.0), entry(10000.0, 30.0, 290.0), entry(0.0, 12.0, 280.0)],
        pilot_trip_wind: None,
        origin_altitude: 0.0,
        destination_altitude: 0.0,
    }
}

fn vector(f: Forecast) -> WindVector {
    match f {
        Forecast::Vector(v) => v,
        Forecast::Tailwind(_) => panic!("vector expected"),
    }
}

#[test]
fn a380_climb_winds_scale_the_highest_entry_as_fbw_does() {
    let fbw = WindProfile::new(plan(), None, WindFixes::FBW);
    let v = vector(fbw.climb_wind_forecast(5000.0));
    let k = 5000.0 / 30000.0;
    assert!((v.magnitude.unwrap() - 70.0 * k).abs() < 1e-9);
    assert!((v.direction.unwrap().to_degrees() - 290.0 * k).abs() < 1e-9);
}

#[test]
fn fixed_climb_winds_use_the_lowest_entry_and_keep_the_direction() {
    let fixed = WindProfile::new(plan(), None, WindFixes::ALL);
    let v = vector(fixed.climb_wind_forecast(5000.0));
    assert!((v.magnitude.unwrap() - 25.0).abs() < 1e-9);
    assert!((v.direction.unwrap().to_degrees() - 275.0).abs() < 1e-9);
    let mut p = plan();
    p.origin_altitude = 1000.0;
    p.climb_wind_entries = vec![entry(30000.0, 70.0, 290.0), entry(10000.0, 30.0, 280.0)];
    let below = vector(WindProfile::new(p, None, WindFixes::ALL).climb_wind_forecast(5500.0));
    assert!((below.magnitude.unwrap() - 15.0).abs() < 1e-9);
    assert!((below.direction.unwrap().to_degrees() - 280.0).abs() < 1e-9);
}

#[test]
fn descent_winds_interpolate_between_entries() {
    let w = WindProfile::new(plan(), None, WindFixes::FBW);
    let v = vector(w.descent_wind_forecast(20000.0));
    assert!((v.magnitude.unwrap() - 45.0).abs() < 1e-9);
    assert!((v.direction.unwrap().to_degrees() - 295.0).abs() < 1e-9);
}

#[test]
fn shortest_arc_fix_across_north() {
    let a = WindVector::from_degrees(10.0, 350.0);
    let b = WindVector::from_degrees(10.0, 10.0);
    let fbw = interpolate_wind_vector(0.5, 0.0, 1.0, a, b, WindFixes::FBW);
    let fixed = interpolate_wind_vector(0.5, 0.0, 1.0, a, b, WindFixes::ALL);
    assert!((fbw.direction.unwrap().to_degrees() - 180.0).abs() < 1e-9);
    assert!(fixed.direction.unwrap().to_degrees().abs() < 1e-9 || (fixed.direction.unwrap().to_degrees() - 360.0).abs() < 1e-9);
}

#[test]
fn tailwind_follows_the_leg_track() {
    let w = WindProfile::new(plan(), None, WindFixes::FBW);
    let tail = w.descent_tailwind(200.0, 20000.0);
    let expected = -45.0 * (295.0_f64 - 270.0).to_radians().cos();
    assert!((tail - expected).abs() < 1e-9);
    assert_eq!(w.leg_index(50.0), 1);
    assert_eq!(w.leg_index(250.0), 2);
}

#[test]
fn cruise_winds_propagate_and_interpolate_along_legs() {
    let w = WindProfile::new(plan(), None, WindFixes::FBW);
    assert_eq!(w.plan.propagate_winds_at(1), vec![entry(36000.0, 90.0, 300.0)]);
    let v = vector(w.cruise_wind_forecast(200.0, 36000.0));
    assert!((v.magnitude.unwrap() - 90.0).abs() < 1e-9);
}

use ops::aircraft::*;
use ops::aoc::{gate_from_spot, AocSim, Event, Flight, Kind, Oooi};
use ops::boarding::{doors_in_use, Boarding};
use ops::loadsheet::{Load, Loadsheet, Moment};
use ops::refuel::{distribute, rate_kg_per_s, Refuel};
use ops::settings::{airline_units, FuelSource, OpsSettings, Speed, Units, UnitsChoice};
use ops::trim::{cg_for_ths, ths_for_cg, ths_in_cyan_band};

fn r2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

#[test]
fn tables_match_fbw() {
    assert_eq!(total_seats(), 484);
    assert_eq!(PAX_ZONES.iter().filter(|z| z.deck == Deck::Upper).map(|z| z.seats).sum::<u32>(), 90);
    assert_eq!(HOLDS.iter().map(|h| h.max_kg).sum::<f64>(), 51_400.0);
    assert!((fuel_capacity_kg() - 85_471.7 * FUEL_GALLONS_TO_KG).abs() < 0.1);
}

#[test]
fn empty_aircraft_cg_matches_fbw_airframe_test() {
    let ls = Loadsheet::compute(&Load::default());
    assert_eq!(r2(ls.zfw_cg), 36.23);
    assert_eq!(r2(ls.gw_cg), 36.23);
    assert_eq!(ls.zfw_kg, OEW_KG);
}

#[test]
fn fbw_high_fuel_full_pax_full_cargo_case() {
    let oew = Moment::of(OEW_KG, OEW_ARM_FT);
    let zfw = oew.add(Moment::of(174.0 * PAX_KG, -9.7966)).add(Moment::of(9435.0, -14.3122));
    let gw = zfw.add(Moment::of(5600.0 * FUEL_GALLONS_TO_KG, -8.2951));
    let (z, g) = (r2(zfw.percent_mac()), r2(gw.percent_mac()));
    assert!(z > 39.0 && z < 40.0, "{z}");
    assert!(g > 41.0 && g < 42.0, "{g}");
}

#[test]
fn fbw_low_fuel_half_pax_case() {
    let zfw = Moment::of(OEW_KG, OEW_ARM_FT).add(Moment::of(87.0 * PAX_KG, -9.7966));
    let gw = zfw.add(Moment::of(948.0 * FUEL_GALLONS_TO_KG, -10.8165));
    assert!(zfw.percent_mac() > 37.0 && zfw.percent_mac() < 38.0);
    assert!(gw.percent_mac() > 37.0 && gw.percent_mac() < 38.0);
}

#[test]
fn full_zone_load_sums_every_station() {
    let load = Load { pax: Load::full_cabin(), cargo_kg: [28_577.0, 20_310.0, 2_513.0], ..Load::default() };
    let ls = Loadsheet::compute(&load);
    let pax_ft = 56.0 * 75.7 + 132.0 * 30.6 + 124.0 * -11.1 + 82.0 * -46.9 + 14.0 * 60.8 + 58.0 * 11.7 + 18.0 * -29.3;
    let moment = OEW_KG * OEW_ARM_FT + pax_ft * 84.0 + 28_577.0 * 67.4 + 20_310.0 * -18.5 + 2_513.0 * -52.9;
    let kg = OEW_KG + 484.0 * 84.0 + 51_400.0;
    assert!((ls.zfw_kg - kg).abs() < 1e-6);
    assert!((ls.zfw_cg - -100.0 * (moment / kg - LEMAC_FT) / MAC_FT).abs() < 1e-9);
    assert_eq!(r2(ls.zfw_cg), 28.88);
    assert_eq!((ls.pax, ls.pax_main, ls.pax_upper), (484, 394, 90));
    assert!(ls.over_mzfw && !ls.within_limits());
}

#[test]
fn typical_load_within_limits_and_trim_from_cg() {
    let mut load = Load { pax: Load::full_cabin(), cargo_kg: [12_000.0, 9_000.0, 1_000.0], taxi_fuel_kg: 1_200.0, trip_fuel_kg: 110_000.0, ..Load::default() };
    load.fuel_kg = distribute(140_000.0, 0.0);
    let ls = Loadsheet::compute(&load);
    assert!(ls.within_limits(), "{ls:?}");
    assert!((ls.tow_kg - (ls.gw_kg - 1_200.0)).abs() < 1e-6);
    assert!((ls.ths_deg - ths_for_cg(ls.tow_cg)).abs() < 1e-12);
    assert!(ls.zfw_cg > 23.0 && ls.zfw_cg < 48.0);
}

#[test]
fn ths_table_is_fbw_pitch_trim_utils() {
    assert_eq!(ths_for_cg(20.0), 5.8);
    assert_eq!(ths_for_cg(29.0), 5.8);
    assert!((ths_for_cg(34.5) - 4.68).abs() < 1e-12);
    assert!((ths_for_cg(43.0) - -0.18).abs() < 1e-12);
    assert_eq!(ths_for_cg(45.0), -0.18);
    for cg in [30.0, 33.0, 36.0, 40.0, 42.5] {
        assert!((cg_for_ths(ths_for_cg(cg)) - cg).abs() < 1e-9);
    }
    assert!(ths_in_cyan_band(36.0, ths_for_cg(36.0)));
    assert!(!ths_in_cyan_band(36.0, 6.0));
}

#[test]
fn boarding_time_scales_with_doors() {
    let target = Load::full_cabin();
    let time = |doors: Vec<Door>| {
        let mut b = Boarding::new([0; 14], [0.0; 3], doors, Speed::Realistic);
        b.set_target(target, [0.0; 3]);
        b.estimate_seconds()
    };
    let one = time(vec![Door::M1L]);
    let two = time(doors_in_use(&[Door::M1L], &[Door::M2L]));
    let three = time(doors_in_use(&[Door::M1L, Door::M2L, Door::U1L], &[]));
    assert_eq!(one, 484.0 * 5.0);
    assert_eq!(two, 242.0 * 5.0);
    assert!((three - (484f64 / 3.0).ceil() * 5.0).abs() < 1e-9);
    assert_eq!(time(vec![]), one);
}

#[test]
fn boarding_steps_to_target_and_deboards() {
    let mut b = Boarding::new([0; 14], [0.0; 3], vec![Door::M1L, Door::M2L], Speed::Fast);
    b.set_target(Load::full_cabin(), [600.0, 0.0, 120.0]);
    let est = b.estimate_seconds();
    let mut t = 0.0;
    while !b.step(0.1) {
        t += 0.1;
        assert!(t < 10_000.0);
    }
    assert!((t - est).abs() < 0.5, "{t} vs {est}");
    assert_eq!(b.pax, Load::full_cabin());
    assert_eq!(b.cargo_kg, [600.0, 0.0, 120.0]);
    b.set_target([0; 14], [0.0; 3]);
    b.speed = Speed::Instant;
    assert!(b.step(0.0));
    assert_eq!(b.pax, [0; 14]);
}

#[test]
fn boarding_fills_zones_in_door_order() {
    let mut b = Boarding::new([0; 14], [0.0; 3], vec![Door::M2L], Speed::Realistic);
    b.set_target(Load::full_cabin(), [0.0; 3]);
    b.step(5.0 * 28.0);
    assert_eq!(b.pax[1], 28);
    assert_eq!(b.pax.iter().sum::<u32>(), 28);
}

#[test]
fn refuel_distribution_and_totals() {
    for total in [10_000.0, 22_000.0, 40_000.0, 120_000.0, 200_000.0, 240_000.0] {
        let d = distribute(total, 0.0);
        assert!((d.iter().sum::<f64>() - total).abs() < 1e-6, "{total}");
        assert_eq!(d[FEED_1], d[FEED_4]);
        assert_eq!(d[LEFT_MID], d[RIGHT_MID]);
    }
    let d = distribute(10_000.0, 0.0);
    assert_eq!(d[FEED_1], 2_500.0);
    assert_eq!(d[LEFT_OUTER], 0.0);
    let with_trim = distribute(120_000.0, 6_000.0);
    assert_eq!(with_trim[TRIM], 6_000.0);
    assert!((with_trim.iter().sum::<f64>() - 120_000.0).abs() < 1e-6);
}

#[test]
fn refuel_rate_and_total_time() {
    let target = distribute(100_000.0, 0.0);
    let rate = rate_kg_per_s(Speed::Realistic, FuelSource::Hydrant).unwrap();
    assert!((rate - 16.0 * FUEL_GALLONS_TO_KG).abs() < 1e-12);
    let mut r = Refuel::new([0.0; TANK_COUNT], target, Speed::Realistic, FuelSource::Hydrant);
    let est = r.estimate_seconds();
    assert!((est - 100_000.0 / rate).abs() < 1e-6);
    let mut t = 0.0;
    while !r.step(1.0) {
        t += 1.0;
        let expected = (t * rate).min(100_000.0);
        assert!((r.total_kg() - expected).abs() < 1e-6, "{t}");
    }
    assert!((r.total_kg() - 100_000.0).abs() < 1e-6);
    assert!((t + 1.0 - est.ceil()).abs() <= 1.0);
    let fast = Refuel::new([0.0; TANK_COUNT], target, Speed::Fast, FuelSource::Hydrant).estimate_seconds();
    assert!((est / fast - 5.0).abs() < 1e-9);
    let mut instant = Refuel::new([0.0; TANK_COUNT], target, Speed::Instant, FuelSource::Truck);
    assert!(instant.step(0.0));
    assert_eq!(instant.tanks_kg, target);
    let mut defuel = Refuel::new(target, distribute(20_000.0, 0.0), Speed::Fast, FuelSource::Truck);
    while !defuel.step(1.0) {}
    assert!((defuel.total_kg() - 20_000.0).abs() < 1e-6);
}

#[test]
fn settings_units_and_aoc_switch() {
    let s = OpsSettings::default();
    assert_eq!(s.resolved_units(Some("UAE")), Units::Kg);
    assert_eq!(s.resolved_units(Some("ual")), Units::Lb);
    assert_eq!(s.resolved_units(None), Units::Kg);
    let s = OpsSettings { units: UnitsChoice::Lb, ..OpsSettings::default() };
    assert_eq!(s.resolved_units(Some("UAE")), Units::Lb);
    assert_eq!(airline_units("QFA"), Units::Kg);
    assert!((Units::Lb.from_kg(1000.0) - 2204.622_621_8).abs() < 1e-6);
    assert!(s.aoc_active(false) && !s.aoc_active(true));
}

#[test]
fn gate_names_from_parking_spots() {
    assert_eq!(gate_from_spot("Gate A12").as_deref(), Some("A12"));
    assert_eq!(gate_from_spot("Stand 305 Heavy").as_deref(), Some("305"));
    assert_eq!(gate_from_spot("Ramp 4").as_deref(), Some("R4"));
    assert_eq!(gate_from_spot("B7").as_deref(), Some("B7"));
    assert_eq!(gate_from_spot("Cargo Apron"), None);
}

#[test]
fn aoc_sequence_for_a_full_flight() {
    let flight = Flight {
        airline_icao: "UAE".into(),
        flight_number: "1".into(),
        origin: "OMDB".into(),
        destination: "EGLL".into(),
        std_s: Some(8.0 * 3600.0),
    };
    let mut aoc = AocSim::new();
    let mut log = Vec::new();
    let run = |aoc: &mut AocSim, from: f64, to: f64, log: &mut Vec<(f64, Kind, String)>| {
        let mut t = from;
        while t <= to {
            for u in aoc.poll(t) {
                log.push((t, u.kind, u.text));
            }
            t += 5.0;
        }
    };
    let t0 = 6.5 * 3600.0;
    aoc.on_event(t0, Event::FlightInitialised { flight, parking_spot: Some("Gate A1 - Heavy".into()) });
    run(&mut aoc, t0, t0 + 600.0, &mut log);
    aoc.on_event(t0 + 600.0, Event::BoardingStarted);
    let mut load = Load { pax: Load::full_cabin(), cargo_kg: [10_000.0, 8_000.0, 500.0], taxi_fuel_kg: 1_000.0, trip_fuel_kg: 80_000.0, ..Load::default() };
    load.fuel_kg = distribute(120_000.0, 0.0);
    let ls = Loadsheet::compute(&load);
    aoc.on_event(t0 + 3_000.0, Event::BoardingComplete { loadsheet: ls, units: Units::Kg });
    run(&mut aoc, t0 + 605.0, 8.0 * 3600.0 + 20.0 * 60.0, &mut log);
    let out = 8.0 * 3600.0 + 25.0 * 60.0;
    aoc.on_event(out, Event::Oooi { kind: Oooi::Out });
    aoc.on_event(out + 900.0, Event::Oooi { kind: Oooi::Off });
    aoc.on_event(out + 7.0 * 3600.0, Event::Oooi { kind: Oooi::On });
    aoc.on_event(out + 7.0 * 3600.0 + 300.0, Event::ArrivalGate { parking_spot: "Stand 509".into() });
    aoc.on_event(out + 7.0 * 3600.0 + 600.0, Event::Oooi { kind: Oooi::In });
    run(&mut aoc, out, out + 8.0 * 3600.0, &mut log);

    let kinds: Vec<Kind> = log.iter().map(|l| l.1).collect();
    assert_eq!(
        kinds,
        vec![Kind::Gate, Kind::Info, Kind::Loadsheet, Kind::Delay, Kind::OooiAck, Kind::OooiAck, Kind::OooiAck, Kind::Gate, Kind::OooiAck]
    );
    assert_eq!(log[0].2, "UAE1 OMDB-EGLL DEP GATE A1");
    assert!(log[2].2.starts_with("FINAL LOADSHEET UAE1\nOMDB-EGLL\nPAX 484 (MAIN 394 UPPER 90)"), "{}", log[2].2);
    assert!(log[2].2.contains("ZFWCG") && log[2].2.contains("THS"));
    assert!(log[3].2.starts_with("UAE1 DELAYED STD 0800"), "{}", log[3].2);
    assert_eq!(log[4].2, "UAE1 OUT 0825 RCVD");
    assert_eq!(log[7].2, "UAE1 ARR EGLL GATE 509");
    assert_eq!(aoc.pending(), 0);
}

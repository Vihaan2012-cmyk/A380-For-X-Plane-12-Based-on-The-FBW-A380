use fms_perf::constants::*;
use fms_perf::cost_index::*;
use fms_perf::managed::*;
use fms_perf::phase::*;
use fms_perf::speeds::*;
use fms_perf::takeoff::*;
use fms_perf::weights::*;
use fms_perf::{FlightPhase, PerfComputer, PerfInputs};

fn inp(phase: FlightPhase, mass: f64, flap: i32, cas: f64, alt: f64) -> OperatingSpeedsInput {
    OperatingSpeedsInput {
        mass_kg: mass,
        cg_percent_mac: 35.0,
        cas_kt: cas,
        flap_position: flap,
        phase,
        altitude_ft: alt,
        ..Default::default()
    }
}

#[test]
fn tables_interpolate_and_clamp() {
    assert_eq!(SpeedsLookupTables::s_speed(250_000.0), 146.0);
    assert_eq!(SpeedsLookupTables::s_speed(100_000.0), 146.0);
    assert_eq!(SpeedsLookupTables::s_speed(650_000.0), 215.0);
    assert!((SpeedsLookupTables::s_speed(275_000.0) - 153.0).abs() < 1e-9);
    assert_eq!(SpeedsLookupTables::approach_vls(1, 29.0, 250_000.0), 128.0);
    assert_eq!(SpeedsLookupTables::approach_vls(1, 43.0, 600_000.0), 195.0);
    assert!((SpeedsLookupTables::approach_vls(1, 36.0, 250_000.0) - 127.0).abs() < 1e-9);
    assert_eq!(SpeedsLookupTables::approach_vls(5, 10.0, 250_000.0), 123.0);
    assert_eq!(SpeedsLookupTables::vls_conf0(0.0, 250_000.0), 149.0);
    assert_eq!(SpeedsLookupTables::green_dot(10_000.0, 250_000.0), 160.0);
    assert_eq!(SpeedsLookupTables::green_dot(0.0, 250_000.0), 160.0);
    assert_eq!(SpeedsLookupTables::green_dot(45_000.0, 600_000.0), 260.0);
}

#[test]
#[should_panic]
fn approach_vls_rejects_invalid_conf() {
    SpeedsLookupTables::approach_vls(6, 35.0, 300_000.0);
}

#[test]
fn clean_config_speeds_in_cruise() {
    let s = OperatingSpeeds::compute(&inp(FlightPhase::Cruise, 400_000.0, 0, 280.0, 20_000.0));
    assert_eq!(s.vfe_next, VFE_F1);
    assert!((s.vls - 199.0).abs() < 1e-9);
    assert!((s.vs1g - 199.0 / 1.23).abs() < 1e-9);
    assert_eq!(s.vmax, 340.0_f64.min(vmo_knots(None)));
    assert_eq!(s.s, SpeedsLookupTables::s_speed(400_000.0));
    assert_eq!(s.gd, 219.0);
}

#[test]
fn vmax_is_limited_by_mmo_at_low_static_pressure() {
    let s = OperatingSpeeds::compute(&OperatingSpeedsInput {
        static_pressure_hpa: Some(200.0),
        ..inp(FlightPhase::Cruise, 400_000.0, 0, 280.0, 38_000.0)
    });
    assert!(s.vmax < 340.0);
    assert!((s.vmax - fms_perf::isa::mach_to_kcas(MMO, 200.0)).abs() < 1e-9);
}

#[test]
fn flap_one_above_212_kt_uses_conf_one_with_118_factor() {
    let hi = OperatingSpeeds::compute(&inp(FlightPhase::Climb, 400_000.0, 1, 230.0, 3_000.0));
    let lo = OperatingSpeeds::compute(&inp(FlightPhase::Climb, 400_000.0, 1, 200.0, 3_000.0));
    assert_eq!(hi.vmax, VFE_F1);
    assert_eq!(hi.vfe_next, VFE_F1F);
    assert_eq!(lo.vmax, VFE_F1F);
    assert_eq!(lo.vfe_next, VFE_F2);
    let conf1 = SpeedsLookupTables::approach_vls(1, 35.0, 400_000.0);
    assert!((hi.vls - (1.18 * conf1 / 1.23).max(VMCL)).abs() < 1e-9);
    let conf1f = SpeedsLookupTables::approach_vls(2, 35.0, 400_000.0);
    assert!((lo.vls - conf1f).abs() < 1e-9);
}

#[test]
fn full_flap_has_no_next_vfe_and_wind_correction_is_one_third_clamped() {
    let mut i = inp(FlightPhase::Approach, 350_000.0, 4, 140.0, 0.0);
    let calm = OperatingSpeeds::compute(&i);
    assert_eq!(calm.vfe_next, 0.0);
    assert_eq!(calm.vmax, VFE_FF);
    assert!((calm.vapp - (calm.vref + 5.0)).abs() < 1e-9);
    i.wind_kt = 30.0;
    assert!((OperatingSpeeds::compute(&i).vapp - calm.vref - 10.0).abs() < 1e-9);
    i.wind_kt = 90.0;
    assert!((OperatingSpeeds::compute(&i).vapp - calm.vref - 15.0).abs() < 1e-9);
    i.wind_kt = -20.0;
    assert!((OperatingSpeeds::compute(&i).vapp - calm.vref - 5.0).abs() < 1e-9);
}

#[test]
fn takeoff_phase_vls_and_retraction_speeds_come_from_vs1g() {
    let s = OperatingSpeeds::compute(&inp(FlightPhase::Takeoff, 450_000.0, 1, 100.0, 0.0));
    let vs1g = SpeedsLookupTables::approach_vls(2, 35.0, 450_000.0) / 1.23;
    assert!((s.vls - 1.15 * vs1g).abs() < 1e-9);
    assert!((s.s - 1.21 * SpeedsLookupTables::vls_conf0(0.0, 450_000.0) / 1.23).abs() < 1e-9);
    let vs1g_1f = SpeedsLookupTables::approach_vls(1, 35.0, 450_000.0) / 1.23;
    assert!((s.f2 - (1.18 * vs1g_1f).max(VMCL + 5.0)).abs() < 1e-9);
    assert_eq!(s.f2, s.f3);
}

#[test]
fn spoilers_raise_vls_in_proportion() {
    let base = inp(FlightPhase::Cruise, 400_000.0, 0, 280.0, 20_000.0);
    let clean = OperatingSpeeds::compute(&base);
    let half = OperatingSpeeds::compute(&OperatingSpeedsInput { left_spoiler_1_position: 10.0, ..base });
    assert!((half.vls - clean.vls - 12.5).abs() < 1e-9);
    let ignored = OperatingSpeeds::compute(&OperatingSpeedsInput {
        left_spoiler_1_position: 10.0,
        ignore_spoilers: true,
        ..base
    });
    assert_eq!(ignored.vls, clean.vls);
}

#[test]
fn headwind_and_gs_mini() {
    assert!((SpeedsUtils::headwind(20.0, 90.0, 90.0) - 20.0).abs() < 1e-9);
    assert!((SpeedsUtils::headwind(20.0, 270.0, 90.0) + 20.0).abs() < 1e-9);
    assert!((SpeedsUtils::headwind(20.0, 0.0, 90.0)).abs() < 1e-9);
    assert!((SpeedsUtils::headwind(20.0, 350.0, 10.0) - 20.0 * (20.0_f64).to_radians().cos()).abs() < 1e-9);
    assert_eq!(SpeedsUtils::headwind_diff(10.0, 40.0), 10.0);
    assert_eq!(SpeedsUtils::headwind_diff(40.0, 10.0), -10.0);
    assert_eq!(SpeedsUtils::vtarget_gs_mini(140.0, 10.0, 4, 182.0, 0.0), 150.0);
    assert_eq!(SpeedsUtils::vtarget_gs_mini(140.0, 60.0, 4, 182.0, 0.0), 177.0);
    assert_eq!(SpeedsUtils::vtarget_gs_mini(140.0, -10.0, 4, 182.0, 0.0), 140.0);
    assert_eq!(SpeedsUtils::vtarget_gs_mini(140.0, 60.0, 3, 196.0, 182.0), 182.0);
}

#[test]
fn vmca_vmcg_and_vs1g() {
    assert_eq!(SpeedsUtils::vmca(0.0), 114.0);
    assert_eq!(SpeedsUtils::vmca(1000.0), 114.0);
    assert!((SpeedsUtils::vmca(3000.0) - 113.5).abs() < 1e-9);
    assert_eq!(SpeedsUtils::vmca(20_000.0), 97.0);
    assert_eq!(SpeedsUtils::vmcg(0.0), 116.0);
    assert_eq!(SpeedsUtils::vmcg(-5000.0), 115.0);
    let w = 300_000.0;
    assert!((SpeedsUtils::vs1g(w, 35.0, 5, false) - (SpeedsLookupTables::approach_vls(5, 35.0, w) / 1.18).max(VMCL)).abs() < 1e-9);
    assert!((SpeedsUtils::vs1g(w, 35.0, 3, true) - SpeedsLookupTables::approach_vls(3, 35.0, w) / 1.15).abs() < 1e-9);
}

#[test]
fn perf_computer_matches_fallback_defaults() {
    let mut c = PerfComputer::new();
    let s = c.update_perf_speeds(&PerfInputs { pressure_altitude_ft: Some(83.0), destination_elevation_ft: 12.0, ..Default::default() });
    assert_eq!(s.vs1g, 148.0);
    assert_eq!(s.vls, 170.0);
    assert_eq!(s.flap_retraction, Some(149.0));
    assert_eq!(s.slat_retraction, Some(179.0));
    assert_eq!(s.green_dot, Some(197.0));
    assert_eq!(s.vmax, 340.0);
    assert_eq!(s.vfe_next, 263.0);
    assert_eq!(s.approach_vapp, Some(139.0));
    assert_eq!(s.approach_flap_retraction, Some(166.0));
}

#[test]
fn perf_computer_keeps_flap_speed_between_updates_and_pilot_vapp_wins() {
    let mut c = PerfComputer::new();
    let base = PerfInputs {
        phase: FlightPhase::Climb,
        gross_weight_kg: Some(350_000.0),
        gross_weight_cg: Some(33.0),
        pressure_altitude_ft: Some(5000.0),
        computed_airspeed_kt: Some(220.0),
        flap_lever: 2,
        ..Default::default()
    };
    let a = c.update_perf_speeds(&base);
    let f = a.flap_retraction.unwrap();
    assert!(f >= VMCL + 15.0);
    let b = c.update_perf_speeds(&PerfInputs { flap_lever: 1, pilot_vapp_kt: Some(150.0), ..base });
    assert_eq!(b.flap_retraction, Some(f));
    assert_eq!(b.approach_vapp, Some(150.0));
    let none = c.update_perf_speeds(&PerfInputs { pressure_altitude_ft: None, flap_lever: 0, ..base });
    assert_eq!(none.vs1g, b.vs1g);
}

#[test]
fn approach_phase_uses_actual_weight_and_flap3_selection() {
    let mut c = PerfComputer::new();
    let base = PerfInputs {
        phase: FlightPhase::Climb,
        estimated_landing_weight_kg: Some(300_000.0),
        gross_weight_kg: Some(450_000.0),
        gross_weight_cg: Some(35.0),
        pressure_altitude_ft: Some(3000.0),
        ..Default::default()
    };
    let est = c.update_perf_speeds(&base);
    let act = c.update_perf_speeds(&PerfInputs { phase: FlightPhase::Approach, ..base });
    assert!(act.approach_vls.unwrap() > est.approach_vls.unwrap());
    let f3 = c.update_perf_speeds(&PerfInputs { approach_flaps_three_selected: true, ..base });
    assert!(f3.approach_vls.unwrap() > est.approach_vls.unwrap());
    let no_ldg = c.update_perf_speeds(&PerfInputs { estimated_landing_weight_kg: None, ..base });
    assert_eq!(no_ldg.approach_vls, act.approach_vls);
}

#[test]
fn approach_wind_uses_runway_bearing_headwind() {
    let mut c = PerfComputer::new();
    let base = PerfInputs {
        gross_weight_kg: Some(300_000.0),
        gross_weight_cg: Some(35.0),
        pressure_altitude_ft: Some(0.0),
        approach_wind_direction_deg: Some(90.0),
        approach_wind_magnitude_kt: Some(45.0),
        destination_runway_magnetic_bearing_deg: Some(90.0),
        ..Default::default()
    };
    let head = c.update_perf_speeds(&base);
    let calm = c.update_perf_speeds(&PerfInputs { approach_wind_magnitude_kt: None, ..base });
    assert_eq!(head.approach_vapp.unwrap() - calm.approach_vapp.unwrap(), 10.0);
}

#[test]
fn takeoff_entry_limits_and_flex_encoding() {
    assert_eq!(parse_v_speed(90.0), Ok(90.0));
    assert_eq!(parse_v_speed(340.0), Ok(340.0));
    assert_eq!(parse_v_speed(89.0), Err(EntryError::OutOfRange { min: 90.0, max: 340.0 }));
    assert!(parse_v_speed(341.0).is_err());
    assert_eq!(parse_flex_temperature(0.0), Ok(0.0));
    assert!(parse_flex_temperature(100.0).is_err());
    assert!(parse_flex_temperature(-1.0).is_err());
    assert_eq!(parse_cost_index(999.0), Ok(999.0));
    assert!(parse_cost_index(1000.0).is_err());
    assert_eq!(flex_temp_lvar(Some(0.0)), 0.1);
    assert_eq!(flex_temp_lvar(Some(52.0)), 52.0);
    assert_eq!(flex_temp_lvar(None), 0.0);
    assert_eq!(flex_temp_lvar_for_setting(TakeoffPowerSetting::Toga, Some(52.0)), 0.0);
    assert_eq!(flex_temp_lvar_for_setting(TakeoffPowerSetting::Derated, Some(52.0)), 0.0);
    assert_eq!(flex_temp_lvar_for_setting(TakeoffPowerSetting::Flex, Some(0.0)), 0.1);
}

#[test]
fn takeoff_speed_checks_and_discrete_bits() {
    let ok = TakeoffSpeeds { v1: Some(140.0), vr: Some(145.0), v2: Some(150.0) };
    assert!(ok.valid());
    assert!(!ok.not_inserted());
    let bad = TakeoffSpeeds { v1: Some(150.0), vr: Some(145.0), v2: Some(150.0) };
    assert!(!bad.valid());
    let partial = TakeoffSpeeds { v1: Some(150.0), vr: None, v2: Some(145.0) };
    assert!(!partial.valid());
    assert!(partial.not_inserted());
    assert!(TakeoffSpeeds::default().valid());
    let w = ToSpeedsChecks::evaluate(&bad).discrete_word_3_bits(0);
    assert_eq!(w, 1 << 15 | 0);
    let w = ToSpeedsChecks::evaluate(&TakeoffSpeeds::default()).discrete_word_3_bits(0);
    assert_eq!(w, 1 << 17);
    assert_eq!(set_bit(0b1111, 2, false), 0b1101);
    assert_eq!(takeoff_flaps_discrete_word_2(0, Some(2)), 1 << 14);
    assert_eq!(takeoff_flaps_discrete_word_2(1 << 12, Some(3)), 1 << 15);
    assert_eq!(takeoff_flaps_discrete_word_2(0xffff_ffff, None) & (0xf << 12), 0);
}

#[test]
fn takeoff_speeds_too_low_check() {
    let low = TakeoffSpeeds { v1: Some(100.0), vr: Some(125.0), v2: Some(130.0) };
    assert!(low.too_low(true, true, Some(0.0)));
    assert!(!low.too_low(false, true, Some(0.0)));
    assert!(!low.too_low(true, false, Some(0.0)));
    assert!(!low.too_low(true, true, None));
    let fine = TakeoffSpeeds { v1: Some(150.0), vr: Some(155.0), v2: Some(160.0) };
    assert!(!fine.too_low(true, true, Some(0.0)));
    let vr_low = TakeoffSpeeds { v1: Some(150.0), vr: Some(118.0), v2: Some(160.0) };
    assert!(vr_low.too_low(true, true, Some(0.0)));
    let v2_low = TakeoffSpeeds { v1: Some(150.0), vr: Some(155.0), v2: Some(124.0) };
    assert!(v2_low.too_low(true, true, Some(0.0)));
}

#[test]
fn weights_follow_the_fmc() {
    assert_eq!(gross_weight_tonnes(Some(400_000.0), None, None), Some(400.0));
    assert_eq!(gross_weight_tonnes(None, Some(300.0), Some(80.0)), Some(380.0));
    assert_eq!(gross_weight_tonnes(None, None, Some(80.0)), None);
    assert_eq!(gross_weight_kg(None, Some(0.0), Some(0.0)), None);
    assert_eq!(fuel_on_board_tonnes(true, Some(90_000.0), Some(10.0)), Some(90.0));
    assert_eq!(fuel_on_board_tonnes(false, Some(90_000.0), Some(10.0)), Some(10.0));
    assert_eq!(zero_fuel_weight_tonnes(None), 270.0);

    let ground = LandingWeightInputs {
        takeoff_weight_kg: Some(500_000.0),
        gross_weight_kg: Some(505_000.0),
        trip_fuel_kg: Some(120_000.0),
        engines_were_started: false,
        phase: FlightPhase::Preflight,
        taxi_fuel_tonnes: Some(5.0),
    };
    assert_eq!(landing_weight_kg(&ground), Some(380_000.0));
    let flying = LandingWeightInputs { engines_were_started: true, phase: FlightPhase::Climb, ..ground };
    assert_eq!(landing_weight_kg(&flying), Some(385_000.0));
    let pre = LandingWeightInputs { phase: FlightPhase::Preflight, ..flying };
    assert_eq!(landing_weight_kg(&pre), Some(380_000.0));
    assert_eq!(landing_weight_kg(&LandingWeightInputs { trip_fuel_kg: None, takeoff_weight_kg: None, ..ground }), None);
}

#[test]
fn takeoff_weight_is_frozen_after_liftoff() {
    let base = TakeoffWeightInputs {
        active_or_copy_of_active: true,
        phase: FlightPhase::Preflight,
        engines_were_started: false,
        zero_fuel_weight_tonnes: Some(300.0),
        block_fuel_tonnes: Some(100.0),
        taxi_fuel_tonnes: Some(5.0),
        gross_weight_kg: Some(402_000.0),
    };
    match calculate_takeoff_weight(&base) {
        TakeoffWeight::Value(Some(v)) => assert_eq!(v, 395_000.0),
        _ => panic!(),
    }
    match calculate_takeoff_weight(&TakeoffWeightInputs { engines_were_started: true, ..base }) {
        TakeoffWeight::Value(Some(v)) => assert_eq!(v, 397_000.0),
        _ => panic!(),
    }
    assert!(matches!(
        calculate_takeoff_weight(&TakeoffWeightInputs { phase: FlightPhase::Climb, ..base }),
        TakeoffWeight::Frozen
    ));
}

#[test]
fn recommended_altitudes() {
    let rec = rec_max_altitude_ft(Some(500_000.0), 0.0).unwrap();
    assert_eq!(rec % 500.0, 0.0);
    assert!((rec - 37_500.0).abs() <= 500.0);
    assert!(rec_max_altitude_ft(Some(300_000.0), 0.0).unwrap() > rec);
    assert!(rec_max_altitude_ft(Some(500_000.0), 15.0).unwrap() < rec);
    assert!(rec_max_altitude_ft(Some(200_000.0), -20.0).unwrap() <= MAX_CERTIFIED_ALT);
    assert_eq!(rec_max_altitude_ft(None, 0.0), None);
    let fl = rec_max_flight_level(Some(500_000.0), 0.0);
    assert_eq!(fl, Some(rec / 100.0));
    assert_eq!(opt_flight_level(Some(375.0), false, FlightPhase::Cruise), Some(360.0));
    assert_eq!(opt_flight_level(Some(375.0), true, FlightPhase::Cruise), None);
    assert_eq!(opt_flight_level(Some(375.0), false, FlightPhase::Descent), None);
    assert_eq!(eo_max_flight_level(Some(375.0)), Some(300.0));
}

#[test]
fn reserve_and_extra_fuel() {
    assert_eq!(route_reserve_fuel_kg(Some(5.0), Some(100_000.0), Some(5.0)), Some(5000.0));
    assert_eq!(route_reserve_fuel_kg(None, Some(100_000.0), Some(5.0)), Some(5000.0));
    assert_eq!(route_reserve_fuel_kg(None, None, Some(5.0)), None);
    let e = extra_fuel_kg(true, false, 0.0, Some(120.0), Some(5.0), 80_000.0, Some(2.0), Some(4_000.0), 0.0);
    assert_eq!(e, 120_000.0 - 5_000.0 - 80_000.0 - 2_000.0 - 4_000.0);
    let e = extra_fuel_kg(false, true, 0.0, None, None, 0.0, Some(2.0), None, 12_000.0);
    assert_eq!(e, 10_000.0);
    let e = extra_fuel_kg(false, true, 0.0, None, None, 0.0, Some(2.0), None, -300_000.0);
    assert!(e < 0.0);
}

#[test]
fn econ_speeds() {
    assert_eq!(managed_climb_speed(None), 250.0);
    assert_eq!(managed_climb_speed(Some(0.0)), 290.0);
    assert_eq!(managed_climb_speed(Some(999.0)), 330.0);
    assert!((managed_climb_speed(Some(30.0)) - 290.036_072_108_144_14).abs() < 1e-9);
    assert_eq!(managed_descent_speed(None, None), 300.0);
    assert_eq!(managed_descent_speed(Some(0.0), None), 288.0);
    assert_eq!(managed_descent_speed(Some(999.0), None), 300.0);
    assert_eq!(managed_descent_speed(Some(500.0), Some(280.0)), 280.0);
    assert_eq!(managed_descent_speed_mach(None), 0.84);
    assert_eq!(managed_descent_speed_mach(Some(0.8)), 0.8);
    assert_eq!(managed_cruise_speed(), 310.0);
    assert_eq!(managed_cruise_speed_mach(), 0.85);
}

#[test]
fn cost_index_economy_formulas() {
    assert_eq!(cost_index_to_climb_cas(0.0, 100.0, 50.0 / 0.000_453_592_37), 270.0);
    assert_eq!(cost_index_to_climb_cas(500.0, 100.0, 77.0 / 0.000_453_592_37), 340.0);
    let w = 60.0 / 0.000_453_592_37;
    assert!((cost_index_to_climb_cas(40.0, 300.0, w) - (240.0 + 30.0 + 40.0 + 5.0)).abs() < 1e-9);
    assert!((cost_index_to_descent_cas(20.0, 300.0, w) - (205.0 + 39.0 + 30.0 - 0.5)).abs() < 1e-9);
    assert_eq!(cost_index_to_descent_cas(100.0, 300.0, w), 340.0);
    assert!((cost_index_to_mach(0.0, 0.78, 0.0) - 0.78).abs() < 1e-12);
    assert!(cost_index_to_mach(100.0, 0.78, 0.0) > 0.79);
    assert!(cost_index_to_mach(0.0, 0.78, 100.0) > 0.78);
    assert!(initial_mach_estimate(800_000.0, 0.2) > 0.5);
}

#[test]
fn managed_targets_switch_to_mach() {
    let p = fms_perf::isa::isa_pressure_hpa(35_000.0);
    let (v, is_mach) = managed_targets(310.0, 0.85, Some(p));
    assert!(is_mach);
    assert!((v - fms_perf::isa::mach_to_kcas(0.85, p)).abs() < 1e-9);
    assert!(v < 310.0);
    let (v, is_mach) = managed_targets(250.0, 0.85, Some(p));
    assert!(!is_mach);
    assert_eq!(v, 250.0);
    assert_eq!(managed_targets(310.0, 0.85, None), (310.0, false));
}

#[test]
fn isa_conversions_round_trip() {
    let p = fms_perf::isa::isa_pressure_hpa(0.0);
    assert!((p - 1013.25).abs() < 1e-6);
    assert!((fms_perf::isa::mach_to_kcas(1.0, p) - 661.47).abs() < 0.1);
    let cas = fms_perf::isa::mach_to_kcas(0.78, 250.0);
    assert!((fms_perf::isa::kcas_to_mach(cas, 250.0) - 0.78).abs() < 1e-9);
    assert!((fms_perf::isa::isa_temp_c(36_089.0) - (-56.5)).abs() < 0.1);
    assert!((fms_perf::isa::isa_pressure_hpa(36_089.0) - 226.3).abs() < 1.0);
}

fn managed(phase: FlightPhase) -> ManagedSpeedInputs {
    ManagedSpeedInputs { phase, static_pressure_hpa: Some(fms_perf::isa::isa_pressure_hpa(10_000.0)), ..Default::default() }
}

#[test]
fn managed_speed_per_phase() {
    let m = update_managed_speed(&ManagedSpeedInputs { v2_kt: Some(158.0), ..managed(FlightPhase::Takeoff) });
    assert_eq!(m, ManagedSpeed { target_kt: Some(158.0), is_mach: false });
    assert_eq!(update_managed_speed(&managed(FlightPhase::Preflight)).target_kt, None);

    let climb = update_managed_speed(&ManagedSpeedInputs { cost_index: Some(30.0), ..managed(FlightPhase::Climb) });
    assert!((climb.target_kt.unwrap() - 290.036_072_108_144_14).abs() < 1e-9);
    assert!(!climb.is_mach);
    let limited = update_managed_speed(&ManagedSpeedInputs {
        cost_index: Some(30.0),
        climb_speed_limit: Some(SpeedLimit { speed: 250.0, under_altitude: 10_000.0 }),
        indicated_altitude_ft: 9_000.0,
        ..managed(FlightPhase::Climb)
    });
    assert_eq!(limited.target_kt, Some(250.0));
    let above = update_managed_speed(&ManagedSpeedInputs {
        cost_index: Some(30.0),
        climb_speed_limit: Some(SpeedLimit { speed: 250.0, under_altitude: 10_000.0 }),
        indicated_altitude_ft: 11_000.0,
        ..managed(FlightPhase::Climb)
    });
    assert!(above.target_kt.unwrap() > 289.0);
    let constrained = update_managed_speed(&ManagedSpeedInputs { speed_constraint_kt: 220.0, ..managed(FlightPhase::Climb) });
    assert_eq!(constrained.target_kt, Some(220.0));

    let high = ManagedSpeedInputs {
        static_pressure_hpa: Some(fms_perf::isa::isa_pressure_hpa(37_000.0)),
        ..managed(FlightPhase::Cruise)
    };
    let crz = update_managed_speed(&high);
    assert!(crz.is_mach);
    assert!(crz.target_kt.unwrap() < 310.0);

    let desc = update_managed_speed(&ManagedSpeedInputs { vnav_managed_pfd_kt: 275.0, cost_index: Some(30.0), ..managed(FlightPhase::Descent) });
    assert_eq!(desc.target_kt, Some(275.0));
    let app = update_managed_speed(&ManagedSpeedInputs { approach_vapp_kt: Some(139.0), ..managed(FlightPhase::Approach) });
    assert_eq!(app.target_kt, Some(139.0));
    let ga = update_managed_speed(&ManagedSpeedInputs { green_dot_kt: Some(197.0), speed_constraint_kt: 180.0, ..managed(FlightPhase::GoAround) });
    assert_eq!(ga.target_kt, Some(180.0));
    let ga = update_managed_speed(&ManagedSpeedInputs { green_dot_kt: Some(197.0), ..managed(FlightPhase::GoAround) });
    assert_eq!(ga.target_kt, Some(197.0));
    assert_eq!(update_managed_speed(&managed(FlightPhase::Done)).target_kt, None);

    let hold = update_managed_speed(&ManagedSpeedInputs { hold_decel_reached: true, hold_speed_target: Some(210.0), ..managed(FlightPhase::Cruise) });
    assert_eq!(hold, ManagedSpeed { target_kt: Some(210.0), is_mach: false });
}

#[test]
fn engine_out_climb_ramps_down_to_green_dot() {
    let i = ManagedSpeedInputs {
        engine_out: true,
        green_dot_kt: Some(200.0),
        open_or_managed_climb_active: true,
        cas_kt: Some(300.0),
        cost_index: Some(30.0),
        ..managed(FlightPhase::Climb)
    };
    assert_eq!(update_managed_speed(&i).target_kt, Some(300.0 - 100.0 * 0.25));
    let none = update_managed_speed(&ManagedSpeedInputs { cas_kt: None, ..i });
    assert_eq!(none.target_kt, Some(200.0));
    let inactive = update_managed_speed(&ManagedSpeedInputs { open_or_managed_climb_active: false, ..i });
    assert!(inactive.target_kt.unwrap() > 289.0);
}

#[test]
fn nav_mode_speed_constraints_and_decel_distance() {
    assert_eq!(calculate_decel_dist(250.0, 250.0), 3.0);
    assert_eq!(calculate_decel_dist(300.0, 200.0), 15.0);
    assert_eq!(calculate_decel_dist(400.0, 100.0), 20.0);
    let c = LegSpeedConstraints { climb_speed: 210.0, descent_speed: 180.0, previous_descent_speed: 250.0 };
    assert_eq!(nav_mode_speed_constraint(FlightPhase::Climb, Some(c), None, 300.0), 210.0);
    assert_eq!(nav_mode_speed_constraint(FlightPhase::GoAround, Some(c), None, 300.0), 210.0);
    assert_eq!(nav_mode_speed_constraint(FlightPhase::Cruise, Some(c), None, 300.0), f64::INFINITY);
    assert_eq!(nav_mode_speed_constraint(FlightPhase::Descent, Some(c), Some(50.0), 300.0), 250.0);
    assert_eq!(nav_mode_speed_constraint(FlightPhase::Descent, Some(c), Some(5.0), 300.0), 180.0);
    assert_eq!(nav_mode_speed_constraint(FlightPhase::Descent, None, Some(5.0), 300.0), f64::INFINITY);
    assert_eq!(speed_constraint(false, FlightPhase::Climb, Some(c), None, 300.0), f64::INFINITY);
    assert_eq!(speed_constraint(true, FlightPhase::Climb, Some(c), None, 300.0), 210.0);
}

#[test]
fn approach_managed_speed_by_flap_lever() {
    let f = |lever, conf3| approach_managed_speed(lever, conf3, Some(197.0), Some(179.0), Some(166.0), Some(139.0));
    assert_eq!(f(0, false), Some(197.0));
    assert_eq!(f(1, false), Some(179.0));
    assert_eq!(f(2, false), Some(166.0));
    assert_eq!(f(3, false), Some(166.0));
    assert_eq!(f(3, true), Some(139.0));
    assert_eq!(f(4, false), Some(139.0));
}

#[test]
fn holding_speed_limits() {
    let base = HoldingSpeedInputs {
        fcu_altitude_ft: 5_000.0,
        altitude_ft: None,
        green_dot_kt: 200.0,
        all_engines_on: true,
        phase: FlightPhase::Cruise,
        approach_managed_speed_kt: None,
        speed_constraint_kt: None,
        climb_speed_limit: None,
        descent_speed_limit: None,
        vls_kt: 150.0,
    };
    assert_eq!(holding_speed(&base), 200.0);
    assert_eq!(holding_speed(&HoldingSpeedInputs { all_engines_on: false, ..base }), 225.0);
    assert_eq!(holding_speed(&HoldingSpeedInputs { green_dot_kt: 260.0, ..base }), 230.0);
    assert_eq!(holding_speed(&HoldingSpeedInputs { green_dot_kt: 260.0, fcu_altitude_ft: 15_000.0, ..base }), 240.0);
    assert_eq!(holding_speed(&HoldingSpeedInputs { green_dot_kt: 280.0, fcu_altitude_ft: 25_000.0, ..base }), 265.0);
    let high = holding_speed(&HoldingSpeedInputs { fcu_altitude_ft: 36_000.0, ..base });
    assert!((high - fms_perf::isa::mach_to_kcas_at_altitude(0.83, 36_000.0).ceil()).abs() < 1e-9);
    assert_eq!(holding_speed(&HoldingSpeedInputs { speed_constraint_kt: Some(190.0), ..base }), 190.0);
    assert_eq!(holding_speed(&HoldingSpeedInputs { speed_constraint_kt: Some(90.0), ..base }), 200.0);
    assert_eq!(holding_speed(&HoldingSpeedInputs { vls_kt: 205.5, ..base }), 206.0);
    assert_eq!(
        holding_speed(&HoldingSpeedInputs {
            climb_speed_limit: Some(SpeedLimit { speed: 180.0, under_altitude: 10_000.0 }),
            ..base
        }),
        180.0
    );
    assert_eq!(
        holding_speed(&HoldingSpeedInputs {
            phase: FlightPhase::Approach,
            approach_managed_speed_kt: Some(139.0),
            vls_kt: 130.0,
            ..base
        }),
        139.0
    );
}

#[test]
fn speed_margins() {
    let i = SpeedMarginInputs {
        managed_descent_speed_kt: 300.0,
        managed_descent_speed_mach: 0.84,
        approach_speed_kt: 139.0,
        vmax_kt: 340.0,
        vls_kt: 150.0,
        green_dot_kt: 197.0,
        slat_retraction_kt: 179.0,
        flap_retraction_kt: 166.0,
        flaps_handle_index: 0,
        static_pressure_hpa: fms_perf::isa::isa_pressure_hpa(10_000.0),
    };
    let (lo, up) = i.margins(300.0);
    assert_eq!(lo, 280.0);
    assert_eq!(up, 320.0);
    let (lo, up) = i.margins(280.0);
    assert_eq!(lo, 260.0);
    assert_eq!(up, 285.0);
    let (lo, _) = SpeedMarginInputs { flaps_handle_index: 3, ..i }.margins(160.0);
    assert_eq!(lo, 150.0);
    let (lo, _) = SpeedMarginInputs { flaps_handle_index: 1, ..i }.margins(160.0);
    assert_eq!(lo, 179.0);
    let (lo, up) = SpeedMarginInputs { vmax_kt: 250.0, ..i }.margins(300.0);
    assert_eq!(up, 250.0);
    assert_eq!(lo, 250.0);
    assert_eq!(i.target(500.0, 300.0), 320.0);
    assert_eq!(i.target(100.0, 300.0), 280.0);
    assert_eq!(i.target(290.0, 300.0), 290.0);
}

fn ground() -> PhaseInputs {
    PhaseInputs { on_ground: true, ..Default::default() }
}

fn airborne() -> PhaseInputs {
    PhaseInputs {
        on_ground: false,
        n1_pct: [88.0; 4],
        n2_pct: [92.0; 4],
        ground_speed_kt: 160.0,
        pressure_altitude_ft: Some(500.0),
        baro_corrected_altitude_ft: Some(500.0),
        indicated_altitude_ft: 500.0,
        fm_acc_alt_ft: Some(1_580.0),
        fm_eo_acc_alt_ft: Some(1_580.0),
        ..Default::default()
    }
}

#[test]
fn phase_takeoff_to_climb_to_cruise_to_descent_to_approach_to_done() {
    let mut m = FlightPhaseManager::new();
    let mut i = ground();
    m.init(&i);
    assert_eq!(m.phase(), FlightPhase::Preflight);
    i.vertical_mode = vertical_mode::SRS;
    i.n1_pct = [90.0; 4];
    i.n2_pct = [60.0; 4];
    m.update(100.0, &i);
    assert_eq!(m.phase(), FlightPhase::Preflight);
    m.update(100.0, &i);
    assert_eq!(m.phase(), FlightPhase::Takeoff);
    assert!(m.take_cold_and_dark_cleared());
    assert_eq!(m.airliner_flight_phase_lvar(), 2.0);

    i = airborne();
    i.fm_acc_alt_ft = Some(1_580.0);
    m.update(100.0, &i);
    assert_eq!(m.phase(), FlightPhase::Takeoff);
    i.baro_corrected_altitude_ft = Some(1_700.0);
    m.update(100.0, &i);
    assert_eq!(m.phase(), FlightPhase::Climb);

    i.cruise_altitude_ft = 36_000.0;
    i.pressure_altitude_ft = Some(35_950.0);
    i.fcu_selected_altitude_ft = 36_000.0;
    m.update(100.0, &i);
    assert_eq!(m.phase(), FlightPhase::Cruise);

    i.indicated_altitude_ft = 36_000.0;
    i.fcu_selected_altitude_ft = 10_000.0;
    m.handle_fcu_alt_knob_push_pull(120.0, &i);
    assert_eq!(m.phase(), FlightPhase::Descent);

    i.pressure_altitude_ft = Some(8_000.0);
    i.enable_approach_phase = true;
    m.update(100.0, &i);
    assert_eq!(m.phase(), FlightPhase::Approach);
    assert!(m.take_flex_temp_cleared());

    i.on_ground = true;
    i.n2_pct = [60.0; 4];
    m.update(29_000.0, &i);
    assert_eq!(m.phase(), FlightPhase::Approach);
    m.update(2_000.0, &i);
    assert_eq!(m.phase(), FlightPhase::Done);
    let transitions = m.take_transitions();
    let path: Vec<FlightPhase> = transitions.iter().map(|t| t.next).collect();
    assert_eq!(
        path,
        vec![
            FlightPhase::Takeoff,
            FlightPhase::Climb,
            FlightPhase::Cruise,
            FlightPhase::Descent,
            FlightPhase::Approach,
            FlightPhase::Done
        ]
    );
}

#[test]
fn phase_takeoff_by_ground_speed_and_by_vertical_mode_without_baro() {
    let mut m = FlightPhaseManager::new();
    let mut i = ground();
    i.ground_speed_kt = 95.0;
    m.init(&i);
    m.update(250.0, &i);
    assert_eq!(m.phase(), FlightPhase::Takeoff);
    let mut i = airborne();
    i.baro_corrected_altitude_ft = None;
    m.update(100.0, &i);
    assert_eq!(m.phase(), FlightPhase::Takeoff);
    i.vertical_mode = vertical_mode::OP_CLB;
    m.update(100.0, &i);
    assert_eq!(m.phase(), FlightPhase::Climb);
}

#[test]
fn phase_engine_out_acceleration_needs_green_dot() {
    let mut m = FlightPhaseManager::new();
    let mut i = ground();
    i.ground_speed_kt = 120.0;
    m.init(&i);
    m.update(250.0, &i);
    assert_eq!(m.phase(), FlightPhase::Takeoff);
    let mut i = airborne();
    i.engine_out = true;
    i.green_dot_kt = 200.0;
    i.cas_kt = Some(180.0);
    i.baro_corrected_altitude_ft = Some(2_000.0);
    m.update(100.0, &i);
    assert_eq!(m.phase(), FlightPhase::Takeoff);
    i.cas_kt = Some(205.0);
    m.update(100.0, &i);
    assert_eq!(m.phase(), FlightPhase::Climb);
}

#[test]
fn phase_acceleration_altitude_defaults_to_1500_above_indicated() {
    let mut m = FlightPhaseManager::new();
    let mut i = ground();
    i.ground_speed_kt = 120.0;
    i.indicated_altitude_ft = 100.0;
    m.init(&i);
    m.update(250.0, &i);
    assert_eq!(m.phase(), FlightPhase::Takeoff);
    let mut a = airborne();
    a.baro_corrected_altitude_ft = Some(1_599.0);
    m.update(100.0, &a);
    assert_eq!(m.phase(), FlightPhase::Takeoff);
    a.baro_corrected_altitude_ft = Some(1_601.0);
    m.update(100.0, &a);
    assert_eq!(m.phase(), FlightPhase::Climb);
}

#[test]
fn phase_not_ready_or_slew() {
    let mut m = FlightPhaseManager::new();
    let mut i = ground();
    i.ground_speed_kt = 120.0;
    i.ready = false;
    m.init(&i);
    m.update(1_000.0, &i);
    assert_eq!(m.phase(), FlightPhase::Preflight);
    i.ready = true;
    i.slew_active = true;
    i.altitude_above_ground_ft = 1_000.0;
    m.update(1_000.0, &i);
    assert_eq!(m.phase(), FlightPhase::Preflight);
    i.altitude_above_ground_ft = 1_500.0;
    m.update(1_000.0, &i);
    assert_eq!(m.phase(), FlightPhase::Climb);
}

#[test]
fn phase_go_around_and_new_destination() {
    let mut m = FlightPhaseManager::new();
    let i = airborne();
    m.spawn(false, 3_000.0, 3, &i);
    assert_eq!(m.phase(), FlightPhase::Approach);
    let mut ga = airborne();
    ga.vertical_mode = vertical_mode::SRS_GA;
    m.update(100.0, &ga);
    assert_eq!(m.phase(), FlightPhase::GoAround);
    assert!(m.take_flex_temp_cleared());
    ga.indicated_altitude_ft = 1_000.0;
    ga.fm_acc_alt_ft = Some(1_500.0);
    ga.fm_eo_acc_alt_ft = Some(900.0);
    m.handle_new_destination_airport_entered(&ga);
    assert_eq!(m.phase(), FlightPhase::GoAround);
    ga.engine_out = true;
    m.handle_new_destination_airport_entered(&ga);
    assert_eq!(m.phase(), FlightPhase::Climb);
}

#[test]
fn phase_spawn_rules() {
    let i = airborne();
    let mut m = FlightPhaseManager::new();
    m.spawn(true, 0.0, 0, &ground());
    assert_eq!(m.phase(), FlightPhase::Preflight);
    let mut m = FlightPhaseManager::new();
    m.spawn(false, 30_000.0, 0, &i);
    assert_eq!(m.phase(), FlightPhase::Cruise);
    let mut m = FlightPhaseManager::new();
    m.spawn(false, 5_000.0, 2, &i);
    assert_eq!(m.phase(), FlightPhase::Approach);
    let mut m = FlightPhaseManager::new();
    m.spawn(false, 5_000.0, 0, &i);
    assert_eq!(m.phase(), FlightPhase::Climb);
}

#[test]
fn phase_cruise_altitude_changes() {
    let i = airborne();
    let mut m = FlightPhaseManager::new();
    m.spawn(false, 5_000.0, 0, &i);
    assert_eq!(m.phase(), FlightPhase::Climb);
    let mut a = airborne();
    a.pressure_altitude_ft = Some(30_000.0);
    m.handle_new_cruise_altitude_entered(250.0, &a);
    assert_eq!(m.phase(), FlightPhase::Cruise);
    m.change_phase(FlightPhase::Descent, &a);
    m.handle_new_cruise_altitude_entered(350.0, &a);
    assert_eq!(m.phase(), FlightPhase::Climb);
    m.change_phase(FlightPhase::Approach, &a);
    m.handle_new_cruise_altitude_entered(350.0, &a);
    assert_eq!(m.phase(), FlightPhase::Climb);
}

#[test]
fn phase_fcu_knobs_and_descent_conditions() {
    let mut a = airborne();
    a.indicated_altitude_ft = 36_000.0;
    a.pressure_altitude_ft = Some(36_000.0);
    a.cruise_altitude_ft = 36_000.0;
    a.fcu_selected_altitude_ft = 10_000.0;
    let mut m = FlightPhaseManager::new();
    m.change_phase(FlightPhase::Cruise, &a);
    m.handle_fcu_alt_knob_turn(150.0, 0.0, 0.0, &a);
    assert_eq!(m.phase(), FlightPhase::Cruise);
    a.vertical_mode = vertical_mode::VS;
    m.handle_fcu_alt_knob_turn(150.0, 1000.0, 0.0, &a);
    assert_eq!(m.phase(), FlightPhase::Cruise);
    m.handle_fcu_alt_knob_turn(150.0, -1000.0, 0.0, &a);
    assert_eq!(m.phase(), FlightPhase::Descent);

    let mut m = FlightPhaseManager::new();
    m.change_phase(FlightPhase::Cruise, &a);
    a.fcu_selected_altitude_ft = 36_000.0;
    assert_eq!(m.handle_fcu_vs_knob(150.0, -1000.0, 0.0, &a), VsKnobResult::StepClimbDescent);
    assert_eq!(m.phase(), FlightPhase::Cruise);
    a.fcu_selected_altitude_ft = 20_000.0;
    assert_eq!(m.handle_fcu_vs_knob(150.0, -1000.0, 0.0, &a), VsKnobResult::Descent);
    assert_eq!(m.phase(), FlightPhase::Descent);
    let mut m = FlightPhaseManager::new();
    m.change_phase(FlightPhase::Climb, &a);
    a.vertical_mode = vertical_mode::OP_CLB;
    assert_eq!(m.handle_fcu_vs_knob(150.0, -1000.0, 0.0, &a), VsKnobResult::None);
}

#[test]
fn phase_early_descent_and_try_approach() {
    let mut a = airborne();
    a.indicated_altitude_ft = 36_000.0;
    a.cruise_altitude_ft = 36_000.0;
    a.fcu_selected_altitude_ft = 25_000.0;
    let mut m = FlightPhaseManager::new();
    m.change_phase(FlightPhase::Climb, &a);
    m.handle_fcu_alt_knob_push_pull(250.0, &a);
    assert_eq!(m.phase(), FlightPhase::Climb);
    a.fcu_selected_altitude_ft = 20_000.0;
    m.handle_fcu_alt_knob_push_pull(250.0, &a);
    assert_eq!(m.phase(), FlightPhase::Descent);

    let mut m = FlightPhaseManager::new();
    assert!(!m.try_go_in_approach_phase(&a));
    m.change_phase(FlightPhase::Cruise, &a);
    assert!(m.try_go_in_approach_phase(&a));
    assert_eq!(m.phase(), FlightPhase::Approach);
    assert!(m.try_go_in_approach_phase(&a));
}

#[test]
fn phase_descent_returns_to_cruise_when_level_at_cruise_altitude() {
    let mut a = airborne();
    a.pressure_altitude_ft = Some(36_000.0);
    a.cruise_altitude_ft = 36_000.0;
    a.fcu_selected_altitude_ft = 36_000.0;
    let mut m = FlightPhaseManager::new();
    m.change_phase(FlightPhase::Descent, &a);
    assert_eq!(m.phase(), FlightPhase::Cruise);
    m.change_phase(FlightPhase::Descent, &PhaseInputs { fcu_selected_altitude_ft: 20_000.0, ..a });
    assert_eq!(m.phase(), FlightPhase::Descent);
}

#[test]
fn phase_done_after_ground_with_engines_off() {
    let mut m = FlightPhaseManager::new();
    m.change_phase(FlightPhase::Climb, &airborne());
    let mut g = ground();
    g.n2_pct = [5.0; 4];
    m.update(29_000.0, &g);
    assert_eq!(m.phase(), FlightPhase::Climb);
    m.update(1_001.0, &g);
    assert_eq!(m.phase(), FlightPhase::Done);
    let mut m = FlightPhaseManager::new();
    m.init(&g);
    m.update(60_000.0, &g);
    assert_eq!(m.phase(), FlightPhase::Preflight);
}

#[test]
fn confirmation_node_needs_continuous_input() {
    let mut n = ConfirmationNode::new(1_000.0);
    assert!(!n.update(true, 500.0));
    assert!(!n.update(false, 100.0));
    assert!(!n.update(true, 600.0));
    assert!(n.update(true, 400.0));
    assert!(!n.update(false, 10.0));
}

#[test]
fn outputs_list_known_lvars() {
    use fms_perf::outputs::*;
    let names: Vec<&str> = PERF_LVARS.iter().map(|p| p.0).collect();
    for l in perf_speed_lvars(&Default::default()) {
        assert!(names.contains(&l.name), "{}", l.name);
    }
    for l in speed_margin_lvars(true, 1.0, 2.0) {
        assert!(names.contains(&l.name), "{}", l.name);
    }
    let l = takeoff_lvars(&TakeoffSpeeds::default(), TakeoffPowerSetting::Toga, None, None, false);
    assert_eq!(l[0].value, -1.0);
    assert_eq!(l[1].value, 0.0);
    assert_eq!(l[2].value, -1.0);
    let off = speed_margin_lvars(false, 5.0, 6.0);
    assert!(off.iter().all(|l| l.value == 0.0));
}

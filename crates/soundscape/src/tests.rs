use super::*;

const DT: f64 = 0.02;

fn base() -> Snapshot {
    Snapshot {
        contactors: vec![false; 3],
        fuel_pumps: vec![false; 21],
        bleed_valves: vec![false; 4],
        gear: [1.; 5],
        gear_doors: [0.; 3],
        on_ground: true,
        cockpit_door_locked: true,
        ..Snapshot::default()
    }
}

fn run(sc: &mut Soundscape, s: &Snapshot, ticks: usize) -> Vec<Command> {
    (0..ticks).flat_map(|_| sc.step(s, DT)).collect()
}

fn started(cmds: &[Command], sound: Sound) -> Vec<(u8, bool, f32, f32)> {
    cmds.iter()
        .filter_map(|c| match c {
            Command::Start { id, looped, gain, pitch, .. } if id.sound == sound => Some((id.index, *looped, *gain, *pitch)),
            _ => None,
        })
        .collect()
}

fn stopped(cmds: &[Command], sound: Sound) -> Vec<u8> {
    cmds.iter()
        .filter_map(|c| match c {
            Command::Stop { id, .. } if id.sound == sound => Some(id.index),
            _ => None,
        })
        .collect()
}

fn sets(cmds: &[Command], sound: Sound) -> Vec<(f32, f32)> {
    cmds.iter()
        .filter_map(|c| match c {
            Command::Set { id, gain, pitch } if id.sound == sound => Some((*gain, *pitch)),
            _ => None,
        })
        .collect()
}

fn ready(s: &Snapshot) -> Soundscape {
    let mut sc = Soundscape::new();
    run(&mut sc, s, 5);
    sc
}

#[test]
fn nothing_plays_and_nothing_clicks_on_the_first_snapshot() {
    let mut s = base();
    s.contactors = vec![true, false, true];
    s.ac_buses = [true; 4];
    s.cockpit_door_locked = false;
    let mut sc = Soundscape::new();
    let cmds = sc.step(&s, DT);
    assert!(started(&cmds, Sound::Contactor).is_empty());
    assert!(started(&cmds, Sound::AcBusUp).is_empty());
    assert!(started(&cmds, Sound::CockpitDoorOpen).is_empty());
}

#[test]
fn a_contactor_clicks_once_per_settled_change() {
    let mut s = base();
    let mut sc = ready(&s);
    s.contactors[1] = true;
    let cmds = run(&mut sc, &s, 50);
    assert_eq!(started(&cmds, Sound::Contactor), vec![(1, false, CONTACTOR_GAIN as f32, 1.)]);
    s.contactors[1] = false;
    let cmds = run(&mut sc, &s, 50);
    assert_eq!(started(&cmds, Sound::Contactor), vec![(1, false, CONTACTOR_GAIN as f32, CONTACTOR_OPEN_PITCH as f32)]);
}

#[test]
fn a_contactor_chattering_inside_one_tick_does_not_click() {
    let mut s = base();
    let mut sc = ready(&s);
    let mut cmds = Vec::new();
    for k in 0..40 {
        s.contactors[0] = k % 2 == 0;
        cmds.extend(sc.step(&s, DT));
    }
    assert!(started(&cmds, Sound::Contactor).is_empty());
}

#[test]
fn a_contactor_cycling_faster_than_the_gap_clicks_only_at_the_gap() {
    let mut s = base();
    let mut sc = ready(&s);
    let mut cmds = Vec::new();
    for k in 0..30 {
        s.contactors[2] = (k / 3) % 2 == 0;
        cmds.extend(sc.step(&s, DT));
    }
    let clicks = started(&cmds, Sound::Contactor).len();
    assert!(clicks >= 1 && clicks <= 3, "{clicks}");
}

#[test]
fn ac_buses_thump_on_power_up_only_and_the_tr_hums_with_dc_ess() {
    let mut s = base();
    let mut sc = ready(&s);
    s.ac_buses = [true, false, false, true];
    s.dc_ess = true;
    let cmds = run(&mut sc, &s, 10);
    let mut buses: Vec<u8> = started(&cmds, Sound::AcBusUp).iter().map(|c| c.0).collect();
    buses.sort();
    assert_eq!(buses, vec![0, 3]);
    assert_eq!(started(&cmds, Sound::TrHum).len(), 1);
    s.ac_buses = [false; 4];
    let cmds = run(&mut sc, &s, 10);
    assert!(started(&cmds, Sound::AcBusUp).is_empty());
    assert_eq!(stopped(&cmds, Sound::TrHum), vec![0]);
}

#[test]
fn electric_pumps_loop_above_fbws_3500_rpm_with_hysteresis_and_pitch_by_speed() {
    let mut s = base();
    let mut sc = ready(&s);
    s.emp_rpm[2] = 3400.;
    assert!(started(&run(&mut sc, &s, 2), Sound::HydEmp).is_empty());
    s.emp_rpm[2] = 3600.;
    let st = started(&run(&mut sc, &s, 2), Sound::HydEmp);
    assert_eq!(st.len(), 1);
    assert_eq!(st[0].0, 2);
    assert!(st[0].1);
    s.emp_rpm[2] = 3200.;
    let cmds = run(&mut sc, &s, 2);
    assert!(stopped(&cmds, Sound::HydEmp).is_empty());
    assert!(!sets(&cmds, Sound::HydEmp).is_empty());
    s.emp_rpm[2] = 7600.;
    let up = sets(&run(&mut sc, &s, 2), Sound::HydEmp);
    assert!((up[0].1 - 1.).abs() < 1e-6);
    s.emp_rpm[2] = 2900.;
    assert_eq!(stopped(&run(&mut sc, &s, 2), Sound::HydEmp), vec![2]);
}

#[test]
fn engine_driven_pumps_sit_at_their_engine() {
    let mut s = base();
    let mut sc = ready(&s);
    s.edp_pressurising[5] = true;
    let cmds = run(&mut sc, &s, 2);
    let at = cmds.iter().find_map(|c| match c {
        Command::Start { id, at, .. } if id.sound == Sound::HydEdp => Some((*id, *at)),
        _ => None,
    });
    assert_eq!(at, Some((id(Sound::HydEdp, 5), Location::Engine(3))));
}

#[test]
fn fuel_pumps_are_one_loop_louder_with_more_pumps() {
    let mut s = base();
    let mut sc = ready(&s);
    s.fuel_pumps[0] = true;
    let first = started(&run(&mut sc, &s, 2), Sound::FuelPumps);
    assert_eq!(first.len(), 1);
    s.fuel_pumps.iter_mut().take(12).for_each(|p| *p = true);
    let louder = sets(&run(&mut sc, &s, 2), Sound::FuelPumps);
    assert!(louder[0].0 > first[0].2);
    s.fuel_pumps.iter_mut().for_each(|p| *p = false);
    assert_eq!(stopped(&run(&mut sc, &s, 2), Sound::FuelPumps), vec![0]);
}

#[test]
fn avionics_fans_spin_up_and_run_down_with_ac_power() {
    let mut s = base();
    s.avionics_vent = true;
    let mut sc = ready(&s);
    assert!(!sc.playing(id(Sound::AvionicsVent, 0)));
    s.ac_buses[0] = true;
    let cmds = run(&mut sc, &s, 200);
    assert_eq!(started(&cmds, Sound::AvionicsVent).len(), 1);
    let ramp = sets(&cmds, Sound::AvionicsVent);
    assert!(ramp.windows(2).all(|w| w[1].1 >= w[0].1));
    assert!(ramp.last().unwrap().1 > 0.95);
    s.ac_buses[0] = false;
    let cmds = run(&mut sc, &s, 10);
    assert!(stopped(&cmds, Sound::AvionicsVent).is_empty());
    let cmds = run(&mut sc, &s, 1000);
    assert_eq!(stopped(&cmds, Sound::AvionicsVent), vec![0]);
}

#[test]
fn recirculation_fans_need_their_switch_and_ac() {
    let mut s = base();
    s.ac_buses = [true; 4];
    let mut sc = ready(&s);
    assert!(!sc.playing(id(Sound::RecircFans, 0)));
    s.recirc_fans = true;
    run(&mut sc, &s, 50);
    assert!(sc.playing(id(Sound::RecircFans, 0)));
}

#[test]
fn pack_flow_drives_the_flight_deck_air_and_each_packs_acm() {
    let mut s = base();
    let mut sc = ready(&s);
    s.pack_flow_kg_s = [0.004, 0.];
    let cmds = run(&mut sc, &s, 2);
    assert_eq!(started(&cmds, Sound::CockpitAir).len(), 1);
    assert!(started(&cmds, Sound::PackAcm).is_empty());
    s.pack_flow_kg_s = [1.0, 0.8];
    let cmds = run(&mut sc, &s, 2);
    let mut packs: Vec<u8> = started(&cmds, Sound::PackAcm).iter().map(|c| c.0).collect();
    packs.sort();
    assert_eq!(packs, vec![0, 1]);
    s.pack_flow_kg_s = [0.; 2];
    let cmds = run(&mut sc, &s, 2);
    assert_eq!(stopped(&cmds, Sound::CockpitAir), vec![0]);
    assert!(cmds.iter().any(|c| matches!(c, Command::Stop { id, fade_s } if id.sound == Sound::CockpitAir && *fade_s == COCKPIT_AIR_FADE_S)));
}

#[test]
fn bleed_and_start_valves_clunk_on_each_change() {
    let mut s = base();
    let mut sc = ready(&s);
    s.bleed_valves[3] = true;
    s.start_valves[1] = true;
    let cmds = run(&mut sc, &s, 20);
    assert_eq!(started(&cmds, Sound::BleedValve).iter().map(|c| c.0).collect::<Vec<_>>(), vec![3]);
    assert_eq!(started(&cmds, Sound::StartValve).iter().map(|c| c.0).collect::<Vec<_>>(), vec![1]);
    s.start_valves[1] = false;
    let cmds = run(&mut sc, &s, 40);
    assert_eq!(started(&cmds, Sound::StartValve)[0].3, VALVE_CLOSE_PITCH as f32);
}

#[test]
fn apu_start_run_and_shutdown() {
    let mut s = base();
    let mut sc = ready(&s);
    let mut cmds = Vec::new();
    for n in 0..=100 {
        s.apu_n = n as f64;
        cmds.extend(sc.step(&s, 0.5));
    }
    assert_eq!(started(&cmds, Sound::ApuStart).len(), 1);
    assert_eq!(started(&cmds, Sound::ApuRun).len(), 1);
    assert!(started(&cmds, Sound::ApuShutdown).is_empty());
    let pitches: Vec<f32> = sets(&cmds, Sound::ApuRun).iter().map(|p| p.1).collect();
    assert!(pitches.windows(2).all(|w| w[1] >= w[0]));
    assert!((pitches.last().unwrap() - 1.).abs() < 0.01);
    let mut cmds = Vec::new();
    for n in (0..=100).rev() {
        s.apu_n = n as f64;
        cmds.extend(sc.step(&s, 0.5));
    }
    assert_eq!(started(&cmds, Sound::ApuShutdown).len(), 1);
    assert!(started(&cmds, Sound::ApuStart).is_empty());
    assert_eq!(stopped(&cmds, Sound::ApuRun), vec![0]);
}

#[test]
fn an_apu_already_running_at_load_does_not_play_its_start() {
    let mut s = base();
    s.apu_n = 100.;
    let mut sc = Soundscape::new();
    let cmds = run(&mut sc, &s, 5);
    assert!(started(&cmds, Sound::ApuStart).is_empty());
    assert_eq!(started(&cmds, Sound::ApuRun).len(), 1);
}

#[test]
fn the_apu_flap_motor_runs_while_the_flap_travels() {
    let mut s = base();
    let mut sc = ready(&s);
    let mut cmds = Vec::new();
    for k in 0..=50 {
        s.apu_flap = k as f64 * 2.;
        cmds.extend(sc.step(&s, 0.1));
    }
    cmds.extend(run(&mut sc, &s, 20));
    assert_eq!(started(&cmds, Sound::ApuFlap).len(), 1);
    assert_eq!(stopped(&cmds, Sound::ApuFlap), vec![0]);
}

#[test]
fn gear_retraction_plays_motion_and_one_uplock_per_leg() {
    let mut s = base();
    let mut sc = ready(&s);
    let mut cmds = Vec::new();
    for k in (0..=100).rev() {
        s.gear = [k as f64 / 100.; 5];
        cmds.extend(sc.step(&s, 0.1));
    }
    cmds.extend(run(&mut sc, &s, 20));
    assert_eq!(started(&cmds, Sound::GearMotion).len(), 1);
    assert_eq!(stopped(&cmds, Sound::GearMotion), vec![0]);
    let mut legs: Vec<u8> = started(&cmds, Sound::GearUplock).iter().map(|c| c.0).collect();
    legs.sort();
    assert_eq!(legs, vec![0, 1, 2, 3, 4]);
    assert!(started(&cmds, Sound::GearDownlock).is_empty());
}

#[test]
fn a_gear_leg_hovering_at_the_lock_clunks_once() {
    let mut s = base();
    s.gear = [0.5; 5];
    let mut sc = ready(&s);
    let mut cmds = Vec::new();
    for v in [0.99, 0.996, 0.99, 0.997, 0.99, 0.999, 1.0] {
        s.gear[0] = v;
        cmds.extend(sc.step(&s, DT));
    }
    assert_eq!(started(&cmds, Sound::GearDownlock).len(), 1);
}

#[test]
fn gear_doors_play_while_they_move() {
    let mut s = base();
    let mut sc = ready(&s);
    let mut cmds = Vec::new();
    for k in 0..=20 {
        s.gear_doors[1] = k as f64 / 20.;
        cmds.extend(sc.step(&s, 0.1));
    }
    cmds.extend(run(&mut sc, &s, 20));
    assert_eq!(started(&cmds, Sound::GearDoors).iter().map(|c| c.0).collect::<Vec<_>>(), vec![1]);
    assert_eq!(stopped(&cmds, Sound::GearDoors), vec![1]);
}

#[test]
fn a_cargo_door_runs_its_actuator_and_latches_at_the_end() {
    let mut s = base();
    let mut sc = ready(&s);
    let mut cmds = Vec::new();
    for k in 0..=100 {
        s.cargo_doors[2] = k as f64 / 100.;
        cmds.extend(sc.step(&s, 0.2));
    }
    cmds.extend(run(&mut sc, &s, 20));
    assert_eq!(started(&cmds, Sound::CargoDoor).len(), 1);
    assert_eq!(started(&cmds, Sound::CargoDoorLatch).iter().map(|c| c.0).collect::<Vec<_>>(), vec![2]);
}

#[test]
fn the_cockpit_door_opens_and_closes_once_each() {
    let mut s = base();
    let mut sc = ready(&s);
    s.cockpit_door_locked = false;
    let cmds = run(&mut sc, &s, 10);
    assert_eq!(started(&cmds, Sound::CockpitDoorOpen).len(), 1);
    s.cockpit_door_locked = true;
    let cmds = run(&mut sc, &s, 40);
    assert_eq!(started(&cmds, Sound::CockpitDoorClose).len(), 1);
}

#[test]
fn brakes_squeal_only_when_slow_on_the_ground_and_braking() {
    let mut s = base();
    let mut sc = ready(&s);
    s.ground_speed_kt = 6.;
    s.brake_demand = 0.6;
    let cmds = run(&mut sc, &s, 30);
    assert_eq!(started(&cmds, Sound::BrakeSqueal).len(), 1);
    s.brake_demand = 0.25;
    assert!(stopped(&run(&mut sc, &s, 5), Sound::BrakeSqueal).is_empty());
    s.brake_demand = 0.1;
    assert_eq!(stopped(&run(&mut sc, &s, 5), Sound::BrakeSqueal), vec![0]);
    s.brake_demand = 0.6;
    s.ground_speed_kt = 40.;
    assert!(started(&run(&mut sc, &s, 30), Sound::BrakeSqueal).is_empty());
    s.ground_speed_kt = 6.;
    s.on_ground = false;
    assert!(started(&run(&mut sc, &s, 30), Sound::BrakeSqueal).is_empty());
}

#[test]
fn a_brake_tap_shorter_than_the_hold_is_silent() {
    let mut s = base();
    let mut sc = ready(&s);
    s.ground_speed_kt = 6.;
    s.brake_demand = 0.6;
    let mut cmds = run(&mut sc, &s, 5);
    s.brake_demand = 0.;
    cmds.extend(run(&mut sc, &s, 5));
    assert!(started(&cmds, Sound::BrakeSqueal).is_empty());
}

#[test]
fn flaps_and_slats_motor_while_moving_and_stop_once() {
    let mut s = base();
    let mut sc = ready(&s);
    let mut cmds = Vec::new();
    for k in 0..=50 {
        s.flaps = k as f64 / 200.;
        s.slats = k as f64 / 100.;
        cmds.extend(sc.step(&s, 0.1));
    }
    cmds.extend(run(&mut sc, &s, 30));
    assert_eq!(started(&cmds, Sound::FlapMotor).len(), 1);
    assert_eq!(started(&cmds, Sound::SlatMotor).len(), 1);
    assert_eq!(started(&cmds, Sound::FlapStop).len(), 1);
    assert_eq!(started(&cmds, Sound::SlatStop).len(), 1);
    assert_eq!(stopped(&cmds, Sound::FlapMotor), vec![0]);
}

#[test]
fn spoilers_are_louder_the_faster_they_deploy() {
    let mut s = base();
    let mut sc = ready(&s);
    s.spoilers = 0.004;
    let slow = started(&run(&mut sc, &s, 1), Sound::Spoilers)[0].2;
    let mut sc = ready(&base());
    s.spoilers = 0.5;
    let fast = started(&run(&mut sc, &s, 1), Sound::Spoilers)[0].2;
    assert!(fast > slow);
}

#[test]
fn wipers_switch_loops_by_speed_and_park_when_turned_off() {
    let mut s = base();
    let mut sc = ready(&s);
    s.wipers[0] = WiperMode::Slow;
    assert_eq!(started(&run(&mut sc, &s, 2), Sound::WiperSlow).len(), 1);
    s.wipers[0] = WiperMode::Fast;
    let cmds = run(&mut sc, &s, 2);
    assert_eq!(stopped(&cmds, Sound::WiperSlow), vec![0]);
    assert_eq!(started(&cmds, Sound::WiperFast).len(), 1);
    s.wipers[0] = WiperMode::Off;
    let cmds = run(&mut sc, &s, 2);
    assert_eq!(stopped(&cmds, Sound::WiperFast), vec![0]);
    assert_eq!(started(&cmds, Sound::WiperPark), vec![(0, false, WIPER_GAIN as f32, 1.)]);
    assert!(started(&run(&mut sc, &s, 5), Sound::WiperPark).is_empty());
}

#[test]
fn rain_follows_fbws_threshold_and_grows_with_rate_and_speed() {
    let mut s = base();
    let mut sc = ready(&s);
    s.precip_mm = 0.08;
    assert!(started(&run(&mut sc, &s, 2), Sound::Rain).is_empty());
    s.precip_mm = 2.;
    let quiet = started(&run(&mut sc, &s, 2), Sound::Rain)[0].2;
    s.tas_kt = 250.;
    let loud = sets(&run(&mut sc, &s, 2), Sound::Rain)[0].0;
    assert!(loud > quiet);
    s.precip_mm = 0.07;
    assert!(stopped(&run(&mut sc, &s, 2), Sound::Rain).is_empty());
    s.precip_mm = 0.;
    assert_eq!(stopped(&run(&mut sc, &s, 2), Sound::Rain), vec![0]);
}

#[test]
fn hail_plays_with_the_shower() {
    let mut s = base();
    let mut sc = ready(&s);
    s.hail = 0.5;
    s.tas_kt = 150.;
    assert_eq!(started(&run(&mut sc, &s, 2), Sound::Hail).len(), 1);
    s.hail = 0.;
    assert_eq!(stopped(&run(&mut sc, &s, 2), Sound::Hail), vec![0]);
}

#[test]
fn seat_motors_run_while_the_seat_moves_and_stop_once() {
    let mut s = base();
    let mut sc = ready(&s);
    let mut cmds = Vec::new();
    for k in 0..=20 {
        s.seats[3] = k as f64 / 40.;
        cmds.extend(sc.step(&s, 0.05));
    }
    cmds.extend(run(&mut sc, &s, 20));
    assert_eq!(started(&cmds, Sound::SeatMotor).iter().map(|c| c.0).collect::<Vec<_>>(), vec![3]);
    assert_eq!(started(&cmds, Sound::SeatStop).iter().map(|c| c.0).collect::<Vec<_>>(), vec![3]);
}

#[test]
fn oxygen_mask_test_flows_while_held() {
    let mut s = base();
    let mut sc = ready(&s);
    s.oxy_mask_test[1] = true;
    assert_eq!(started(&run(&mut sc, &s, 2), Sound::OxyMaskTest).iter().map(|c| c.0).collect::<Vec<_>>(), vec![1]);
    s.oxy_mask_test[1] = false;
    assert_eq!(stopped(&run(&mut sc, &s, 2), Sound::OxyMaskTest), vec![1]);
}

#[test]
fn cvr_test_needs_ground_control_like_fbw() {
    let mut s = base();
    let mut sc = ready(&s);
    s.cvr_test = true;
    assert!(started(&run(&mut sc, &s, 2), Sound::CvrTest).is_empty());
    s.cvr_ground_control = true;
    assert_eq!(started(&run(&mut sc, &s, 2), Sound::CvrTest).len(), 1);
}

#[test]
fn pause_stops_every_loop_and_unpause_restarts_them() {
    let mut s = base();
    s.ac_buses = [true; 4];
    s.dc_ess = true;
    s.precip_mm = 3.;
    let mut sc = ready(&s);
    assert!(sc.playing(id(Sound::Rain, 0)));
    s.paused = true;
    let cmds = sc.step(&s, DT);
    assert_eq!(stopped(&cmds, Sound::Rain), vec![0]);
    assert_eq!(stopped(&cmds, Sound::TrHum), vec![0]);
    assert!(sc.step(&s, DT).is_empty());
    s.paused = false;
    let cmds = sc.step(&s, DT);
    assert_eq!(started(&cmds, Sound::Rain).len(), 1);
}

#[test]
fn a_steady_state_sends_nothing() {
    let mut s = base();
    s.ac_buses = [true; 4];
    s.dc_ess = true;
    s.emp_rpm = [7600.; 4];
    s.pack_flow_kg_s = [1.; 2];
    s.fuel_pumps.iter_mut().for_each(|p| *p = true);
    s.apu_n = 100.;
    let mut sc = Soundscape::new();
    run(&mut sc, &s, 2000);
    assert!(run(&mut sc, &s, 50).is_empty());
}

#[test]
fn every_location_names_a_sound_node() {
    let all = [
        Location::Overhead,
        Location::Pedestal,
        Location::FlightDeckAir,
        Location::AvionicsBay,
        Location::CockpitDoor,
        Location::Seat(Side::Left),
        Location::Seat(Side::Right),
        Location::Windshield(Side::Left),
        Location::Windshield(Side::Right),
        Location::NoseGearBay,
        Location::MainGearBay,
        Location::PackBay(Side::Left),
        Location::PackBay(Side::Right),
        Location::WingRoot,
        Location::Wing(Side::Left),
        Location::Wing(Side::Right),
        Location::Engine(1),
        Location::Engine(4),
        Location::ApuTail,
        Location::CargoDoor(0),
    ];
    assert!(all.iter().all(|l| !l.node().is_empty()));
}

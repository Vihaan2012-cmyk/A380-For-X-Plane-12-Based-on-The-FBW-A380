use fuel_mgmt::jettison::*;
use fuel_mgmt::model::total_kg;
use fuel_mgmt::trim::*;
use fuel_mgmt::{apply_delta, capacity_kg, CgModel, Kg, Tank, TANK_COUNT};

fn tanks(v: &[(Tank, f64)]) -> Kg {
    let mut t = [0.0; TANK_COUNT];
    for (k, q) in v {
        t[k.idx()] = *q;
    }
    t
}

fn heavy_tanks() -> Kg {
    tanks(&[
        (Tank::LeftOuter, 6000.0),
        (Tank::RightOuter, 6000.0),
        (Tank::Feed1, 18000.0),
        (Tank::Feed2, 18000.0),
        (Tank::Feed3, 18000.0),
        (Tank::Feed4, 18000.0),
        (Tank::LeftMid, 25000.0),
        (Tank::RightMid, 25000.0),
        (Tank::LeftInner, 30000.0),
        (Tank::RightInner, 30000.0),
        (Tank::Trim, 15000.0),
    ])
}

fn jet_inputs(gw: f64, t: Kg) -> JettisonInputs {
    JettisonInputs { arm_pb: true, active_pb: true, gw_kg: gw, tanks: t, ..Default::default() }
}

fn run_jettison(c: &mut JettisonController, i: &mut JettisonInputs, seconds: f64, dt: f64) -> JettisonOutput {
    let mut out = c.update(i, 0.0);
    let steps = (seconds / dt) as usize;
    for _ in 0..steps {
        out = c.update(i, dt);
        let applied = apply_delta(&mut i.tanks, &out.delta_kg(dt));
        i.gw_kg += applied.iter().sum::<f64>();
    }
    out
}

#[test]
fn jettison_stops_at_target_weight_and_closes_valves() {
    let mut c = JettisonController::default();
    let mut i = jet_inputs(500_000.0, heavy_tanks());
    i.target_gw_kg = Some(391_000.0);
    let feed_before: Vec<f64> = Tank::FEED.iter().map(|t| i.tanks[t.idx()]).collect();
    let out = run_jettison(&mut c, &mut i, 4000.0, 0.5);
    assert_eq!(out.state, JettisonState::Completed);
    assert!(out.memo_completed && !out.memo_jettison);
    assert!((i.gw_kg - 391_000.0).abs() < 400.0, "gw {}", i.gw_kg);
    assert!(i.gw_kg >= 391_000.0 - 400.0);
    assert_eq!(out.valve_cmd, [false, false]);
    assert!(out.valve_pos.iter().all(|p| *p < 0.01));
    for (k, t) in Tank::FEED.iter().enumerate() {
        assert_eq!(i.tanks[t.idx()], feed_before[k]);
    }
    assert!(i.tanks.iter().all(|q| *q >= 0.0));
}

#[test]
fn jettison_rate_is_bounded_and_ramps_with_valve_travel() {
    let mut c = JettisonController::default();
    let i = jet_inputs(500_000.0, heavy_tanks());
    let first = c.update(&i, 1.0);
    assert!(first.flow_kg_s[0] < 0.25 * 20.8);
    let mut last = first;
    for _ in 0..10 {
        last = c.update(&i, 1.0);
    }
    let total: f64 = last.flow_kg_s.iter().sum();
    assert!((total - 41.6).abs() < 0.5, "total {total}");
}

#[test]
fn jettison_requires_arm_and_active_and_a_weight_above_target() {
    let mut c = JettisonController::default();
    let mut i = jet_inputs(500_000.0, heavy_tanks());
    i.arm_pb = false;
    assert_eq!(c.update(&i, 1.0).state, JettisonState::Off);
    i.arm_pb = true;
    i.active_pb = false;
    let o = c.update(&i, 1.0);
    assert_eq!(o.state, JettisonState::Armed);
    assert!(o.arm_lamp && !o.active_lamp && o.flow_kg_s == [0.0, 0.0]);
    i.active_pb = true;
    i.gw_kg = 380_000.0;
    let o = c.update(&i, 1.0);
    assert_eq!(o.state, JettisonState::Armed);
    assert_eq!(o.inhibit, Some(JettisonInhibit::AtTarget));
}

#[test]
fn jettison_is_inhibited_on_ground_and_with_flaps_out() {
    let mut c = JettisonController::default();
    let mut i = jet_inputs(500_000.0, heavy_tanks());
    i.on_ground = true;
    let o = c.update(&i, 1.0);
    assert_eq!(o.inhibit, Some(JettisonInhibit::OnGround));
    assert_eq!(o.valve_cmd, [false, false]);
    i.on_ground = false;
    i.flap_config = 3;
    assert_eq!(c.update(&i, 1.0).inhibit, Some(JettisonInhibit::FlapsExtended));
    i.flap_config = 0;
    i.power_ok = false;
    assert_eq!(c.update(&i, 1.0).inhibit, Some(JettisonInhibit::NoPower));
}

#[test]
fn jettison_stops_when_flaps_extend_and_needs_reselect() {
    let mut c = JettisonController::default();
    let mut i = jet_inputs(500_000.0, heavy_tanks());
    run_jettison(&mut c, &mut i, 60.0, 1.0);
    assert!(c.update(&i, 0.1).memo_jettison);
    i.flap_config = 4;
    let o = run_jettison(&mut c, &mut i, 20.0, 1.0);
    assert_eq!(o.state, JettisonState::Stopped(JettisonInhibit::FlapsExtended));
    assert!(o.valve_pos.iter().all(|p| *p < 0.01));
    i.flap_config = 0;
    assert_eq!(c.update(&i, 1.0).state, JettisonState::Stopped(JettisonInhibit::FlapsExtended));
    i.active_pb = false;
    c.update(&i, 1.0);
    i.active_pb = true;
    assert_eq!(c.update(&i, 1.0).state, JettisonState::Active);
}

#[test]
fn jettison_with_one_nozzle_failed_closed_is_half_rate_and_alerts() {
    let mut c = JettisonController::default();
    let mut i = jet_inputs(500_000.0, heavy_tanks());
    i.valve_failed_closed = [true, false];
    let out = run_jettison(&mut c, &mut i, 30.0, 1.0);
    assert_eq!(out.flow_kg_s[0], 0.0);
    assert!(out.flow_kg_s[1] > 19.0);
    assert!(out.alert_fault && out.fault_lamp);
    let mut c2 = JettisonController::default();
    i.target_gw_kg = Some(480_000.0);
    i.gw_kg = 500_000.0;
    i.tanks = heavy_tanks();
    let out = run_jettison(&mut c2, &mut i, 2000.0, 0.5);
    assert_eq!(out.state, JettisonState::Completed);
    assert!((i.gw_kg - 480_000.0).abs() < 300.0, "gw {}", i.gw_kg);
}

#[test]
fn jettison_valve_stuck_open_raises_not_closed_alert() {
    let mut c = JettisonController::default();
    let mut i = jet_inputs(500_000.0, heavy_tanks());
    i.target_gw_kg = Some(499_000.0);
    run_jettison(&mut c, &mut i, 200.0, 1.0);
    i.valve_stuck_open = [false, true];
    i.gw_kg = 500_000.0;
    let mut c = JettisonController::default();
    i.active_pb = true;
    let o = run_jettison(&mut c, &mut i, 60.0, 1.0);
    assert!(o.valve_pos[1] > 0.1);
    i.active_pb = false;
    let o = run_jettison(&mut c, &mut i, 30.0, 1.0);
    assert!(o.alert_valve_not_closed);
    assert!(o.valve_pos[1] > 0.1);
}

#[test]
fn jettison_never_touches_feed_tanks_and_stops_at_minimum_fuel() {
    let mut c = JettisonController::default();
    let t = tanks(&[
        (Tank::Feed1, 3000.0),
        (Tank::Feed2, 3000.0),
        (Tank::Feed3, 3000.0),
        (Tank::Feed4, 3000.0),
        (Tank::LeftInner, 9000.0),
        (Tank::RightInner, 9000.0),
    ]);
    let mut i = jet_inputs(300_000.0, t);
    i.target_gw_kg = Some(100_000.0);
    let out = run_jettison(&mut c, &mut i, 3000.0, 0.5);
    assert_eq!(out.state, JettisonState::Completed);
    assert!(out.completed_short);
    assert!(total_kg(&i.tanks) >= 10_000.0 - 800.0, "left {}", total_kg(&i.tanks));
    for f in Tank::FEED {
        assert_eq!(i.tanks[f.idx()], 3000.0);
    }
}

#[test]
fn jettison_prefers_trim_fuel_when_cg_is_aft_and_spares_it_when_forward() {
    let mut aft = JettisonController::default();
    let mut fwd = JettisonController::default();
    let mut ia = jet_inputs(500_000.0, heavy_tanks());
    ia.cg_pct = Some(43.5);
    ia.cg_target_pct = Some(41.5);
    let mut ifw = ia.clone();
    ifw.cg_pct = Some(38.0);
    let mut oa = aft.update(&ia, 0.0);
    let mut of = fwd.update(&ifw, 0.0);
    for _ in 0..10 {
        oa = aft.update(&ia, 1.0);
        of = fwd.update(&ifw, 1.0);
    }
    let trim = Tank::Trim.idx();
    assert!(oa.draw_kg_s[trim] > of.draw_kg_s[trim] * 5.0);
}

#[test]
fn jettison_draws_more_from_the_heavier_wing() {
    let mut c = JettisonController::default();
    let mut t = heavy_tanks();
    t[Tank::LeftInner.idx()] += 2500.0;
    t[Tank::RightInner.idx()] -= 2500.0;
    let i = jet_inputs(500_000.0, t);
    let mut o = c.update(&i, 0.0);
    for _ in 0..10 {
        o = c.update(&i, 1.0);
    }
    assert!(o.flow_kg_s[0] > o.flow_kg_s[1] * 1.3, "{:?}", o.flow_kg_s);
}

#[test]
fn jettison_active_inhibits_auto_trim_transfer() {
    let mut c = JettisonController::default();
    let i = jet_inputs(500_000.0, heavy_tanks());
    let o = c.update(&i, 1.0);
    assert!(o.inhibits_auto_trim);
}

fn trim_tanks(trim: f64, inner: f64, feed: f64) -> Kg {
    tanks(&[
        (Tank::Trim, trim),
        (Tank::LeftInner, inner),
        (Tank::RightInner, inner),
        (Tank::LeftMid, 10000.0),
        (Tank::RightMid, 10000.0),
        (Tank::Feed1, feed),
        (Tank::Feed2, feed),
        (Tank::Feed3, feed),
        (Tank::Feed4, feed),
    ])
}

struct Sim {
    c: TrimController,
    cg: CgModel,
    i: TrimInputs,
}

impl Sim {
    fn new(i: TrimInputs) -> Self {
        Self { c: TrimController::default(), cg: CgModel::default(), i }
    }

    fn step(&mut self, dt: f64) -> TrimOutput {
        let out = self.c.update(&self.i, dt);
        let applied = apply_delta(&mut self.i.tanks, &out.delta_kg(dt));
        let (cg, gw) = self.cg.cg_after(self.i.cg_pct.unwrap_or(41.0), self.i.gw_kg, &applied);
        if self.i.cg_pct.is_some() {
            self.i.cg_pct = Some(cg);
        }
        self.i.gw_kg = gw;
        out
    }

    fn burn(&mut self, kg_s_per_engine: f64, dt: f64) {
        let mut d = [0.0; TANK_COUNT];
        for f in Tank::FEED {
            d[f.idx()] = -kg_s_per_engine * dt;
        }
        let applied = apply_delta(&mut self.i.tanks, &d);
        let (cg, gw) = self.cg.cg_after(self.i.cg_pct.unwrap_or(41.0), self.i.gw_kg, &applied);
        if self.i.cg_pct.is_some() {
            self.i.cg_pct = Some(cg);
        }
        self.i.gw_kg = gw;
    }
}

fn cruise_inputs(cg: f64, t: Kg) -> TrimInputs {
    TrimInputs { tanks: t, gw_kg: 500_000.0, cg_pct: Some(cg), phase: FlightPhase::Cruise, ..Default::default() }
}

#[test]
fn trim_aft_transfer_moves_cg_aft_into_the_band_and_stops() {
    let mut s = Sim::new(cruise_inputs(38.0, trim_tanks(0.0, 25000.0, 20000.0)));
    let target = default_cg_target_pct(500_000.0);
    let start = s.i.cg_pct.unwrap();
    let mut mode_seen = false;
    for _ in 0..4000 {
        let o = s.step(1.0);
        mode_seen |= o.mode == TrimMode::Aft;
    }
    let end = s.i.cg_pct.unwrap();
    assert!(mode_seen);
    assert!(end > start + 1.5, "cg {start} -> {end}");
    assert!(end >= target - 0.55 && end <= target + 0.05, "end {end} target {target}");
    assert!(s.i.tanks[Tank::Trim.idx()] > 2000.0);
    assert_eq!(s.c.update(&s.i, 1.0).mode, TrimMode::Idle);
}

#[test]
fn trim_forward_transfer_moves_cg_forward_when_aft_of_target() {
    let target = default_cg_target_pct(500_000.0);
    let mut s = Sim::new(cruise_inputs(target + 1.0, trim_tanks(12000.0, 15000.0, 12000.0)));
    let mut saw_fwd = false;
    for _ in 0..3000 {
        saw_fwd |= s.step(1.0).mode == TrimMode::Forward;
    }
    let end = s.i.cg_pct.unwrap();
    assert!(saw_fwd);
    assert!(end < target - 0.5 && end > target - 1.0, "end {end} target {target}");
    assert!(s.i.tanks[Tank::Trim.idx()] > 0.0);
}

#[test]
fn trim_holds_cg_near_target_during_a_burn_with_forward_transfers() {
    let target = default_cg_target_pct(500_000.0);
    let mut s = Sim::new(cruise_inputs(target - 0.3, trim_tanks(12000.0, 15000.0, 20000.0)));
    let mut worst: f64 = f64::MIN;
    let mut fwd_seen = false;
    for _ in 0..1800 {
        s.burn(10.0, 1.0);
        let o = s.step(1.0);
        fwd_seen |= o.mode == TrimMode::Forward;
        let tgt = default_cg_target_pct(s.i.gw_kg);
        worst = worst.max(s.i.cg_pct.unwrap() - tgt);
    }
    assert!(fwd_seen);
    assert!(worst < 0.5, "worst {worst}");
}

#[test]
fn trim_tank_empties_forward_in_descent_even_with_cg_in_band() {
    let target = default_cg_target_pct(500_000.0);
    let mut i = cruise_inputs(target - 0.3, trim_tanks(9000.0, 15000.0, 12000.0));
    i.phase = FlightPhase::Descent;
    let mut s = Sim::new(i);
    let mut reason = TrimReason::None;
    for _ in 0..1500 {
        let o = s.step(1.0);
        if o.mode == TrimMode::Forward {
            reason = o.reason;
        }
    }
    assert_eq!(reason, TrimReason::LandingPreparation);
    assert!(s.i.tanks[Tank::Trim.idx()] <= 20.0);
}

#[test]
fn trim_no_aft_transfer_in_descent_or_on_approach() {
    for ph in [FlightPhase::Descent, FlightPhase::Approach] {
        let mut i = cruise_inputs(36.0, trim_tanks(0.0, 25000.0, 20000.0));
        i.phase = ph;
        let mut c = TrimController::default();
        assert_eq!(c.update(&i, 1.0).mode, TrimMode::Idle);
    }
}

#[test]
fn trim_is_inhibited_on_ground_refuel_jettison_invalid_cg_and_auto_off() {
    let base = cruise_inputs(36.0, trim_tanks(0.0, 25000.0, 20000.0));
    let check = |f: &dyn Fn(&mut TrimInputs), want: TrimInhibit| {
        let mut i = base.clone();
        f(&mut i);
        let o = TrimController::default().update(&i, 1.0);
        assert_eq!(o.inhibit, Some(want));
        assert_eq!(o.mode, TrimMode::Idle);
        assert!(o.flow_kg_s.iter().all(|f| *f == 0.0));
    };
    check(&|i| i.phase = FlightPhase::Ground, TrimInhibit::OnGround);
    check(&|i| i.refuel_active = true, TrimInhibit::Refuelling);
    check(&|i| i.jettison_active = true, TrimInhibit::Jettison);
    check(&|i| i.cg_pct = None, TrimInhibit::CgInvalid);
    check(&|i| i.auto_on = false, TrimInhibit::AutoOff);
    check(&|i| i.power_ok = false, TrimInhibit::NoPower);
    assert_eq!(TrimController::default().update(&base, 1.0).mode, TrimMode::Aft);
}

#[test]
fn trim_flows_conserve_mass() {
    for (cg, trim, ph) in [(36.0, 0.0, FlightPhase::Cruise), (44.0, 9000.0, FlightPhase::Cruise), (41.0, 5000.0, FlightPhase::Descent)] {
        let mut i = cruise_inputs(cg, trim_tanks(trim, 25000.0, 12000.0));
        i.phase = ph;
        let o = TrimController::default().update(&i, 1.0);
        assert!(o.flow_kg_s.iter().sum::<f64>().abs() < 1e-9);
        assert!(o.rate_kg_s > 0.0);
    }
}

#[test]
fn trim_forward_with_one_pump_failed_halves_rate_and_flags_degraded() {
    let target = default_cg_target_pct(500_000.0);
    let mut i = cruise_inputs(target + 2.0, trim_tanks(12000.0, 15000.0, 12000.0));
    let full = TrimController::default().update(&i, 1.0).rate_kg_s;
    i.faults.pump_failed = [true, false];
    let mut c = TrimController::default();
    let mut o = c.update(&i, 1.0);
    assert!((o.rate_kg_s - full / 2.0).abs() < 1e-6);
    assert_eq!(o.trim_pump_cmd, [false, true]);
    for _ in 0..12 {
        o = c.update(&i, 1.0);
    }
    assert!(o.alert_xfr_degraded && !o.alert_auto_xfr_fault);
}

#[test]
fn trim_both_pumps_lost_gives_fault_after_confirm() {
    let target = default_cg_target_pct(500_000.0);
    let mut i = cruise_inputs(target + 2.0, trim_tanks(12000.0, 15000.0, 12000.0));
    i.faults.pump_failed = [true, true];
    let mut c = TrimController::default();
    let mut o = c.update(&i, 1.0);
    assert!(!o.alert_auto_xfr_fault);
    for _ in 0..11 {
        o = c.update(&i, 1.0);
    }
    assert!(o.alert_auto_xfr_fault);
    assert_eq!(o.rate_kg_s, 0.0);
}

#[test]
fn trim_pump_switches_off_stop_forward_transfer() {
    let target = default_cg_target_pct(500_000.0);
    let mut i = cruise_inputs(target + 2.0, trim_tanks(12000.0, 15000.0, 12000.0));
    i.pump_sw_on = [false, false];
    let o = TrimController::default().update(&i, 1.0);
    assert_eq!(o.rate_kg_s, 0.0);
    assert_eq!(o.trim_pump_cmd, [false, false]);
}

#[test]
fn trim_aft_valves_follow_failures() {
    let mut i = cruise_inputs(36.0, trim_tanks(0.0, 25000.0, 20000.0));
    let full = TrimController::default().update(&i, 1.0);
    i.faults.inlet_valve_failed = [false, true];
    let o = TrimController::default().update(&i, 1.0);
    assert!((o.rate_kg_s - full.rate_kg_s / 2.0).abs() < 1e-6);
    assert_eq!(o.inlet_valve_cmd, [true, false]);
    assert!(o.iso_aft_cmd && o.aft_source_pumps_on);
    i.faults.iso_aft_failed = true;
    assert_eq!(TrimController::default().update(&i, 1.0).rate_kg_s, 0.0);
}

#[test]
fn trim_low_feed_tank_forces_forward_transfer_even_with_cg_forward() {
    let mut i = cruise_inputs(36.0, trim_tanks(8000.0, 15000.0, 12000.0));
    i.tanks[Tank::Feed2.idx()] = 2500.0;
    let o = TrimController::default().update(&i, 1.0);
    assert_eq!((o.mode, o.reason), (TrimMode::Forward, TrimReason::LowFeedTank));
    assert!(o.flow_kg_s[Tank::Feed2.idx()] > o.flow_kg_s[Tank::Feed1.idx()]);
}

#[test]
fn trim_manual_forward_works_with_auto_off_but_not_below_forward_limit() {
    let mut i = cruise_inputs(36.0, trim_tanks(8000.0, 15000.0, 12000.0));
    i.auto_on = false;
    i.manual = ManualTransfer::Forward;
    let o = TrimController::default().update(&i, 1.0);
    assert_eq!((o.mode, o.reason, o.inhibit), (TrimMode::Forward, TrimReason::Manual, None));
    i.cg_pct = Some(27.0);
    assert_eq!(TrimController::default().update(&i, 1.0).mode, TrimMode::Idle);
}

#[test]
fn trim_manual_aft_stops_at_target() {
    let target = default_cg_target_pct(500_000.0);
    let mut i = cruise_inputs(target + 0.1, trim_tanks(0.0, 25000.0, 12000.0));
    i.manual = ManualTransfer::Aft;
    assert_eq!(TrimController::default().update(&i, 1.0).mode, TrimMode::Idle);
    i.cg_pct = Some(target - 2.0);
    assert_eq!(TrimController::default().update(&i, 1.0).mode, TrimMode::Aft);
}

#[test]
fn trim_cg_limit_alert() {
    let i = cruise_inputs(44.0, trim_tanks(0.0, 25000.0, 20000.0));
    assert!(TrimController::default().update(&i, 1.0).alert_cg_out_of_limits);
}

#[test]
fn capacities_total_about_320_kilolitres() {
    let total_l: f64 = capacity_kg().iter().sum::<f64>() / 0.8;
    assert!((total_l - 323_000.0).abs() < 5_000.0, "{total_l}");
}

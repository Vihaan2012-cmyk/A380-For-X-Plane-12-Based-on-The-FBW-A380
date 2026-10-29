//! Nosewheel and body-gear (rear-axle) steering, with a shimmy damper.
//!
//! # Steering actuator
//! A rate-limited hydraulic position servo tracking a commanded angle: the
//! nosewheel at 15 deg/s up to 70 deg, the body gear aft axles at 4.5 deg/s
//! up to 15 deg on the BWS schedule in `body_steering_angle_deg`.
//!
//! # Shimmy
//! A steerable wheel is a rotational mass-spring-damper about its
//! steering/caster axis; a torque proportional to groundspeed that opposes
//! the mechanical damper (the classic destabilising mechanism: tyre
//! cornering-force lag interacting with caster geometry, standard published
//! shimmy theory, e.g. NASA/FAA shimmy analyses -- not A380-specific data)
//! reduces the net effective damping as speed rises:
//!
//!   I*delta'' + c_net(v)*delta' + K*delta = disturbance
//!   c_net(v) = c_mechanical(fault) - DESTAB_GAIN*v
//!
//! `c_mechanical` falls from a healthy value (GENERIC, sized so the
//! resulting critical speed -- where `c_net` reaches zero -- sits far above
//! any realistic ground speed) toward a small residual structural value as
//! `shimmy_damper_fail` rises (GENERIC, sized so a fully failed damper's
//! critical speed sits in the realistic taxi/rollout range): below the
//! critical speed the system is a damped oscillator settling to a small
//! offset from a constant disturbance torque (GENERIC, representing runway
//! surface roughness); above it, the same linear system is unstable and the
//! perturbation grows -- shimmy, an emergent consequence of the equation,
//! not a scripted symptom. All of `I`, `K`, `c_mechanical`, `DESTAB_GAIN`
//! and the disturbance magnitude are GENERIC, chosen only to place the
//! healthy/failed critical speeds in the right qualitative regimes (checked
//! by this file's own tests), since no public A380 nosewheel torsional data
//! exists.

/// Normal NWS rate, deg/s (FCOM DSC-32-10-40; FBW nose actuator 15 deg/s).
const NOSE_STEERING_RATE_DEG_S: f64 = 15.0;
/// Alternate NWS rate (LEHGS/accumulator supply), deg/s (FCOM DSC-32-10-40).
const NOSE_STEERING_ALTERNATE_RATE_DEG_S: f64 = 10.0;
/// BWS rate, deg/s (FCOM DSC-32-10-40; FBW body actuators 4.5 deg/s).
const BODY_STEERING_RATE_DEG_S: f64 = 4.5;
/// Handwheel NWS limit, deg (FCOM DSC-32-10-40).
const MAX_NOSE_ANGLE_DEG: f64 = 70.0;
/// BWS limit, deg (FBW a380 hydraulic BodyWheelSteeringController).
const MAX_BODY_ANGLE_DEG: f64 = 15.0;
/// BWS is active only above this NWS angle and below this ground speed
/// (FCOM DSC-32-10-40).
pub const BWS_NOSE_THRESHOLD_DEG: f64 = 20.0;
pub const BWS_MAX_GROUNDSPEED_KT: f64 = 30.0;
const KT_TO_MS: f64 = 1852.0 / 3600.0;
/// Inner-side and outer-side BWS angle at full 70 deg NWS (FBW's NWS-to-BWS
/// table: 15 deg inner, 11 deg outer, zero up to 20 deg NWS).
const BWS_INNER_AT_FULL_NWS_DEG: f64 = 15.0;
const BWS_OUTER_AT_FULL_NWS_DEG: f64 = 11.0;

/// Torsional inertia about the steering axis, kg*m^2 (GENERIC).
const I_EFF: f64 = 40.0;
/// Torsional (centering) stiffness, N*m/rad (GENERIC).
const K_EFF: f64 = 60_000.0;
/// Healthy mechanical shimmy-damper coefficient, N*m*s/rad (GENERIC, sized
/// so the healthy critical speed is far above any realistic ground speed --
/// see `critical_speed_ms`'s test).
const C_MECHANICAL_HEALTHY: f64 = 20_000.0;
/// Residual structural damping with the damper fully failed, N*m*s/rad
/// (GENERIC: never literally zero friction).
const C_RESIDUAL_FAILED: f64 = 500.0;
/// Destabilising gain, N*m*s/rad per m/s of groundspeed (GENERIC).
const DESTAB_GAIN: f64 = 150.0;
/// Small constant seed disturbance torque, N*m (GENERIC: runway surface
/// roughness/joint texture).
const DISTURBANCE_NM: f64 = 50.0;
/// Perturbation magnitude beyond which nonlinear mechanical limits this
/// module does not otherwise model (stops, tyre scrub) are assumed to cap
/// further growth (GENERIC).
const MAX_PERTURBATION_DEG: f64 = 8.0;

const SUB_STEP_S: f64 = 0.005;

fn to_deg(rad: f64) -> f64 {
    rad * 180.0 / std::f64::consts::PI
}
fn to_rad(deg: f64) -> f64 {
    deg * std::f64::consts::PI / 180.0
}

/// BWS demand for one body gear's aft axle, deg positive right, opposing
/// the nose: zero up to 20 deg NWS or from 30 kt, then rising to 15 deg on
/// the inner gear and 11 deg on the outer gear at 70 deg NWS.
pub fn body_steering_angle_deg(nose_angle_deg: f64, groundspeed_ms: f64, left_side: bool) -> f64 {
    if groundspeed_ms.abs() >= BWS_MAX_GROUNDSPEED_KT * KT_TO_MS {
        return 0.0;
    }
    let excess = ((nose_angle_deg.abs() - BWS_NOSE_THRESHOLD_DEG) / (MAX_NOSE_ANGLE_DEG - BWS_NOSE_THRESHOLD_DEG)).clamp(0.0, 1.0);
    if excess == 0.0 {
        return 0.0;
    }
    let inner = (nose_angle_deg < 0.0) == left_side;
    let full = if inner { BWS_INNER_AT_FULL_NWS_DEG } else { BWS_OUTER_AT_FULL_NWS_DEG };
    -nose_angle_deg.signum() * full * excess
}

/// Above this fault severity the nosewheel disconnect mechanism no longer
/// responds to a new selection (GENERIC, this crate's own BITE-flag
/// convention, e.g. `ANTISKID_BITE_THRESHOLD`).
const DISC_MECHANISM_FAIL_THRESHOLD: f64 = 0.5;
/// Above this fault severity the nosewheel steering angle limit switch/
/// software clamp is defeated, letting a genuine over-limit angle occur
/// (GENERIC, same convention).
const OVERTRAVEL_FAIL_THRESHOLD: f64 = 0.5;

/// Faults this steering system carries, each a fraction 0 (healthy) .. 1
/// (fully failed).
#[derive(Clone, Copy, Debug, Default)]
pub struct SteeringFaults {
    /// Shimmy damper degraded/failed.
    pub shimmy_damper_fail: f64,
    /// Steering actuator internal leak: reduces slew rate.
    pub actuator_leak: f64,
    /// `E-IND-DESIGN.md` 320800057/059: the nosewheel disconnect (towing)
    /// mechanism resists commanded release/engagement, so it stops
    /// responding to new selections above `DISC_MECHANISM_FAIL_THRESHOLD`.
    /// Only meaningful on the nose position; harmless (never armed) on the
    /// two body positions, which have no disconnect mechanism at all.
    pub disc_mechanism_fail: f64,
    /// `E-IND-DESIGN.md` 320800056: the nosewheel steering angle limit
    /// switch/software clamp is defeated above
    /// `OVERTRAVEL_FAIL_THRESHOLD`, letting a genuine over-limit angle
    /// occur that this model's own healthy clamp otherwise prevents. Only
    /// meaningful on the nose position.
    pub steer_overtravel_fail: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct SteeringInputs {
    /// Commanded angle, deg (already scheduled/limited by whoever drives
    /// the tiller or pedals; this module does not itself impose the
    /// high-speed pedal-steering-only limits some types apply).
    pub commanded_angle_deg: f64,
    pub groundspeed_ms: f64,
    pub dt_s: f64,
    /// `E-IND-DESIGN.md` 320800057/059: the nosewheel disconnect (towing)
    /// lever selected. Only meaningful on the nose position; the two body
    /// positions have no disconnect mechanism and always pass `false`.
    pub disconnect_selected: bool,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct SteeringOutputs {
    /// True wheel angle including the shimmy perturbation, deg.
    pub angle_deg: f64,
    /// The commanded (base) angle actually tracked so far, deg.
    pub base_angle_deg: f64,
    /// The shimmy perturbation alone, deg.
    pub shimmy_deg: f64,
    /// The net damping at the current speed/fault state has gone unstable
    /// (a genuine, ongoing shimmy tendency, not just noise).
    pub shimmy_unstable: bool,
    /// `E-IND-DESIGN.md` 320800057/059: whether the nosewheel is currently
    /// mechanically disconnected from its steering actuator. Always `false`
    /// on the two body positions (no disconnect mechanism exists there).
    pub disconnected: bool,
}

pub struct SteeringActuator {
    max_angle_deg: f64,
    rate_deg_s: f64,
    base_angle_deg: f64,
    perturbation_deg: f64,
    perturbation_rate: f64,
    /// `E-IND-DESIGN.md` 320800057/059: persistent disconnect state, so a
    /// jammed mechanism (`disc_mechanism_fail`) can genuinely freeze it
    /// rather than the output being recomputed fresh from the selection
    /// every tick.
    disconnected: bool,
}

impl SteeringActuator {
    pub fn new_nose() -> Self {
        Self { max_angle_deg: MAX_NOSE_ANGLE_DEG, rate_deg_s: NOSE_STEERING_RATE_DEG_S, base_angle_deg: 0.0, perturbation_deg: 0.0, perturbation_rate: 0.0, disconnected: false }
    }

    /// Nose only: normal (green) or alternate NWS slew rate.
    pub fn set_alternate(&mut self, alternate: bool) {
        self.rate_deg_s = if alternate { NOSE_STEERING_ALTERNATE_RATE_DEG_S } else { NOSE_STEERING_RATE_DEG_S };
    }

    pub fn new_body() -> Self {
        Self { max_angle_deg: MAX_BODY_ANGLE_DEG, rate_deg_s: BODY_STEERING_RATE_DEG_S, base_angle_deg: 0.0, perturbation_deg: 0.0, perturbation_rate: 0.0, disconnected: false }
    }

    /// The groundspeed at which the net effective damping reaches zero for
    /// the given fault severity: below it the shimmy mode is damped, above
    /// it the same linear equation is unstable.
    pub fn critical_speed_ms(shimmy_damper_fail: f64) -> f64 {
        let c_mech = C_RESIDUAL_FAILED + (C_MECHANICAL_HEALTHY - C_RESIDUAL_FAILED) * (1.0 - shimmy_damper_fail.clamp(0.0, 1.0));
        c_mech / DESTAB_GAIN
    }

    pub fn step(&mut self, inputs: &SteeringInputs, faults: &SteeringFaults) -> SteeringOutputs {
        let dt = inputs.dt_s.max(0.0);

        // `E-IND-DESIGN.md` 320800057/059: the disconnect mechanism only
        // responds to a new selection while healthy; a jam freezes it at
        // whatever state it was last in, regardless of the new selection.
        if faults.disc_mechanism_fail < DISC_MECHANISM_FAIL_THRESHOLD {
            self.disconnected = inputs.disconnect_selected;
        }

        // `E-IND-DESIGN.md` 320800056: healthy behaviour clamps the
        // commanded angle to this actuator's own travel limit, exactly as
        // before. An armed overtravel failure is a real defeat of that
        // limit switch/software clamp, so it is the one case allowed to
        // skip it -- everything else about the actuator (rate, shimmy) is
        // unchanged.
        let overtravel = faults.steer_overtravel_fail >= OVERTRAVEL_FAIL_THRESHOLD;
        let target = if overtravel { inputs.commanded_angle_deg } else { inputs.commanded_angle_deg.clamp(-self.max_angle_deg, self.max_angle_deg) };
        let rate = self.rate_deg_s * (1.0 - faults.actuator_leak.clamp(0.0, 1.0)).max(0.0);
        let diff = target - self.base_angle_deg;
        let step = diff.signum() * (rate * dt).min(diff.abs());
        self.base_angle_deg += step;

        let c_mech = C_RESIDUAL_FAILED + (C_MECHANICAL_HEALTHY - C_RESIDUAL_FAILED) * (1.0 - faults.shimmy_damper_fail.clamp(0.0, 1.0));
        let c_net = c_mech - DESTAB_GAIN * inputs.groundspeed_ms.max(0.0);

        let mut delta = to_rad(self.perturbation_deg);
        let mut delta_dot = self.perturbation_rate;
        let mut remaining = dt;
        while remaining > 1e-9 {
            let h = SUB_STEP_S.min(remaining);
            // The damping term is taken implicitly. Explicitly, the rate is
            // multiplied by (1 - c_net*h/I_EFF) each sub-step, which for the
            // healthy damper at rest is -1.5: the perturbation grew without
            // bound in a few ticks, overflowed, and the NaN stayed in the
            // state for the rest of the session (seen live as
            // NW_STEER_ANGLE_DEG = NaN every frame on a parked aircraft).
            // Dividing by (1 + c_net*h/I_EFF) instead is the exact solution
            // of the linear damping over the sub-step and is stable for any
            // positive damping and any h; the stiffness and disturbance
            // terms are unchanged. Negative c_net (an unstable, failed
            // damper above its critical speed) still grows, as it should,
            // and the clamp below still caps it.
            let forcing = (DISTURBANCE_NM - K_EFF * delta) / I_EFF;
            delta_dot = (delta_dot + forcing * h) / (1.0 + c_net * h / I_EFF).max(1e-6);
            delta += delta_dot * h;
            remaining -= h;
        }
        // A non-finite state cannot recover on its own and would poison
        // every frame after it; start the perturbation over instead.
        if !delta.is_finite() || !delta_dot.is_finite() {
            delta = 0.0;
            delta_dot = 0.0;
        }
        self.perturbation_deg = to_deg(delta).clamp(-MAX_PERTURBATION_DEG, MAX_PERTURBATION_DEG);
        self.perturbation_rate = delta_dot;

        let bound = self.max_angle_deg + MAX_PERTURBATION_DEG;
        let angle_deg = if overtravel { self.base_angle_deg + self.perturbation_deg } else { (self.base_angle_deg + self.perturbation_deg).clamp(-bound, bound) };

        SteeringOutputs {
            angle_deg,
            base_angle_deg: self.base_angle_deg,
            shimmy_deg: self.perturbation_deg,
            shimmy_unstable: c_net < 0.0,
            disconnected: self.disconnected,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn healthy() -> SteeringFaults {
        SteeringFaults::default()
    }

    #[test]
    fn alternate_nws_slews_at_10_deg_per_s() {
        let inputs = SteeringInputs { commanded_angle_deg: 70.0, groundspeed_ms: 0.0, dt_s: 1.0, disconnect_selected: false };
        let mut normal = SteeringActuator::new_nose();
        let mut alternate = SteeringActuator::new_nose();
        alternate.set_alternate(true);
        assert!((normal.step(&inputs, &healthy()).base_angle_deg - 15.0).abs() < 1e-9);
        assert!((alternate.step(&inputs, &healthy()).base_angle_deg - 10.0).abs() < 1e-9);
        alternate.set_alternate(false);
        assert!((alternate.step(&inputs, &healthy()).base_angle_deg - 25.0).abs() < 1e-9);
    }

    #[test]
    fn a_healthy_damper_never_goes_unstable_at_realistic_ground_speeds() {
        let v_crit = SteeringActuator::critical_speed_ms(0.0);
        assert!(v_crit > 90.0, "healthy critical speed should be far above realistic taxi/rollout speeds: {v_crit} m/s");
    }

    #[test]
    fn a_fully_failed_damper_has_a_low_critical_speed() {
        let v_crit = SteeringActuator::critical_speed_ms(1.0);
        assert!(v_crit < 10.0, "a fully failed damper should go unstable at a realistic taxi speed: {v_crit} m/s");
    }

    #[test]
    fn a_healthy_damper_at_rest_stays_finite_and_settles() {
        // The live defect: parked, healthy damper, the explicit damping
        // step diverged (multiplier -1.5 per sub-step) and the angle was NaN
        // from the first seconds of the session onward.
        let mut s = SteeringActuator::new_nose();
        let inputs = SteeringInputs { commanded_angle_deg: 0.0, groundspeed_ms: 0.0, dt_s: 1.0 / 36.0, disconnect_selected: false };
        let mut out = SteeringOutputs::default();
        for _ in 0..3_600 {
            out = s.step(&inputs, &healthy());
            assert!(out.angle_deg.is_finite(), "steering angle went non-finite: {}", out.angle_deg);
        }
        assert!(!out.shimmy_unstable);
        // 50 N*m against 60 kN*m/rad is a 0.048 degree static offset.
        assert!(out.shimmy_deg.abs() < 0.1, "at rest the perturbation must settle near its static offset, got {}", out.shimmy_deg);
    }

    #[test]
    fn a_healthy_damper_settles_and_stays_bounded_at_high_speed() {
        let mut s = SteeringActuator::new_nose();
        let inputs = SteeringInputs { commanded_angle_deg: 0.0, groundspeed_ms: 90.0, dt_s: 0.01, disconnect_selected: false };
        let mut out = SteeringOutputs::default();
        for _ in 0..2_000 {
            out = s.step(&inputs, &healthy());
        }
        assert!(!out.shimmy_unstable);
        assert!(out.shimmy_deg.abs() < 1.0, "a healthy damper must settle to a small bounded offset, not grow: {}", out.shimmy_deg);
    }

    #[test]
    fn a_failed_damper_lets_the_perturbation_grow_at_a_speed_just_above_its_critical_speed() {
        let mut s = SteeringActuator::new_nose();
        let faults = SteeringFaults { shimmy_damper_fail: 1.0, ..SteeringFaults::default() };
        // 4 m/s is just above `critical_speed_ms(1.0)` (3.33 m/s): a mild,
        // gradual instability, not an instant slam into the perturbation
        // cap, so "early" and "late" amplitudes stay distinguishable.
        let inputs = SteeringInputs { commanded_angle_deg: 0.0, groundspeed_ms: 4.0, dt_s: 0.01, disconnect_selected: false };
        let mut early = 0.0;
        let mut out = SteeringOutputs::default();
        for i in 0..3_000 {
            out = s.step(&inputs, &faults);
            if i == 200 {
                early = out.shimmy_deg.abs();
            }
        }
        assert!(out.shimmy_unstable, "4 m/s is above the fully-failed critical speed and must be flagged unstable");
        assert!(out.shimmy_deg.abs() > early + 0.1, "the perturbation must have grown from its early value ({early}) to its later one ({})", out.shimmy_deg);
    }

    #[test]
    fn the_actuator_tracks_a_commanded_angle_within_its_travel_limit() {
        let mut s = SteeringActuator::new_body();
        let inputs = SteeringInputs { commanded_angle_deg: 45.0, groundspeed_ms: 0.0, dt_s: 0.1, disconnect_selected: false };
        let mut out = SteeringOutputs::default();
        for _ in 0..50 {
            out = s.step(&inputs, &healthy());
        }
        assert!((out.base_angle_deg - MAX_BODY_ANGLE_DEG).abs() < 0.5, "commanding past the body axle's travel limit must clamp to it: {}", out.base_angle_deg);
    }

    #[test]
    fn bws_engages_above_20_degrees_nws_below_30_kt_and_opposes_the_nose() {
        let kt = |v: f64| v * KT_TO_MS;
        assert_eq!(body_steering_angle_deg(20.0, kt(5.0), true), 0.0, "20 deg NWS is not above the threshold");
        assert_eq!(body_steering_angle_deg(-19.0, kt(5.0), false), 0.0);
        assert!(body_steering_angle_deg(25.0, kt(5.0), true) < 0.0, "above 20 deg the aft axle steers against the nose");
        assert!(body_steering_angle_deg(-25.0, kt(29.0), true) > 0.0);
        assert_eq!(body_steering_angle_deg(70.0, kt(30.0), true), 0.0, "off from 30 kt");
        assert_eq!(body_steering_angle_deg(70.0, kt(45.0), false), 0.0);
    }

    #[test]
    fn full_nws_gives_15_degrees_inner_and_11_degrees_outer_bws() {
        // Left turn: the left body gear is the inner one.
        assert!((body_steering_angle_deg(-70.0, 0.0, true) - 15.0).abs() < 1e-9);
        assert!((body_steering_angle_deg(-70.0, 0.0, false) - 11.0).abs() < 1e-9);
        assert!((body_steering_angle_deg(70.0, 0.0, false) + 15.0).abs() < 1e-9);
        assert!((body_steering_angle_deg(70.0, 0.0, true) + 11.0).abs() < 1e-9);
        assert!((body_steering_angle_deg(-45.0, 0.0, true) - 7.5).abs() < 1e-9, "linear between 20 and 70 deg NWS");
    }

    #[test]
    fn nws_slews_at_15_and_bws_at_4_5_degrees_per_second() {
        let one_second = |mut s: SteeringActuator| {
            let inputs = SteeringInputs { commanded_angle_deg: 60.0, groundspeed_ms: 0.0, dt_s: 0.01, disconnect_selected: false };
            let mut out = SteeringOutputs::default();
            for _ in 0..100 {
                out = s.step(&inputs, &healthy());
            }
            out.base_angle_deg
        };
        assert!((one_second(SteeringActuator::new_nose()) - 15.0).abs() < 1e-6);
        assert!((one_second(SteeringActuator::new_body()) - 4.5).abs() < 1e-6);
    }

    #[test]
    fn numerically_safe_at_rest_and_dt_zero() {
        let mut s = SteeringActuator::new_nose();
        let inputs = SteeringInputs { commanded_angle_deg: 0.0, groundspeed_ms: 0.0, dt_s: 0.0, disconnect_selected: false };
        let out = s.step(&inputs, &healthy());
        assert!(out.angle_deg.is_finite());
        assert!(!out.shimmy_deg.is_nan());
    }

    /// `E-IND-DESIGN.md` 320800056 STEER N/W STEER ANGLE LIMIT EXCEEDED: the
    /// healthy clamp always holds; an armed `steer_overtravel_fail` is the
    /// one case that lets a commanded angle past `MAX_NOSE_ANGLE_DEG`
    /// through.
    #[test]
    fn steer_overtravel_fail_lets_a_commanded_angle_exceed_the_healthy_clamp() {
        let mut s = SteeringActuator::new_nose();
        let big_command = SteeringInputs { commanded_angle_deg: MAX_NOSE_ANGLE_DEG + 40.0, groundspeed_ms: 0.0, dt_s: 0.5, disconnect_selected: false };

        let mut healthy_actuator = SteeringActuator::new_nose();
        let mut healthy_out = SteeringOutputs::default();
        for _ in 0..50 {
            healthy_out = healthy_actuator.step(&big_command, &healthy());
        }
        assert!(healthy_out.angle_deg.abs() <= MAX_NOSE_ANGLE_DEG + MAX_PERTURBATION_DEG + 1e-6, "the healthy clamp must hold: {}", healthy_out.angle_deg);

        let faults = SteeringFaults { steer_overtravel_fail: 1.0, ..SteeringFaults::default() };
        let mut out = SteeringOutputs::default();
        for _ in 0..50 {
            out = s.step(&big_command, &faults);
        }
        assert!(out.angle_deg.abs() > MAX_NOSE_ANGLE_DEG + MAX_PERTURBATION_DEG, "an armed overtravel failure must let the angle exceed the healthy clamp, got {}", out.angle_deg);
    }

    /// `E-IND-DESIGN.md` 320800057/059 STEER N/W STEER DISC FAULT / NOT DISC:
    /// healthy, the disconnect state tracks a new selection; an armed
    /// `disc_mechanism_fail` freezes it regardless of the selection.
    #[test]
    fn disc_mechanism_fail_freezes_the_disconnect_state_against_a_new_selection() {
        let mut s = SteeringActuator::new_nose();
        let not_selected = SteeringInputs { commanded_angle_deg: 0.0, groundspeed_ms: 0.0, dt_s: 0.1, disconnect_selected: false };
        let out = s.step(&not_selected, &healthy());
        assert!(!out.disconnected, "must start connected");

        let selected = SteeringInputs { disconnect_selected: true, ..not_selected };
        let out = s.step(&selected, &healthy());
        assert!(out.disconnected, "a healthy mechanism must respond to a new selection");

        // Now jam it, and try to deselect: a jammed mechanism must not move.
        let faults = SteeringFaults { disc_mechanism_fail: 1.0, ..SteeringFaults::default() };
        let out = s.step(&not_selected, &faults);
        assert!(out.disconnected, "a jammed mechanism must stay stuck disconnected against a new deselection");
    }
}

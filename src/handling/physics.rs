//! What X-Plane's flight model is given from FlyByWire's gear, brakes, flaps,
//! slats and steering: the X-Plane side of FlyByWire's `VariablesToObject`
//! writes and `variable_to_event` brake and steering events.

/// Gear deployment for X-Plane's five legs, from FlyByWire's three gear
/// positions (percent), with the fake door drag of `GearPosition::write`
/// (gear.rs:91-111).
///
/// MSFS drives its contact points by retract group, the fifth column of each
/// `point.N` in flight_model.cfg:100-104: point 0 (nose) with GEAR CENTER
/// POSITION, points 1 and 3 with GEAR LEFT POSITION, points 2 and 4 with GEAR
/// RIGHT POSITION. The converted .acf keeps MSFS's order (`_gear/0` nose at
/// x 0; `_gear/1`, `_gear/3` at x -11.5 and -23; `_gear/2`, `_gear/4` at x 11.5
/// and 23).
pub fn gear_deploy(center: f64, left: f64, right: f64, door_center: f64, door_left: f64, door_right: f64) -> ([f64; 5], bool) {
    const GEAR_POSITION_FOR_FAKE_DOOR_DRAG: f64 = 0.10;
    let gear_deployed = center > 5. || left > 5. || right > 5.;
    let door_opened = door_center > 10. || door_left > 10. || door_right > 10.;
    let (nose, l, r) = if door_opened && !gear_deployed {
        (
            (door_center / 100.).min(GEAR_POSITION_FOR_FAKE_DOOR_DRAG),
            (door_left / 100.).min(GEAR_POSITION_FOR_FAKE_DOOR_DRAG),
            (door_right / 100.).min(GEAR_POSITION_FOR_FAKE_DOOR_DRAG),
        )
    } else {
        (center / 100., left / 100., right / 100.)
    };
    ([nose, l, r, l, r], gear_deployed)
}

/// The converted .acf's flap detents, degrees of flap 1 and 2 at each of its
/// `acf/_flap_detents 5` stops (`acf/_flap1_dn/0..5` = `acf/_flap2_dn/0..5`),
/// which the converter took from flight_model.cfg's [FLAPS.0]
/// `flaps-position.0..5` (lines 874-879), with FULL at 33 deg (FCOM LIM-13;
/// `tools/acf_patch.py` TUNING).
pub const ACF_FLAP_DETENT_DEG: [f64; 6] = [0., 0.01, 8., 17., 26., 33.];
/// `acf/_slat1_dn_max_deg` = `acf/_slat2_dn_max_deg`.
pub const ACF_SLAT_MAX_DEG: f64 = 23.;
/// The converted .acf's slat detents (X-Plane's leading-edge flap fractions,
/// `acf/_slat1_dn/0..5` = `acf/_slat2_dn/0..5`, acf.rs:735-741). The installed
/// .acf carries flight_model.cfg's `[FLAPS.2]` leading-edge table (confirmed
/// from the installed `_slat1_dn/0..5` ratios and `_slat1_dn_max_deg=23.01`),
/// not the systems' own repeated-degree table: FlyByWire's A380X slats only
/// ever sit at 0, 20 or 23 physical degrees (sfcc/slats_channel.rs
/// `demanded_slats_fppu_angle_from_conf`, hydraulic/mod.rs:1756-1760
/// `SLAT_FPPU_TO_SURFACE_ANGLE_DEGREES`), so those exact values never land in
/// the table's tiny 20.00-20.02 or 23.00-23.01 sub-steps and the ratio this
/// produces is the same either way.
pub const ACF_SLAT_DETENT_DEG: [f64; 6] = [0., 20.00, 20.01, 20.02, 23.00, 23.01];

/// X-Plane's flap ratio for a flap angle: the inverse of X-Plane's detent
/// table, taking deflection as linear in the ratio between detents. Angles
/// past the last detent give 1.
pub fn flap_ratio_for_angle(deg: f64, detents: &[f64]) -> f64 {
    let steps = (detents.len() - 1) as f64;
    if deg <= detents[0] {
        return 0.;
    }
    for i in 0..detents.len() - 1 {
        let (lo, hi) = (detents[i], detents[i + 1]);
        if deg <= hi && hi > lo {
            return (i as f64 + (deg - lo) / (hi - lo)) / steps;
        }
    }
    1.
}

/// X-Plane's slat ratio for a slat angle: the same detent-index inverse as
/// `flap_ratio_for_angle`, against the slat's own (flatter) detent table.
/// `slat1_deploy_ratio` is not itself a physical deployment fraction: X-Plane
/// looks it up BY INDEX into `acf/_slat1_dn`, which already holds the
/// physical fraction (`ACF_SLAT_DETENT_DEG[i] / ACF_SLAT_MAX_DEG`) at that
/// index. Scaling linearly by `deg / ACF_SLAT_MAX_DEG` conflated the two
/// scales: at FlyByWire's first slat stop (20 deg, detent index 1 of 5, the
/// correct ratio is 0.2) the old code wrote `20./23. = 0.87`, landing between
/// detents 4 and 5 (both 23 deg, ratio 1.0) and reporting the slats as fully
/// deployed at their very first, partial extension.
pub fn slat_ratio_for_angle(deg: f64) -> f64 {
    flap_ratio_for_angle(deg, &ACF_SLAT_DETENT_DEG)
}

/// FlyByWire's steering actuators' travel (a380_systems hydraulic/mod.rs:1792,
/// 1803, 1814): the `_POSITION_RATIO` they write is angle over this
/// (nose_steering.rs:276-280).
pub const NOSE_STEERING_MAX_DEG: f64 = 75.;
pub const BODY_STEERING_MAX_DEG: f64 = 15.;

/// Tyre steering for X-Plane's five legs, degrees positive right: nose from
/// the nose actuator, the body gears (`_gear/1`, `_gear/2`) from theirs, the
/// wing gears fixed. FlyByWire's ratios are positive right (the tiller is
/// "-1 is left", nose_wheel_steering.rs:105; body gear demand opposes the nose,
/// hydraulic/mod.rs:4148-4174, which these already carry).
pub fn tyre_steer_deg(nose_ratio: f64, left_body_ratio: f64, right_body_ratio: f64) -> [f64; 5] {
    [
        nose_ratio * NOSE_STEERING_MAX_DEG,
        left_body_ratio * BODY_STEERING_MAX_DEG,
        right_body_ratio * BODY_STEERING_MAX_DEG,
        0.,
        0.,
    ]
}

/// Nosewheel angle above which the brake control system releases the body
/// gear brakes and the aft wing-gear axle brakes (FCOM PRO-NOR-SOP-100, brake
/// pivot turn).
pub const PIVOT_NWS_DEG: f64 = 20.;
/// Braked wheels left per side in a pivot: the forward wing-gear axle, 2 of
/// FlyByWire's 4 wing + 4 body brakes per side (hydraulic/mod.rs:1733-1736).
pub const PIVOT_BRAKE_SHARE: f64 = 2. / 8.;

/// Share of a side's pedal brake force X-Plane gets: X-Plane brakes every
/// braked leg of a side alike, so the released brakes become a smaller
/// per-side ratio. The parking brake (body gear only, FCOM DSC-32-10-30-30)
/// is left alone.
pub fn pivot_brake_share(nose_deg: f64, park_brake_on: bool) -> f64 {
    if !park_brake_on && nose_deg.abs() > PIVOT_NWS_DEG {
        PIVOT_BRAKE_SHARE
    } else {
        1.
    }
}

/// Body gear brake pressure with the parking brake set and the accumulators
/// filled, psi (FCOM DSC-32-10-30-30-60).
pub const PARK_BRAKE_FULL_PRESSURE_PSI: f64 = 3600.;

/// X-Plane's parking brake ratio: FlyByWire's body gear parking brake
/// pressure (lower side) over the full parking pressure, zero with the lever off.
pub fn park_brake_hold(lever_on: bool, left_body_psi: f64, right_body_psi: f64) -> f64 {
    if !lever_on {
        return 0.;
    }
    (left_body_psi.min(right_body_psi) / PARK_BRAKE_FULL_PRESSURE_PSI).clamp(0., 1.)
}

/// Alternate NWS rate, deg/s (FCOM DSC-32-10-40; normal is 15, FlyByWire's
/// actuator rate).
pub const NWS_ALTERNATE_RATE_DEG_S: f64 = 10.;

/// The nosewheel angle X-Plane gets: FlyByWire's actuator angle, followed at
/// the alternate rate while the green system is lost (alternate NWS).
pub fn nose_steer_follow(previous_deg: f64, actuator_deg: f64, dt: f64, alternate: bool) -> f64 {
    if !alternate {
        return actuator_deg;
    }
    let step = NWS_ALTERNATE_RATE_DEG_S * dt.max(0.);
    previous_deg + (actuator_deg - previous_deg).clamp(-step, step)
}

/// What a cockpit lever the converter animates (a `fbw/cockpit/...` dataref
/// SASL owns) did, and what it should show.
#[derive(Debug, PartialEq)]
pub enum LeverAction {
    /// The pilot moved it: the value it was moved to.
    Moved(f64),
    /// The systems changed what it stands for: move it to this.
    Show(f64),
    None,
}

/// Follows one cockpit lever both ways without fighting a drag: the pilot's
/// movement is taken when the lever changes, and the lever is only moved to
/// FlyByWire's value when that value changes.
#[derive(Default)]
pub struct LeverBridge {
    lever: Option<f64>,
    fbw: Option<f64>,
}

impl LeverBridge {
    pub fn update(&mut self, lever: f64, fbw_as_lever: f64) -> LeverAction {
        const EPS: f64 = 1e-4;
        let action = match (self.lever, self.fbw) {
            (Some(last), _) if (lever - last).abs() > EPS => LeverAction::Moved(lever),
            (_, Some(last)) if (fbw_as_lever - last).abs() > EPS && (lever - fbw_as_lever).abs() > EPS => {
                LeverAction::Show(fbw_as_lever)
            }
            (None, _) if (lever - fbw_as_lever).abs() > EPS => LeverAction::Show(fbw_as_lever),
            _ => LeverAction::None,
        };
        self.lever = Some(match action {
            LeverAction::Show(v) => v,
            _ => lever,
        });
        self.fbw = Some(fbw_as_lever);
        action
    }

    /// The lever was moved to `lever` from here, as when it snaps into a detent.
    pub fn placed(&mut self, lever: f64) {
        self.lever = Some(lever);
    }
}

/// The flap lever's detent for where it was left: handle index 0..4 at
/// quarters of its travel.
pub fn flap_detent(lever: f64) -> f64 {
    (lever * 4.).round().clamp(0., 4.) / 4.
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_flap_lever_settles_into_the_nearest_detent() {
        assert_eq!(flap_detent(0.37), 0.25);
        assert_eq!(flap_detent(0.38), 0.5);
        assert_eq!(flap_detent(1.2), 1.0);
        let mut b = LeverBridge::default();
        b.update(0.37, 0.25);
        b.placed(0.25);
        assert_eq!(b.update(0.25, 0.25), LeverAction::None, "the snap is not a pilot move");
    }

    #[test]
    fn gear_positions_go_to_the_legs_by_retract_group() {
        let (deploy, handle) = gear_deploy(100., 50., 20., 100., 100., 100.);
        assert_eq!(deploy, [1., 0.5, 0.2, 0.5, 0.2]);
        assert!(handle);
        let (up, handle) = gear_deploy(0., 0., 0., 0., 0., 0.);
        assert_eq!(up, [0.; 5]);
        assert!(!handle);
    }

    #[test]
    fn open_doors_with_gear_up_show_a_little_gear_for_drag() {
        // gear.rs:97-101: doors open, gear still in.
        let (deploy, handle) = gear_deploy(0., 4., 0., 100., 5., 60.);
        assert_eq!(deploy, [0.1, 0.05, 0.1, 0.05, 0.1]);
        assert!(!handle);
    }

    #[test]
    fn flap_angles_map_onto_the_acf_detents() {
        let d = &ACF_FLAP_DETENT_DEG;
        assert_eq!(flap_ratio_for_angle(0., d), 0.);
        assert!((flap_ratio_for_angle(8., d) - 0.4).abs() < 1e-12);
        assert!((flap_ratio_for_angle(17., d) - 0.6).abs() < 1e-12);
        assert!((flap_ratio_for_angle(21.5, d) - 0.7).abs() < 1e-12);
        assert!((flap_ratio_for_angle(26., d) - 0.8).abs() < 1e-12);
        assert!((flap_ratio_for_angle(29.5, d) - 0.9).abs() < 1e-12);
        // FlyByWire's FULL (hydraulic/mod.rs:1738-1739) is the .acf's last detent.
        assert_eq!(flap_ratio_for_angle(33., d), 1.);
        assert_eq!(flap_ratio_for_angle(34., d), 1.);
    }

    #[test]
    fn a_pivot_turn_leaves_only_the_forward_wing_axle_braking() {
        assert_eq!(pivot_brake_share(0., false), 1.);
        assert_eq!(pivot_brake_share(20., false), 1.);
        assert_eq!(pivot_brake_share(20.5, false), 0.25);
        assert_eq!(pivot_brake_share(-70., false), 0.25);
        assert_eq!(pivot_brake_share(-70., true), 1., "the parking brake is not released");
    }

    #[test]
    fn the_park_brake_hold_follows_body_gear_pressure() {
        assert_eq!(park_brake_hold(false, 3600., 3600.), 0.);
        assert_eq!(park_brake_hold(true, 3600., 3700.), 1.);
        assert!((park_brake_hold(true, 1800., 3600.) - 0.5).abs() < 1e-9, "the weaker side limits the hold");
        assert_eq!(park_brake_hold(true, 0., 0.), 0., "drained accumulators hold nothing");
    }

    #[test]
    fn alternate_nws_slews_at_10_deg_per_s() {
        assert_eq!(nose_steer_follow(0., 70., 0.1, false), 70.);
        let mut a = 0.;
        for _ in 0..10 {
            a = nose_steer_follow(a, 70., 0.1, true);
        }
        assert!((a - 10.).abs() < 1e-9);
        assert_eq!(nose_steer_follow(69.5, 70., 0.1, true), 70.);
        assert!((nose_steer_follow(5., -5., 0.5, true) - 0.).abs() < 1e-12);
    }

    #[test]
    fn slat_angles_are_a_share_of_the_acf_maximum() {
        // hydraulic/mod.rs:1744-1745: slats 20 then 23 degrees.
        // Detent index 1 of 5 (FlyByWire's first real slat stop, 20 deg):
        // ratio 0.2, not 20./23. (the old, buggy linear scale).
        assert!((slat_ratio_for_angle(20.) - 0.2).abs() < 1e-12);
        // Mid-transit between 0 deg (index 0) and 20 deg (index 1).
        assert!((slat_ratio_for_angle(10.) - 0.1).abs() < 1e-12);
        // Detent index 3 (20 deg) to index 4 (23 deg, ratio 0.8): equivalent
        // to full deployment since index 4 and 5 are both 23 deg (ratio 1.0)
        // in acf/_slat1_dn, so X-Plane interpolates flat from here to 1.0.
        assert!((slat_ratio_for_angle(23.) - 0.8).abs() < 1e-12);
        assert_eq!(slat_ratio_for_angle(24.), 1.);
        assert_eq!(slat_ratio_for_angle(-1.), 0.);
    }

    #[test]
    fn steering_ratios_become_tyre_angles() {
        assert_eq!(tyre_steer_deg(1., -1., 0.5), [75., -15., 7.5, 0., 0.]);
    }

    #[test]
    fn a_lever_follows_the_pilot_and_the_systems_without_fighting() {
        let mut b = LeverBridge::default();
        // First sight: the lever is put where the systems are.
        assert_eq!(b.update(0., 0.5), LeverAction::Show(0.5));
        // Nothing changed.
        assert_eq!(b.update(0.5, 0.5), LeverAction::None);
        // Pilot drags part way: taken, and not pushed back while the systems
        // have not changed.
        assert_eq!(b.update(0.6, 0.5), LeverAction::Moved(0.6));
        assert_eq!(b.update(0.6, 0.5), LeverAction::None);
        // The systems change (a key press): the lever is moved.
        assert_eq!(b.update(0.6, 1.), LeverAction::Show(1.));
        assert_eq!(b.update(1., 1.), LeverAction::None);
    }
}

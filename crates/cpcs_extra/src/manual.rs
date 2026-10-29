pub const VS_MIN_FPM: f64 = -1500.0;
pub const VS_MAX_FPM: f64 = 2500.0;
pub const VS_STEP_FPM: f64 = 50.0;
pub const ALT_MIN_FT: f64 = -2000.0;
pub const ALT_MAX_FT: f64 = 14000.0;
pub const ALT_STEP_FT: f64 = 100.0;
pub const ALT_DEFAULT_FT: f64 = 0.0;
pub const VS_DEFAULT_FPM: f64 = 0.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Knob {
    value: f64,
    min: f64,
    max: f64,
    step: f64,
}

impl Knob {
    pub fn new(value: f64, min: f64, max: f64, step: f64) -> Self {
        let mut k = Self { value: 0.0, min, max, step };
        k.set(value);
        k
    }

    pub fn value(&self) -> f64 {
        self.value
    }

    pub fn set(&mut self, v: f64) {
        let v = if v.is_finite() { v } else { self.value };
        self.value = ((v.clamp(self.min, self.max) / self.step).round() * self.step).clamp(self.min, self.max);
    }

    pub fn click(&mut self, clicks: i32) {
        self.set(self.value + clicks as f64 * self.step);
    }

    pub fn drag(&mut self, fraction_of_range: f64) {
        self.set(self.value + fraction_of_range * (self.max - self.min));
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Panel {
    pub altitude: Knob,
    pub vertical_speed: Knob,
}

impl Default for Panel {
    fn default() -> Self {
        Self {
            altitude: Knob::new(ALT_DEFAULT_FT, ALT_MIN_FT, ALT_MAX_FT, ALT_STEP_FT),
            vertical_speed: Knob::new(VS_DEFAULT_FPM, VS_MIN_FPM, VS_MAX_FPM, VS_STEP_FPM),
        }
    }
}

impl Panel {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn seed_altitude_from_cabin(&mut self, cabin_alt_ft: f64) {
        self.altitude.set(cabin_alt_ft);
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Advisory {
    pub expect_high_cabin_rate: bool,
    pub target_unreachable_at_rate: bool,
    pub minutes_to_target: Option<f64>,
}

pub fn advisory(
    alt_man: bool,
    vs_man: bool,
    cabin_alt_ft: f64,
    alt_target_ft: f64,
    vs_target_fpm: f64,
    delta_p_psi: f64,
) -> Advisory {
    let rate = if alt_man && !vs_man {
        let gap = alt_target_ft - cabin_alt_ft;
        if gap.abs() < 1.0 { 0.0 } else if gap > 0.0 { 500.0_f64.min(gap) } else { (-300.0_f64).max(gap) }
    } else if vs_man {
        vs_target_fpm
    } else {
        0.0
    };
    let moving_toward = alt_man && (alt_target_ft - cabin_alt_ft) * rate > 0.0;
    let minutes_to_target = if moving_toward { Some((alt_target_ft - cabin_alt_ft) / rate) } else { None };
    Advisory {
        expect_high_cabin_rate: (alt_man || vs_man) && (delta_p_psi >= 8.6 || delta_p_psi <= 0.5),
        target_unreachable_at_rate: alt_man && vs_man && !moving_toward && (alt_target_ft - cabin_alt_ft).abs() > 50.0,
        minutes_to_target,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn knobs_clamp_and_snap_to_their_step() {
        let mut p = Panel::new();
        p.vertical_speed.set(130.0);
        assert_eq!(p.vertical_speed.value(), 150.0);
        p.vertical_speed.set(9000.0);
        assert_eq!(p.vertical_speed.value(), VS_MAX_FPM);
        p.vertical_speed.set(-9000.0);
        assert_eq!(p.vertical_speed.value(), VS_MIN_FPM);
        p.altitude.set(7432.0);
        assert_eq!(p.altitude.value(), 7400.0);
    }

    #[test]
    fn clicks_step_and_stop_at_the_ends() {
        let mut k = Knob::new(0.0, VS_MIN_FPM, VS_MAX_FPM, VS_STEP_FPM);
        k.click(3);
        assert_eq!(k.value(), 150.0);
        k.click(-100);
        assert_eq!(k.value(), VS_MIN_FPM);
        k.click(1000);
        assert_eq!(k.value(), VS_MAX_FPM);
    }

    #[test]
    fn dragging_moves_a_fraction_of_the_range() {
        let mut k = Knob::new(0.0, 0.0, 1000.0, 10.0);
        k.drag(0.25);
        assert_eq!(k.value(), 250.0);
        k.drag(f64::NAN);
        assert_eq!(k.value(), 250.0);
    }

    #[test]
    fn the_altitude_knob_can_be_seeded_from_the_current_cabin() {
        let mut p = Panel::new();
        p.seed_altitude_from_cabin(6538.0);
        assert_eq!(p.altitude.value(), 6500.0);
    }

    #[test]
    fn altitude_manual_alone_uses_the_default_climb_and_descent_rates() {
        let up = advisory(true, false, 4000.0, 8000.0, 0.0, 5.0);
        assert_eq!(up.minutes_to_target, Some(8.0));
        let down = advisory(true, false, 8000.0, 6000.0, 0.0, 5.0);
        assert!((down.minutes_to_target.unwrap() - 2000.0 / 300.0).abs() < 1e-9);
    }

    #[test]
    fn a_manual_rate_pointing_away_from_the_target_is_flagged() {
        let a = advisory(true, true, 4000.0, 8000.0, -500.0, 5.0);
        assert!(a.target_unreachable_at_rate);
        assert_eq!(a.minutes_to_target, None);
        let ok = advisory(true, true, 4000.0, 8000.0, 500.0, 5.0);
        assert!(!ok.target_unreachable_at_rate);
        assert_eq!(ok.minutes_to_target, Some(8.0));
    }

    #[test]
    fn auto_mode_gives_no_advisory() {
        let a = advisory(false, false, 4000.0, 8000.0, 500.0, 9.0);
        assert!(!a.expect_high_cabin_rate);
        assert_eq!(a.minutes_to_target, None);
    }

    #[test]
    fn manual_at_the_pressure_limits_expects_a_high_cabin_rate() {
        assert!(advisory(true, false, 4000.0, 8000.0, 0.0, 8.8).expect_high_cabin_rate);
        assert!(advisory(false, true, 4000.0, 8000.0, 0.0, 0.2).expect_high_cabin_rate);
        assert!(!advisory(false, true, 4000.0, 8000.0, 0.0, 5.0).expect_high_cabin_rate);
    }
}

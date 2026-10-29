pub const MASK_ALT_FT: f64 = 14000.0;
pub const TARGET_LEVEL_FT: f64 = 10000.0;
pub const LOW_DELTA_P_PSI: f64 = 1.0;

const TRIGGER_DEBOUNCE_S: f64 = 1.0;
const UNCONTROLLED_HOLD_S: f64 = 5.0;
const TIME_TO_MASKS_RECOMMEND_S: f64 = 60.0;
const ESTABLISHED_VS_FPM: f64 = -1500.0;
const ESTABLISHED_HOLD_S: f64 = 3.0;
const LEVELLED_VS_FPM: f64 = 500.0;
const CABIN_RECOVERED_FT: f64 = 10000.0;
const CLEAR_HOLD_S: f64 = 30.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Inactive,
    Recommended,
    Established,
    Recovered,
}

impl Phase {
    pub fn code(self) -> f64 {
        match self {
            Phase::Inactive => 0.0,
            Phase::Recommended => 1.0,
            Phase::Established => 2.0,
            Phase::Recovered => 3.0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Inputs {
    pub excess_alt: bool,
    pub cabin_alt_ft: f64,
    pub cabin_vs_fpm: f64,
    pub delta_p_psi: f64,
    pub aircraft_alt_ft: f64,
    pub aircraft_vs_fpm: f64,
    pub on_ground: bool,
    pub dt_s: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Outputs {
    pub phase: Phase,
    pub time_to_mask_alt_s: Option<f64>,
    pub masks_expected: bool,
    pub announce_pa: bool,
    pub recommend_idle_thrust: bool,
    pub recommend_speed_brake: bool,
    pub target_level_ft: f64,
    pub cabin_recovered: bool,
}

#[derive(Clone, Debug)]
pub struct Monitor {
    phase: Phase,
    excess_s: f64,
    uncontrolled_s: f64,
    established_s: f64,
    clear_s: f64,
}

impl Default for Monitor {
    fn default() -> Self {
        Self { phase: Phase::Inactive, excess_s: 0.0, uncontrolled_s: 0.0, established_s: 0.0, clear_s: 0.0 }
    }
}

pub fn time_to_mask_alt_s(cabin_alt_ft: f64, cabin_vs_fpm: f64) -> Option<f64> {
    if cabin_alt_ft >= MASK_ALT_FT {
        Some(0.0)
    } else if cabin_vs_fpm > 50.0 {
        Some((MASK_ALT_FT - cabin_alt_ft) / cabin_vs_fpm * 60.0)
    } else {
        None
    }
}

impl Monitor {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn phase(&self) -> Phase {
        self.phase
    }

    pub fn update(&mut self, i: &Inputs) -> Outputs {
        let dt = i.dt_s.max(0.0);
        let time_to_mask = time_to_mask_alt_s(i.cabin_alt_ft, i.cabin_vs_fpm);
        let active = i.excess_alt && !i.on_ground;
        let previous = self.phase;

        if i.on_ground {
            *self = Self::default();
        } else {
            self.excess_s = if active { self.excess_s + dt } else { 0.0 };
            let triggered = self.excess_s >= TRIGGER_DEBOUNCE_S;
            let uncontrolled = i.cabin_alt_ft >= MASK_ALT_FT
                || (i.delta_p_psi < LOW_DELTA_P_PSI && i.cabin_alt_ft > 9000.0)
                || time_to_mask.is_some_and(|t| t <= TIME_TO_MASKS_RECOMMEND_S);

            match self.phase {
                Phase::Inactive => {
                    self.uncontrolled_s = if triggered && uncontrolled { self.uncontrolled_s + dt } else { 0.0 };
                    if self.uncontrolled_s >= UNCONTROLLED_HOLD_S {
                        self.phase = Phase::Recommended;
                    }
                }
                Phase::Recommended => {
                    self.established_s = if i.aircraft_vs_fpm <= ESTABLISHED_VS_FPM { self.established_s + dt } else { 0.0 };
                    if self.established_s >= ESTABLISHED_HOLD_S {
                        self.phase = Phase::Established;
                        self.established_s = 0.0;
                    }
                }
                Phase::Established => {
                    let levelled = i.aircraft_alt_ft <= TARGET_LEVEL_FT + 500.0 && i.aircraft_vs_fpm.abs() < LEVELLED_VS_FPM;
                    if levelled && i.cabin_alt_ft <= CABIN_RECOVERED_FT {
                        self.phase = Phase::Recovered;
                    }
                }
                Phase::Recovered => {
                    self.clear_s = if !i.excess_alt { self.clear_s + dt } else { 0.0 };
                    if self.clear_s >= CLEAR_HOLD_S {
                        *self = Self::default();
                    }
                }
            }
        }

        let in_procedure = matches!(self.phase, Phase::Recommended | Phase::Established);
        Outputs {
            phase: self.phase,
            time_to_mask_alt_s: time_to_mask,
            masks_expected: i.cabin_alt_ft >= MASK_ALT_FT,
            announce_pa: previous == Phase::Inactive && self.phase == Phase::Recommended,
            recommend_idle_thrust: in_procedure,
            recommend_speed_brake: in_procedure,
            target_level_ft: TARGET_LEVEL_FT,
            cabin_recovered: i.cabin_alt_ft <= CABIN_RECOVERED_FT,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn climbing_cabin() -> Inputs {
        Inputs {
            excess_alt: true,
            cabin_alt_ft: 11000.0,
            cabin_vs_fpm: 3000.0,
            delta_p_psi: 4.0,
            aircraft_alt_ft: 38000.0,
            aircraft_vs_fpm: 0.0,
            on_ground: false,
            dt_s: 0.5,
        }
    }

    fn run(m: &mut Monitor, i: &Inputs, seconds: f64) -> Outputs {
        let mut last = m.update(i);
        let mut t = 0.0;
        while t < seconds {
            last = m.update(i);
            t += i.dt_s;
        }
        last
    }

    #[test]
    fn time_to_masks_follows_cabin_rate() {
        assert_eq!(time_to_mask_alt_s(11000.0, 3000.0), Some(60.0));
        assert_eq!(time_to_mask_alt_s(8000.0, 0.0), None);
        assert_eq!(time_to_mask_alt_s(8000.0, -500.0), None);
        assert_eq!(time_to_mask_alt_s(14200.0, -500.0), Some(0.0));
    }

    #[test]
    fn a_fast_climbing_excess_alt_recommends_an_emergency_descent_with_one_announcement() {
        let mut m = Monitor::new();
        let i = climbing_cabin();
        let mut announces = 0;
        let mut phase = Phase::Inactive;
        for _ in 0..40 {
            let o = m.update(&i);
            announces += o.announce_pa as u32;
            phase = o.phase;
        }
        assert_eq!(phase, Phase::Recommended);
        assert_eq!(announces, 1);
    }

    #[test]
    fn a_slow_excess_alt_that_is_already_recovering_does_not_escalate() {
        let mut m = Monitor::new();
        let i = Inputs { cabin_alt_ft: 9800.0, cabin_vs_fpm: -300.0, delta_p_psi: 5.0, ..climbing_cabin() };
        let o = run(&mut m, &i, 60.0);
        assert_eq!(o.phase, Phase::Inactive);
        assert!(!o.recommend_idle_thrust);
    }

    #[test]
    fn masks_are_expected_at_fourteen_thousand() {
        let mut m = Monitor::new();
        let i = Inputs { cabin_alt_ft: 14100.0, ..climbing_cabin() };
        assert!(m.update(&i).masks_expected);
        let i = Inputs { cabin_alt_ft: 13900.0, ..climbing_cabin() };
        assert!(!m.update(&i).masks_expected);
    }

    #[test]
    fn low_differential_pressure_at_altitude_escalates_even_with_a_flat_cabin() {
        let mut m = Monitor::new();
        let i = Inputs { cabin_alt_ft: 10000.0, cabin_vs_fpm: 0.0, delta_p_psi: 0.4, ..climbing_cabin() };
        let o = run(&mut m, &i, 20.0);
        assert_eq!(o.phase, Phase::Recommended);
    }

    #[test]
    fn the_descent_is_established_then_recovered_then_cleared() {
        let mut m = Monitor::new();
        run(&mut m, &climbing_cabin(), 20.0);
        assert_eq!(m.phase(), Phase::Recommended);

        let descending = Inputs { aircraft_vs_fpm: -6000.0, aircraft_alt_ft: 30000.0, cabin_alt_ft: 13000.0, cabin_vs_fpm: -200.0, ..climbing_cabin() };
        let o = run(&mut m, &descending, 10.0);
        assert_eq!(o.phase, Phase::Established);
        assert!(o.recommend_speed_brake && o.recommend_idle_thrust);

        let levelled = Inputs { aircraft_vs_fpm: 0.0, aircraft_alt_ft: 10000.0, cabin_alt_ft: 9800.0, cabin_vs_fpm: -300.0, excess_alt: false, ..descending };
        let o = run(&mut m, &levelled, 5.0);
        assert_eq!(o.phase, Phase::Recovered);
        assert!(o.cabin_recovered);
        assert!(!o.recommend_idle_thrust);

        let o = run(&mut m, &levelled, 40.0);
        assert_eq!(o.phase, Phase::Inactive);
    }

    #[test]
    fn landing_resets_everything() {
        let mut m = Monitor::new();
        run(&mut m, &climbing_cabin(), 20.0);
        assert_eq!(m.phase(), Phase::Recommended);
        let o = m.update(&Inputs { on_ground: true, ..climbing_cabin() });
        assert_eq!(o.phase, Phase::Inactive);
    }

    #[test]
    fn a_blip_shorter_than_the_debounce_never_triggers() {
        let mut m = Monitor::new();
        let mut i = climbing_cabin();
        for _ in 0..20 {
            i.excess_alt = true;
            m.update(&Inputs { dt_s: 0.4, ..i });
            i.excess_alt = false;
            m.update(&Inputs { dt_s: 0.4, ..i });
        }
        assert_eq!(m.phase(), Phase::Inactive);
    }
}

pub const POSITIVE_OPEN_PSI: f64 = 9.0;
pub const POSITIVE_FULL_PSI: f64 = 9.5;
pub const NEGATIVE_OPEN_PSI: f64 = -0.725;
pub const NEGATIVE_FULL_PSI: f64 = -1.225;
pub const TRAVEL_S: f64 = 1.0;
pub const VALVE_AREA_M2: f64 = 0.1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fault {
    None,
    StuckClosed,
    StuckOpen,
}

#[derive(Clone, Copy, Debug)]
pub struct ReliefValve {
    open: f64,
    pub fault: Fault,
}

impl Default for ReliefValve {
    fn default() -> Self {
        Self { open: 0.0, fault: Fault::None }
    }
}

impl ReliefValve {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn open_fraction(&self) -> f64 {
        self.open
    }

    pub fn open_percent(&self) -> f64 {
        self.open * 100.0
    }

    pub fn leak_area_m2(&self) -> f64 {
        self.open * VALVE_AREA_M2
    }

    pub fn is_relieving(&self) -> bool {
        self.open > 0.01
    }

    pub fn step(&mut self, delta_p_psi: f64, dt_s: f64) -> f64 {
        let target = match self.fault {
            Fault::StuckClosed => 0.0,
            Fault::StuckOpen => 1.0,
            Fault::None => self.target(delta_p_psi),
        };
        let max_move = if TRAVEL_S > 0.0 { dt_s.max(0.0) / TRAVEL_S } else { 1.0 };
        let delta = (target - self.open).clamp(-max_move, max_move);
        self.open = (self.open + delta).clamp(0.0, 1.0);
        self.open
    }

    fn target(&self, dp: f64) -> f64 {
        if dp >= POSITIVE_OPEN_PSI {
            ((dp - POSITIVE_OPEN_PSI) / (POSITIVE_FULL_PSI - POSITIVE_OPEN_PSI)).clamp(0.0, 1.0)
        } else if dp <= NEGATIVE_OPEN_PSI {
            ((NEGATIVE_OPEN_PSI - dp) / (NEGATIVE_OPEN_PSI - NEGATIVE_FULL_PSI)).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settle(v: &mut ReliefValve, dp: f64) -> f64 {
        let mut o = 0.0;
        for _ in 0..40 {
            o = v.step(dp, 0.1);
        }
        o
    }

    #[test]
    fn closed_in_the_normal_band() {
        let mut v = ReliefValve::new();
        assert_eq!(settle(&mut v, 8.0), 0.0);
        assert_eq!(settle(&mut v, 0.0), 0.0);
        assert!(!v.is_relieving());
    }

    #[test]
    fn opens_progressively_above_the_positive_limit() {
        let mut v = ReliefValve::new();
        let half = settle(&mut v, 9.25);
        assert!((half - 0.5).abs() < 1e-9);
        assert_eq!(settle(&mut v, 9.8), 1.0);
        assert!((v.leak_area_m2() - VALVE_AREA_M2).abs() < 1e-12);
    }

    #[test]
    fn opens_below_the_negative_limit_and_recloses() {
        let mut v = ReliefValve::new();
        assert_eq!(settle(&mut v, -1.5), 1.0);
        assert_eq!(settle(&mut v, 1.0), 0.0);
    }

    #[test]
    fn travel_takes_about_a_second() {
        let mut v = ReliefValve::new();
        let a = v.step(10.0, 0.5);
        assert!((a - 0.5).abs() < 1e-9);
        let b = v.step(10.0, 0.5);
        assert_eq!(b, 1.0);
    }

    #[test]
    fn a_valve_stuck_closed_never_relieves_but_stuck_open_leaks() {
        let mut c = ReliefValve::new();
        c.fault = Fault::StuckClosed;
        assert_eq!(settle(&mut c, 10.0), 0.0);
        let mut o = ReliefValve::new();
        o.fault = Fault::StuckOpen;
        assert_eq!(settle(&mut o, 4.0), 1.0);
        assert!(o.is_relieving());
    }
}

const HISS_TAU_S: f64 = 1.5;
const ROAR_TAU_S: f64 = 12.0;
const BANG_RATE_PSI_PER_S: f64 = -1.5;
const BANG_MIN_DELTA_P_PSI: f64 = 1.0;
const POP_CLIMB_FT: f64 = 600.0;
const POP_DESCENT_FT: f64 = 300.0;
const POP_MIN_RATE_FPM: f64 = 150.0;
const MAX_DELTA_P_PSI: f64 = 9.0;

#[derive(Clone, Copy, Debug, Default)]
pub struct Inputs {
    pub cabin_alt_ft: f64,
    pub cabin_vs_fpm: f64,
    pub delta_p_psi: f64,
    pub outflow_open_pct: f64,
    pub safety_valve_pct: f64,
    pub on_ground: bool,
    pub door_open: bool,
    pub hull_breach: bool,
    pub dt_s: f64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Outputs {
    pub outflow_hiss_gain: f64,
    pub outflow_hiss_pitch: f64,
    pub safety_valve_gain: f64,
    pub ear_pop: bool,
    pub decompression_bang: bool,
    pub decompression_roar_gain: f64,
    pub residual_hiss_gain: f64,
    pub cabin_density_gain: f64,
}

#[derive(Clone, Debug, Default)]
pub struct CabinPressureSound {
    hiss: f64,
    roar: f64,
    pop_accumulator_ft: f64,
    previous_delta_p: Option<f64>,
}

pub fn density_gain(cabin_alt_ft: f64) -> f64 {
    1.0 - 0.28 * (cabin_alt_ft / 14000.0).clamp(0.0, 1.0)
}

impl CabinPressureSound {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update(&mut self, i: &Inputs) -> Outputs {
        let dt = i.dt_s.max(0.0);
        let dp = i.delta_p_psi.max(0.0);
        let dp_norm = (dp / MAX_DELTA_P_PSI).clamp(0.0, 1.0);

        let open = (i.outflow_open_pct / 100.0).clamp(0.0, 1.0);
        let airborne_flow = if i.on_ground { 0.0 } else { (dp / 0.5).clamp(0.0, 1.0) };
        let hiss_target = airborne_flow * (0.15 + 0.85 * dp_norm.sqrt()) * (0.3 + 0.7 * open);
        let k = if HISS_TAU_S > 0.0 { (dt / HISS_TAU_S).min(1.0) } else { 1.0 };
        self.hiss += (hiss_target - self.hiss) * k;

        let rate = match (self.previous_delta_p, dt > 0.0) {
            (Some(prev), true) => (i.delta_p_psi - prev) / dt,
            _ => 0.0,
        };
        let bang = rate <= BANG_RATE_PSI_PER_S && self.previous_delta_p.is_some_and(|p| p >= BANG_MIN_DELTA_P_PSI);
        if bang {
            let strength = (self.previous_delta_p.unwrap_or(0.0) / MAX_DELTA_P_PSI).clamp(0.2, 1.0);
            let scale = if i.hull_breach { 1.0 } else { 0.6 };
            self.roar = self.roar.max(strength * scale);
        }
        self.previous_delta_p = Some(i.delta_p_psi);

        let roar_floor = if i.hull_breach && !i.on_ground { (dp / 5.0).clamp(0.0, 1.0) } else { 0.0 };
        self.roar *= (-dt / ROAR_TAU_S).exp();
        self.roar = self.roar.max(roar_floor);

        let ear_pop = self.ear_pop(i.cabin_vs_fpm, dt);

        Outputs {
            outflow_hiss_gain: self.hiss,
            outflow_hiss_pitch: 0.85 + 0.3 * dp_norm.sqrt(),
            safety_valve_gain: (i.safety_valve_pct / 100.0).clamp(0.0, 1.0) * 0.8,
            ear_pop,
            decompression_bang: bang,
            decompression_roar_gain: self.roar,
            residual_hiss_gain: if i.door_open && dp > 0.05 { (dp / 0.5).clamp(0.0, 1.0) } else { 0.0 },
            cabin_density_gain: density_gain(i.cabin_alt_ft),
        }
    }

    fn ear_pop(&mut self, vs_fpm: f64, dt: f64) -> bool {
        if vs_fpm.abs() < POP_MIN_RATE_FPM {
            self.pop_accumulator_ft *= (-dt / 20.0).exp();
            return false;
        }
        self.pop_accumulator_ft += vs_fpm / 60.0 * dt;
        if self.pop_accumulator_ft >= POP_CLIMB_FT || self.pop_accumulator_ft <= -POP_DESCENT_FT {
            self.pop_accumulator_ft = 0.0;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cruise() -> Inputs {
        Inputs { cabin_alt_ft: 6000.0, cabin_vs_fpm: 0.0, delta_p_psi: 8.0, outflow_open_pct: 20.0, dt_s: 0.1, ..Inputs::default() }
    }

    fn run(s: &mut CabinPressureSound, i: &Inputs, seconds: f64) -> Outputs {
        let mut o = Outputs::default();
        let mut t = 0.0;
        while t < seconds {
            o = s.update(i);
            t += i.dt_s;
        }
        o
    }

    #[test]
    fn cruise_has_an_audible_outflow_hiss_and_the_ground_has_none() {
        let mut s = CabinPressureSound::new();
        let o = run(&mut s, &cruise(), 15.0);
        assert!(o.outflow_hiss_gain > 0.3, "{}", o.outflow_hiss_gain);
        let mut g = CabinPressureSound::new();
        let o = run(&mut g, &Inputs { on_ground: true, delta_p_psi: 0.0, outflow_open_pct: 100.0, ..cruise() }, 15.0);
        assert!(o.outflow_hiss_gain < 0.01);
    }

    #[test]
    fn more_differential_pressure_raises_the_hiss_and_its_pitch() {
        let mut a = CabinPressureSound::new();
        let mut b = CabinPressureSound::new();
        let low = run(&mut a, &Inputs { delta_p_psi: 2.0, ..cruise() }, 15.0);
        let high = run(&mut b, &Inputs { delta_p_psi: 8.5, ..cruise() }, 15.0);
        assert!(high.outflow_hiss_gain > low.outflow_hiss_gain);
        assert!(high.outflow_hiss_pitch > low.outflow_hiss_pitch);
    }

    #[test]
    fn the_hiss_is_smoothed() {
        let mut s = CabinPressureSound::new();
        let first = s.update(&cruise());
        assert!(first.outflow_hiss_gain < 0.1);
    }

    #[test]
    fn a_sudden_loss_of_differential_pressure_bangs_once_and_roars_while_breached() {
        let mut s = CabinPressureSound::new();
        run(&mut s, &cruise(), 5.0);
        let breach = Inputs { hull_breach: true, delta_p_psi: 3.0, ..cruise() };
        let o = s.update(&breach);
        assert!(o.decompression_bang);
        assert!(o.decompression_roar_gain > 0.4);
        let next = s.update(&Inputs { delta_p_psi: 2.9, ..breach });
        assert!(!next.decompression_bang);
        assert!(next.decompression_roar_gain > 0.3);
    }

    #[test]
    fn a_slow_normal_depressurisation_does_not_bang() {
        let mut s = CabinPressureSound::new();
        let mut bangs = 0;
        let mut dp = 8.0;
        for _ in 0..200 {
            dp -= 0.01;
            bangs += s.update(&Inputs { delta_p_psi: dp, ..cruise() }).decompression_bang as u32;
        }
        assert_eq!(bangs, 0);
    }

    #[test]
    fn the_roar_dies_away_after_the_breach_is_gone() {
        let mut s = CabinPressureSound::new();
        run(&mut s, &cruise(), 5.0);
        s.update(&Inputs { delta_p_psi: 1.0, ..cruise() });
        let o = run(&mut s, &Inputs { delta_p_psi: 0.0, ..cruise() }, 60.0);
        assert!(o.decompression_roar_gain < 0.05);
    }

    #[test]
    fn ears_pop_every_few_hundred_feet_of_cabin_change() {
        let mut s = CabinPressureSound::new();
        let climb = Inputs { cabin_vs_fpm: 600.0, ..cruise() };
        let mut pops = 0;
        for _ in 0..1200 {
            pops += s.update(&climb).ear_pop as u32;
        }
        assert_eq!(pops, 2);
        let mut d = CabinPressureSound::new();
        let descend = Inputs { cabin_vs_fpm: -600.0, ..cruise() };
        let mut pops = 0;
        for _ in 0..600 {
            pops += d.update(&descend).ear_pop as u32;
        }
        assert_eq!(pops, 2);
    }

    #[test]
    fn a_steady_cabin_never_pops() {
        let mut s = CabinPressureSound::new();
        let mut pops = 0;
        for _ in 0..3000 {
            pops += s.update(&Inputs { cabin_vs_fpm: 100.0, ..cruise() }).ear_pop as u32;
        }
        assert_eq!(pops, 0);
    }

    #[test]
    fn opening_a_door_with_residual_pressure_hisses() {
        let mut s = CabinPressureSound::new();
        let o = s.update(&Inputs { on_ground: true, door_open: true, delta_p_psi: 0.3, ..cruise() });
        assert!(o.residual_hiss_gain > 0.5);
        let o = s.update(&Inputs { on_ground: true, door_open: true, delta_p_psi: 0.0, ..cruise() });
        assert_eq!(o.residual_hiss_gain, 0.0);
        let o = s.update(&Inputs { on_ground: true, door_open: false, delta_p_psi: 0.3, ..cruise() });
        assert_eq!(o.residual_hiss_gain, 0.0);
    }

    #[test]
    fn the_safety_valve_is_loud_only_when_open() {
        let mut s = CabinPressureSound::new();
        assert_eq!(s.update(&cruise()).safety_valve_gain, 0.0);
        let o = s.update(&Inputs { safety_valve_pct: 100.0, ..cruise() });
        assert!(o.safety_valve_gain > 0.7);
    }

    #[test]
    fn thin_cabin_air_is_quieter() {
        assert_eq!(density_gain(0.0), 1.0);
        assert!(density_gain(8000.0) < density_gain(2000.0));
        assert!((density_gain(14000.0) - 0.72).abs() < 1e-9);
        assert!((density_gain(40000.0) - 0.72).abs() < 1e-9);
    }
}

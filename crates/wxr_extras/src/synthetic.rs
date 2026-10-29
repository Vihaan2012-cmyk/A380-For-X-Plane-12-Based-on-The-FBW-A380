use crate::sample::{Point3, WeatherPoint, WeatherSampler};

#[derive(Clone, Copy, Debug)]
pub struct Microburst {
    pub east_m: f64,
    pub north_m: f64,
    pub radius_m: f64,
    pub peak_outflow_ms: f64,
    pub peak_downdraft_ms: f64,
    pub full_strength_to_m: f64,
    pub top_m: f64,
    pub precip: f32,
}

impl Microburst {
    pub fn new(east_m: f64, north_m: f64, radius_m: f64, peak_outflow_ms: f64, peak_downdraft_ms: f64) -> Self {
        Self { east_m, north_m, radius_m, peak_outflow_ms, peak_downdraft_ms, full_strength_to_m: 400., top_m: 1200., precip: 0. }
    }

    pub fn wet(mut self, precip: f32) -> Self {
        self.precip = precip;
        self
    }

    fn height_factor(&self, alt_m: f64) -> f64 {
        if alt_m <= self.full_strength_to_m {
            1.
        } else if alt_m >= self.top_m {
            0.
        } else {
            1. - (alt_m - self.full_strength_to_m) / (self.top_m - self.full_strength_to_m)
        }
    }

    fn wind(&self, p: Point3) -> (f64, f64, f64) {
        let (de, dn) = (p.east_m - self.east_m, p.north_m - self.north_m);
        let rho = de.hypot(dn);
        let h = self.height_factor(p.alt_m);
        let r = rho / self.radius_m;
        let out = self.peak_outflow_ms * r * (0.5 * (1. - r * r)).exp() * h;
        let (ue, un) = if rho > 1e-6 { (de / rho, dn / rho) } else { (0., 0.) };
        (out * ue, out * un, -self.peak_downdraft_ms * (-r * r).exp() * h)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct StormCell {
    pub east_m: f64,
    pub north_m: f64,
    pub radius_m: f64,
    pub peak_precip: f32,
    pub peak_turbulence: f32,
    pub base_m: f64,
    pub top_m: f64,
}

#[derive(Clone, Debug, Default)]
pub struct SyntheticField {
    pub background_wind: (f32, f32),
    pub bursts: Vec<Microburst>,
    pub storms: Vec<StormCell>,
    pub linear_shear: Option<LinearShear>,
}

#[derive(Clone, Copy, Debug)]
pub struct LinearShear {
    pub bearing_deg: f64,
    pub from_m: f64,
    pub to_m: f64,
    pub change_ms: f64,
    pub origin_east_m: f64,
    pub origin_north_m: f64,
}

impl WeatherSampler for SyntheticField {
    fn sample(&mut self, p: Point3) -> Option<WeatherPoint> {
        let mut w = WeatherPoint { wind_east_ms: self.background_wind.0, wind_north_ms: self.background_wind.1, temp_c: Some((15. - 6.5 * p.alt_m / 1000.) as f32), ..Default::default() };
        for b in &self.bursts {
            let (e, n, v) = b.wind(p);
            w.wind_east_ms += e as f32;
            w.wind_north_ms += n as f32;
            w.vertical_ms += v as f32;
            let rho = (p.east_m - b.east_m).hypot(p.north_m - b.north_m);
            if b.precip > 0. && rho < 1.5 * b.radius_m && p.alt_m < b.top_m {
                w.precip = w.precip.max(b.precip);
            }
        }
        if let Some(s) = &self.linear_shear {
            let (se, sn) = crate::geom::bearing_unit(s.bearing_deg);
            let along = (p.east_m - s.origin_east_m) * se + (p.north_m - s.origin_north_m) * sn;
            let t = ((along - s.from_m) / (s.to_m - s.from_m)).clamp(0., 1.);
            let d = s.change_ms * t;
            w.wind_east_ms += (d * se) as f32;
            w.wind_north_ms += (d * sn) as f32;
        }
        for c in &self.storms {
            let rho = (p.east_m - c.east_m).hypot(p.north_m - c.north_m);
            if rho < c.radius_m && p.alt_m >= c.base_m && p.alt_m <= c.top_m {
                let t = (1. - rho / c.radius_m) as f32;
                w.precip = w.precip.max(c.peak_precip * t.sqrt());
                w.turbulence = w.turbulence.max(c.peak_turbulence * t.sqrt());
                w.cloud_top_m = Some(c.top_m as f32);
            }
        }
        Some(w)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn microburst_blows_outward_and_sinks_in_the_middle() {
        let b = Microburst::new(0., 0., 1000., 20., 8.);
        let (e, _, w) = b.wind(Point3 { east_m: 1000., north_m: 0., alt_m: 100. });
        assert!((e - 20.).abs() < 1e-6);
        assert!(w < 0.);
        let (_, _, wc) = b.wind(Point3 { east_m: 0., north_m: 0., alt_m: 100. });
        assert!((wc + 8.).abs() < 1e-6);
        assert_eq!(b.wind(Point3 { east_m: 0., north_m: 0., alt_m: 5000. }).2, 0.);
    }

    #[test]
    fn storm_cell_precipitates_between_base_and_top_only() {
        let mut f = SyntheticField { storms: vec![StormCell { east_m: 0., north_m: 0., radius_m: 5000., peak_precip: 0.8, peak_turbulence: 0.7, base_m: 500., top_m: 9000. }], ..Default::default() };
        assert!(f.sample(Point3 { east_m: 0., north_m: 0., alt_m: 3000. }).unwrap().precip > 0.7);
        assert_eq!(f.sample(Point3 { east_m: 0., north_m: 0., alt_m: 10_000. }).unwrap().precip, 0.);
    }
}

// Translated from FlyByWire's fmgc guidance/vnav/common.ts (GPL-3.0-or-later).

pub const FT_PER_NM: f64 = 6076.12;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AccelFactorMode {
    ConstantCas,
    ConstantMach,
}

pub fn interpolate(x: f64, x0: f64, x1: f64, y0: f64, y1: f64) -> f64 {
    (y0 * (x1 - x) + y1 * (x - x0)) / (x1 - x0)
}

pub fn pounds_to_metric_tons(pounds: f64) -> f64 {
    pounds / 2204.6
}

pub fn isa_temp(alt: f64, above_tropo: bool) -> f64 {
    if above_tropo {
        -56.5
    } else {
        15.0 - 0.0019812 * alt
    }
}

pub fn temp(alt: f64, isa_dev: f64, above_tropo: bool) -> f64 {
    if above_tropo {
        -56.5 + isa_dev
    } else {
        15.0 - 0.0019812 * alt + isa_dev
    }
}

pub fn theta(alt: f64, isa_dev: f64, above_tropo: bool) -> f64 {
    if above_tropo {
        (216.65 + isa_dev) / 288.15
    } else {
        (288.15 - 0.0019812 * alt + isa_dev) / 288.15
    }
}

pub fn theta2(theta: f64, mach: f64) -> f64 {
    theta * (1.0 + 0.2 * mach * mach)
}

pub fn delta(alt: f64, above_tropo: bool) -> f64 {
    if above_tropo {
        0.22336 * ((36089.24 - alt) / 20805.7).exp()
    } else {
        theta(alt, 0.0, false).powf(5.25588)
    }
}

pub fn delta2(delta: f64, mach: f64) -> f64 {
    delta * (1.0 + 0.2 * mach * mach).powf(3.5)
}

pub fn mach_to_tas(mach: f64, theta: f64) -> f64 {
    661.4786 * mach * theta.sqrt()
}

pub fn mach_to_eas(mach: f64, delta: f64) -> f64 {
    661.4786 * mach * delta.sqrt()
}

pub fn cas_to_mach(cas: f64, delta: f64) -> f64 {
    let term1 = 1.0 + 0.2 * (cas / 661.4786).powi(2);
    let term2 = (1.0 / delta) * (term1.powf(3.5) - 1.0);
    let term3 = 5.0 * ((term2 + 1.0).powf(1.0 / 3.5) - 1.0);
    term3.sqrt()
}

pub fn mach_to_cas(mach: f64, delta: f64) -> f64 {
    let term1 = (0.2 * mach * mach + 1.0).powf(3.5);
    let term2 = (delta * (term1 - 1.0) + 1.0).powf(1.0 / 3.5) - 1.0;
    1479.1 * term2.sqrt()
}

pub fn tas_to_cas(tas: f64, theta: f64, delta: f64) -> f64 {
    let term1 = 1.0 + (1.0 / theta) * (tas / 1479.1).powi(2);
    let term2 = delta * (term1.powf(3.5) - 1.0) + 1.0;
    let term3 = term2.powf(1.0 / 3.5) - 1.0;
    1479.1 * term3.sqrt()
}

pub fn cas_to_tas(cas: f64, theta: f64, delta: f64) -> f64 {
    let term1 = 1.0 + 0.2 * (cas / 661.4786).powi(2);
    let term2 = (1.0 / delta) * (term1.powf(3.5) - 1.0);
    let term3 = theta * ((term2 + 1.0).powf(1.0 / 3.5) - 1.0);
    1479.1 * term3.sqrt()
}

pub fn cas_to_eas(cas: f64, delta: f64) -> f64 {
    let term1 = 1.0 + 0.2 * (cas / 661.4786).powi(2);
    let term2 = (1.0 / delta) * (term1.powf(3.5) - 1.0);
    let term3 = delta * ((term2 + 1.0).powf(1.0 / 3.5) - 1.0);
    1479.1 * term3.sqrt()
}

pub fn accel_factor_cas(mach: f64, above_tropo: bool, temp_ratio: f64) -> f64 {
    let m2 = mach * mach;
    let phi = ((1.0 + 0.2 * m2).powf(3.5) - 1.0) / (0.7 * m2 * (1.0 + 0.2 * m2).powf(2.5));
    if above_tropo {
        1.0 + 0.7 * m2 * phi
    } else {
        1.0 + 0.7 * m2 * (phi - 0.190263 * temp_ratio)
    }
}

pub fn accel_factor_mach(mach: f64, above_tropo: bool, temp_ratio: f64) -> f64 {
    if above_tropo {
        1.0
    } else {
        1.0 - 0.13318 * mach * mach * temp_ratio
    }
}

pub fn acceleration_factor(
    mach: f64,
    altitude: f64,
    isa_dev: f64,
    above_tropo: bool,
    mode: AccelFactorMode,
) -> f64 {
    let temp_ratio =
        (273.15 + isa_temp(altitude, above_tropo)) / (273.15 + temp(altitude, isa_dev, above_tropo));
    match mode {
        AccelFactorMode::ConstantCas => accel_factor_cas(mach, above_tropo, temp_ratio),
        AccelFactorMode::ConstantMach => accel_factor_mach(mach, above_tropo, temp_ratio),
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Atmosphere {
    pub isa_deviation: f64,
    pub tropopause: f64,
}

impl Atmosphere {
    pub fn new(isa_deviation: f64, tropopause: f64) -> Self {
        Self { isa_deviation, tropopause }
    }

    pub fn above_tropo(&self, altitude: f64) -> bool {
        altitude > self.tropopause
    }

    pub fn predict_sat(&self, altitude: f64) -> f64 {
        isa_temp(altitude, self.above_tropo(altitude)) + self.isa_deviation
    }

    pub fn total_air_temperature(&self, altitude: f64, mach: f64) -> f64 {
        (self.predict_sat(altitude) + 273.15) * (1.0 + 0.2 * mach * mach) - 273.15
    }

    pub fn mach_from_cas(&self, altitude: f64, cas: f64) -> f64 {
        cas_to_mach(cas, delta(altitude, self.above_tropo(altitude)))
    }

    pub fn cas_from_mach(&self, altitude: f64, mach: f64) -> f64 {
        mach_to_cas(mach, delta(altitude, self.above_tropo(altitude)))
    }

    pub fn cas_from_tas(&self, altitude: f64, tas: f64) -> f64 {
        let a = self.above_tropo(altitude);
        tas_to_cas(tas, theta(altitude, self.isa_deviation, a), delta(altitude, a))
    }

    pub fn tas_from_cas(&self, altitude: f64, cas: f64) -> f64 {
        let a = self.above_tropo(altitude);
        cas_to_tas(cas, theta(altitude, self.isa_deviation, a), delta(altitude, a))
    }

    pub fn tas_from_mach(&self, altitude: f64, mach: f64) -> f64 {
        mach_to_tas(mach, theta(altitude, self.isa_deviation, self.above_tropo(altitude)))
    }

    pub fn cas_or_mach_is_mach(&self, cas: f64, mach: f64, altitude: f64) -> bool {
        cas > self.cas_from_mach(altitude, mach)
    }
}

pub const MIN_ELEVATION_FT: f64 = -2000.0;
pub const MAX_ELEVATION_FT: f64 = 14000.0;
pub const RESOLUTION_FT: f64 = 32.0;
pub const MIN_QNH_HPA: f64 = 850.0;
pub const MAX_QNH_HPA: f64 = 1100.0;
pub const MIN_CRUISE_FT: f64 = 1000.0;
pub const MAX_CRUISE_FT: f64 = 45000.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ssm {
    FailureWarning,
    NoComputedData,
    FunctionalTest,
    NormalOperation,
}

impl Ssm {
    pub fn bits(self) -> u32 {
        match self {
            Ssm::FailureWarning => 0,
            Ssm::NoComputedData => 1,
            Ssm::FunctionalTest => 2,
            Ssm::NormalOperation => 3,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FeedWord {
    pub value: f64,
    pub ssm: Ssm,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Destination {
    pub airport_elevation_ft: f64,
    pub runway_tdze_ft: Option<f64>,
    pub runway_threshold_ft: Option<f64>,
}

pub fn quantise(ft: f64) -> f64 {
    ((ft.clamp(MIN_ELEVATION_FT, MAX_ELEVATION_FT) / RESOLUTION_FT).round() * RESOLUTION_FT).clamp(MIN_ELEVATION_FT, MAX_ELEVATION_FT)
}

pub fn landing_elevation_ft(destination: Option<&Destination>) -> Option<f64> {
    let d = destination?;
    let raw = d
        .runway_tdze_ft
        .or(d.runway_threshold_ft)
        .unwrap_or(d.airport_elevation_ft);
    raw.is_finite().then(|| raw.clamp(MIN_ELEVATION_FT, MAX_ELEVATION_FT))
}

pub fn landing_elevation_word(elevation_ft: Option<f64>) -> FeedWord {
    match elevation_ft {
        Some(ft) if ft.is_finite() => FeedWord { value: quantise(ft), ssm: Ssm::NormalOperation },
        _ => FeedWord { value: 0.0, ssm: Ssm::NoComputedData },
    }
}

pub fn destination_qnh_hpa(raw_hpa: f64) -> f64 {
    if raw_hpa.is_finite() && (MIN_QNH_HPA..=MAX_QNH_HPA).contains(&raw_hpa) {
        raw_hpa
    } else {
        0.0
    }
}

pub fn cruise_altitude_ft(raw_ft: f64) -> f64 {
    if raw_ft.is_finite() && (MIN_CRUISE_FT..=MAX_CRUISE_FT).contains(&raw_ft) {
        raw_ft
    } else {
        0.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Fms,
    Latched,
    Departure,
    SeaLevel,
}

impl Source {
    pub fn code(self) -> f64 {
        match self {
            Source::Fms => 0.0,
            Source::Latched => 1.0,
            Source::Departure => 2.0,
            Source::SeaLevel => 3.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Resolved {
    pub elevation_ft: f64,
    pub source: Source,
}

#[derive(Clone, Debug, Default)]
pub struct LandingElevationResolver {
    latched: Option<f64>,
    departure: Option<f64>,
    was_airborne: bool,
}

impl LandingElevationResolver {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update(&mut self, fms_ft: Option<f64>, on_ground: bool, cabin_alt_ft: f64) -> Resolved {
        if on_ground {
            if self.was_airborne {
                self.latched = None;
                self.was_airborne = false;
            }
            self.departure = Some(quantise(cabin_alt_ft));
        } else {
            self.was_airborne = true;
        }
        let valid = fms_ft.filter(|v| v.is_finite() && (MIN_ELEVATION_FT..=MAX_ELEVATION_FT).contains(v));
        if let Some(v) = valid {
            self.latched = Some(v);
            return Resolved { elevation_ft: v, source: Source::Fms };
        }
        if let Some(v) = self.latched {
            return Resolved { elevation_ft: v, source: Source::Latched };
        }
        match self.departure {
            Some(v) => Resolved { elevation_ft: v, source: Source::Departure },
            None => Resolved { elevation_ft: 0.0, source: Source::SeaLevel },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn denver() -> Destination {
        Destination { airport_elevation_ft: 5434.0, runway_tdze_ft: Some(5383.0), runway_threshold_ft: Some(5380.0) }
    }

    #[test]
    fn the_touchdown_zone_wins_over_threshold_and_airport() {
        assert_eq!(landing_elevation_ft(Some(&denver())), Some(5383.0));
        let d = Destination { runway_tdze_ft: None, ..denver() };
        assert_eq!(landing_elevation_ft(Some(&d)), Some(5380.0));
        let d = Destination { runway_threshold_ft: None, runway_tdze_ft: None, ..denver() };
        assert_eq!(landing_elevation_ft(Some(&d)), Some(5434.0));
        assert_eq!(landing_elevation_ft(None), None);
    }

    #[test]
    fn the_word_is_quantised_to_the_arinc_resolution_and_clamped() {
        assert_eq!(quantise(5383.0), 5376.0);
        assert_eq!(quantise(-5000.0), MIN_ELEVATION_FT);
        assert_eq!(quantise(99999.0), MAX_ELEVATION_FT);
        let w = landing_elevation_word(Some(5383.0));
        assert_eq!(w.value, 5376.0);
        assert_eq!(w.ssm, Ssm::NormalOperation);
        assert_eq!(w.ssm.bits(), 3);
    }

    #[test]
    fn no_destination_gives_no_computed_data() {
        let w = landing_elevation_word(None);
        assert_eq!(w.ssm, Ssm::NoComputedData);
        assert_eq!(w.value, 0.0);
        let w = landing_elevation_word(Some(f64::NAN));
        assert_eq!(w.ssm, Ssm::NoComputedData);
    }

    #[test]
    fn implausible_qnh_and_cruise_are_dropped() {
        assert_eq!(destination_qnh_hpa(1013.0), 1013.0);
        assert_eq!(destination_qnh_hpa(29.92), 0.0);
        assert_eq!(destination_qnh_hpa(f64::NAN), 0.0);
        assert_eq!(cruise_altitude_ft(37000.0), 37000.0);
        assert_eq!(cruise_altitude_ft(0.0), 0.0);
        assert_eq!(cruise_altitude_ft(80000.0), 0.0);
    }

    #[test]
    fn without_an_fms_value_the_departure_field_is_used_until_one_arrives() {
        let mut r = LandingElevationResolver::new();
        let a = r.update(None, true, 41.0);
        assert_eq!(a.source, Source::Departure);
        assert_eq!(a.elevation_ft, 32.0);
        let b = r.update(Some(5383.0), false, 3000.0);
        assert_eq!((b.source, b.elevation_ft), (Source::Fms, 5383.0));
    }

    #[test]
    fn a_dropped_fms_value_is_held_in_flight() {
        let mut r = LandingElevationResolver::new();
        r.update(Some(5383.0), true, 0.0);
        r.update(None, false, 8000.0);
        let held = r.update(None, false, 8000.0);
        assert_eq!((held.source, held.elevation_ft), (Source::Latched, 5383.0));
    }

    #[test]
    fn the_latch_is_cleared_after_landing() {
        let mut r = LandingElevationResolver::new();
        r.update(Some(5383.0), false, 8000.0);
        let landed = r.update(None, true, 5400.0);
        assert_eq!(landed.source, Source::Departure);
        assert_eq!(landed.elevation_ft, 5408.0);
    }

    #[test]
    fn out_of_range_fms_values_are_ignored() {
        let mut r = LandingElevationResolver::new();
        let x = r.update(Some(40000.0), false, 0.0);
        assert_eq!(x.source, Source::SeaLevel);
        assert_eq!(x.elevation_ft, 0.0);
    }
}

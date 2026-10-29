#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum AlertLevel {
    #[default]
    None,
    Caution,
    Warning,
}

/// The TAWS look-ahead envelope whose penetrating terrain the ND paints solid (FCOM DSC-34-20-20-10,
/// terrain alerts: yellow areas for the caution criteria, red for the warning criteria). The plugin
/// fills it from crates/gpws taws.rs (`params`, current `TawsOutputs::flta`).
#[derive(Clone, Copy, Debug, Default)]
pub struct AlertArea {
    pub level: AlertLevel,
    pub lat: f64,
    pub lon: f64,
    pub alt_ft: f64,
    pub vs_fpm: f64,
    pub gs_kt: f64,
    pub track_deg: f64,
    pub caution_time_s: f64,
    pub warning_time_s: f64,
    pub half_angle_deg: f64,
    pub start_width_nm: f64,
    pub clearance_ft: f64,
    pub max_lookahead_nm: f64,
}

impl AlertArea {
    /// The level at which a cell `north_nm`/`east_nm` from the aircraft with elevation `elev_ft`
    /// lies inside the envelope, capped by the alert currently active.
    pub fn level_at(&self, north_nm: f64, east_nm: f64, elev_ft: f64) -> AlertLevel {
        if self.level == AlertLevel::None || self.gs_kt <= 0. {
            return AlertLevel::None;
        }
        let (s, c) = self.track_deg.to_radians().sin_cos();
        let along = north_nm * c + east_nm * s;
        if along < 0. {
            return AlertLevel::None;
        }
        let across = (east_nm * c - north_nm * s).abs();
        if across > self.start_width_nm + along * self.half_angle_deg.to_radians().tan() {
            return AlertLevel::None;
        }
        let t = along / self.gs_kt * 3600.;
        let reach = |time_s: f64| (self.gs_kt * time_s / 3600.).min(self.max_lookahead_nm);
        let predicted = self.alt_ft + self.vs_fpm.min(0.) * t / 60. + self.vs_fpm.max(0.) * t / 60. * 0.5;
        if elev_ft + self.clearance_ft < predicted {
            return AlertLevel::None;
        }
        if self.level >= AlertLevel::Warning && along <= reach(self.warning_time_s) {
            AlertLevel::Warning
        } else if along <= reach(self.caution_time_s) {
            AlertLevel::Caution
        } else {
            AlertLevel::None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area(level: AlertLevel) -> AlertArea {
        AlertArea {
            level,
            alt_ft: 5000.,
            gs_kt: 300.,
            track_deg: 0.,
            caution_time_s: 60.,
            warning_time_s: 30.,
            half_angle_deg: 4.,
            start_width_nm: 0.2,
            clearance_ft: 500.,
            max_lookahead_nm: 12.,
            ..Default::default()
        }
    }

    #[test]
    fn nothing_is_painted_without_an_alert() {
        assert_eq!(area(AlertLevel::None).level_at(2., 0., 9000.), AlertLevel::None);
    }

    #[test]
    fn a_caution_paints_only_the_caution_reach() {
        let a = area(AlertLevel::Caution);
        assert_eq!(a.level_at(2., 0., 9000.), AlertLevel::Caution);
        assert_eq!(a.level_at(4.9, 0., 9000.), AlertLevel::Caution);
        assert_eq!(a.level_at(5.1, 0., 9000.), AlertLevel::None);
        assert_eq!(a.level_at(2., 0., 3000.), AlertLevel::None);
        assert_eq!(a.level_at(-2., 0., 9000.), AlertLevel::None);
        assert_eq!(a.level_at(2., 3., 9000.), AlertLevel::None);
    }

    #[test]
    fn a_warning_paints_red_near_and_amber_beyond() {
        let a = area(AlertLevel::Warning);
        assert_eq!(a.level_at(2., 0., 9000.), AlertLevel::Warning);
        assert_eq!(a.level_at(3.5, 0., 9000.), AlertLevel::Caution);
    }
}

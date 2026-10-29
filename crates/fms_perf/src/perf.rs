use crate::constants::*;
use crate::speeds::*;

#[derive(Clone, Copy, Debug)]
pub struct PerfInputs {
    pub phase: FlightPhase,
    pub estimated_landing_weight_kg: Option<f64>,
    pub gross_weight_kg: Option<f64>,
    pub fuel_on_board_tonnes: Option<f64>,
    pub gross_weight_cg: Option<f64>,
    pub approach_wind_direction_deg: Option<f64>,
    pub approach_wind_magnitude_kt: Option<f64>,
    pub destination_runway_magnetic_bearing_deg: Option<f64>,
    pub approach_flaps_three_selected: bool,
    pub v2_kt: Option<f64>,
    pub destination_elevation_ft: f64,
    pub pilot_vapp_kt: Option<f64>,
    pub pressure_altitude_ft: Option<f64>,
    pub computed_airspeed_kt: Option<f64>,
    pub flap_lever: i32,
    pub left_spoiler_1_position: f64,
    pub static_pressure_hpa: Option<f64>,
    pub weight_limits: WeightLimits,
}

impl Default for PerfInputs {
    fn default() -> Self {
        PerfInputs {
            phase: FlightPhase::Preflight,
            estimated_landing_weight_kg: None,
            gross_weight_kg: None,
            fuel_on_board_tonnes: None,
            gross_weight_cg: None,
            approach_wind_direction_deg: None,
            approach_wind_magnitude_kt: None,
            destination_runway_magnetic_bearing_deg: None,
            approach_flaps_three_selected: false,
            v2_kt: None,
            destination_elevation_ft: 0.0,
            pilot_vapp_kt: None,
            pressure_altitude_ft: None,
            computed_airspeed_kt: None,
            flap_lever: 0,
            left_spoiler_1_position: 0.0,
            static_pressure_hpa: None,
            weight_limits: WeightLimits::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PerfSpeeds {
    pub vs1g: f64,
    pub vls: f64,
    pub vmax: f64,
    pub vfe_next: f64,
    pub flap_retraction: Option<f64>,
    pub slat_retraction: Option<f64>,
    pub green_dot: Option<f64>,
    pub approach_vls: Option<f64>,
    pub approach_vref: Option<f64>,
    pub approach_green_dot: Option<f64>,
    pub approach_slat_retraction: Option<f64>,
    pub approach_flap_retraction: Option<f64>,
    pub approach_vapp: Option<f64>,
}

#[derive(Clone, Debug, Default)]
pub struct PerfComputer {
    pub speeds: PerfSpeeds,
}

impl PerfComputer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update_perf_speeds(&mut self, i: &PerfInputs) -> PerfSpeeds {
        let weight_fallback = i.weight_limits.max_zfw + i.fuel_on_board_tonnes.unwrap_or(0.0) * 1000.0;
        let gross_weight = i.gross_weight_kg.unwrap_or(weight_fallback);
        let gross_weight_cg = i.gross_weight_cg.unwrap_or(35.0);
        let ldg_weight = if i.phase >= FlightPhase::Approach || i.estimated_landing_weight_kg.is_none() {
            Some(gross_weight)
        } else {
            i.estimated_landing_weight_kg
        };

        let mut tower_headwind = 0.0;
        if let (Some(dir), Some(mag)) = (i.approach_wind_direction_deg, i.approach_wind_magnitude_kt) {
            if let Some(bearing) = i.destination_runway_magnetic_bearing_deg {
                tower_headwind = SpeedsUtils::headwind(mag, dir, bearing);
            }
        }

        let v2 = i.v2_kt.unwrap_or(150.0);
        let s = &mut self.speeds;

        if let Some(ldg) = ldg_weight {
            let approach = OperatingSpeeds::compute(&OperatingSpeedsInput {
                mass_kg: ldg,
                cg_percent_mac: gross_weight_cg,
                cas_kt: 0.0,
                flap_position: if i.approach_flaps_three_selected { 3 } else { 4 },
                phase: FlightPhase::Approach,
                v2_kt: v2,
                altitude_ft: i.destination_elevation_ft,
                wind_kt: tower_headwind,
                ignore_spoilers: true,
                left_spoiler_1_position: 0.0,
                static_pressure_hpa: i.static_pressure_hpa,
            });
            s.approach_vls = Some(approach.vls.ceil());
            s.approach_vref = Some(approach.vref.ceil());
            s.approach_green_dot = Some(approach.gd.ceil());
            s.approach_slat_retraction = Some(approach.s.ceil());
            s.approach_flap_retraction = Some(approach.f2.ceil());
            s.approach_vapp = Some(i.pilot_vapp_kt.unwrap_or_else(|| approach.vapp.ceil()));
        } else {
            s.approach_vls = None;
            s.approach_vref = None;
            s.approach_green_dot = None;
            s.approach_slat_retraction = None;
            s.approach_flap_retraction = None;
            s.approach_vapp = None;
        }

        if let Some(alt) = i.pressure_altitude_ft {
            let speeds = OperatingSpeeds::compute(&OperatingSpeedsInput {
                mass_kg: gross_weight,
                cg_percent_mac: gross_weight_cg,
                cas_kt: i.computed_airspeed_kt.unwrap_or(0.0),
                flap_position: i.flap_lever,
                phase: i.phase,
                v2_kt: v2,
                altitude_ft: alt,
                wind_kt: tower_headwind,
                ignore_spoilers: false,
                left_spoiler_1_position: i.left_spoiler_1_position,
                static_pressure_hpa: i.static_pressure_hpa,
            });
            s.vs1g = js_round(speeds.vs1g);
            s.vls = js_round(speeds.vls);

            if i.phase == FlightPhase::Preflight {
                s.flap_retraction = Some(speeds.f2.max(VMCL + 5.0).ceil());
            } else if i.flap_lever == 2 {
                s.flap_retraction = Some(speeds.f2.max(VMCL + 15.0).ceil());
            } else if i.flap_lever == 3 {
                s.flap_retraction = Some(speeds.f3.max(VMCL + 10.0).ceil());
            }
            s.slat_retraction = Some(speeds.s.ceil());
            s.green_dot = Some(speeds.gd.ceil());
            s.vmax = js_round(speeds.vmax);
            s.vfe_next = js_round(speeds.vfe_next);
        }
        *s
    }
}

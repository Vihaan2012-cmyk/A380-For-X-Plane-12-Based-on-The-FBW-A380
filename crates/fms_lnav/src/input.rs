use crate::geo::{Coord, TurnDirection};

#[derive(Clone, Debug)]
pub enum LegSpec {
    If,
    Tf { from: Coord },
    Df,
    Cf { course: f64, length: f64 },
    Rf { from: Coord, center: Coord },
    Ci { course: f64 },
    Fa { course: f64, altitude: f64, start_altitude: f64 },
    Fc { course: f64, length: f64 },
    Fm { course: f64 },
    Ca { course: f64, altitude: f64, after_runway: bool },
    Cr { course: f64, navaid: Coord, theta: f64 },
    Vm { heading: f64 },
    Cd { course: f64, distance: f64, dme: Coord },
    Fd { course: f64, distance: f64, dme: Coord },
    Af { centre: Coord, boundary_radial: f64, boundary_radial_coded: f64, termination_radial: f64 },
    Pi { course: f64 },
    Hold { kind: HoldKind, course: f64, length: Option<f64>, length_time: Option<f64>, altitude: Option<f64> },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HoldKind {
    Ha,
    Hf,
    Hm,
}

#[derive(Clone, Debug)]
pub struct LegInput {
    pub ident: String,
    pub waypoint_id: String,
    pub spec: LegSpec,
    pub fix: Coord,
    pub turn: TurnDirection,
    pub overfly: bool,
    pub enroute: bool,
    pub missed_approach: bool,
    pub is_null: bool,
    pub airport_or_runway: bool,
    pub predicted_tas: Option<f64>,
    pub predicted_gs: Option<f64>,
}

impl LegInput {
    pub fn new(ident: &str, spec: LegSpec, fix: Coord) -> Self {
        Self {
            ident: ident.to_string(),
            waypoint_id: ident.to_string(),
            spec,
            fix,
            turn: TurnDirection::Either,
            overfly: false,
            enroute: true,
            missed_approach: false,
            is_null: false,
            airport_or_runway: false,
            predicted_tas: None,
            predicted_gs: None,
        }
    }

    pub fn initial_fix(ident: &str, fix: Coord) -> Self {
        Self::new(ident, LegSpec::If, fix)
    }

    pub fn track_to_fix(ident: &str, from: Coord, to: Coord) -> Self {
        Self::new(ident, LegSpec::Tf { from }, to)
    }
}

#[derive(Clone, Debug)]
pub enum PlanElement {
    Leg(LegInput),
    Discontinuity,
}

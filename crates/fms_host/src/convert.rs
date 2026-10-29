use fms::plan::{Element, FlightPlan, Leg, LegType};
use fms_lnav::{Coord, HoldKind, LegInput, LegSpec, PlanElement, TurnDirection};

fn coord_of(leg: &Leg) -> Option<Coord> {
    leg.termination_waypoint().map(|w| Coord::new(w.location.lat, w.location.long))
}

fn true_course(leg: &Leg) -> f64 {
    let course = leg.definition.course.unwrap_or(0.);
    match leg.definition.mag_var {
        Some(mv) => (course + mv).rem_euclid(360.),
        None => course,
    }
}

fn navaid_coord(leg: &Leg) -> Option<Coord> {
    leg.definition.recommended_navaid.as_ref().map(|n| Coord::new(n.location.lat, n.location.long))
}

fn turn_of(leg: &Leg) -> TurnDirection {
    match leg.definition.turn_direction {
        Some('L') => TurnDirection::Left,
        Some('R') => TurnDirection::Right,
        _ => TurnDirection::Either,
    }
}

/// Builds the LNAV inputs from a flight plan, one element per plan element so the
/// indices match (FBW `GeometryFactory.createFromFlightPlan`). Leg types the LNAV crate
/// has no geometry for become discontinuities.
pub fn plan_elements(plan: &FlightPlan) -> Vec<PlanElement> {
    elements_from(&plan.all_legs(), plan.first_missed_approach_leg_index())
}

/// `plan_elements` for a bare element list (the engine-out departure segment).
pub fn elements_from(all: &[Element], first_missed: usize) -> Vec<PlanElement> {
    let mut out = Vec::with_capacity(all.len());
    for (i, element) in all.iter().enumerate() {
        let Element::Leg(leg) = element else {
            out.push(PlanElement::Discontinuity);
            continue;
        };
        let prev = i.checked_sub(1).and_then(|p| all.get(p)).and_then(|e| e.leg());
        match leg_input(leg, prev) {
            Some(mut input) => {
                input.missed_approach = i >= first_missed;
                out.push(PlanElement::Leg(input));
            }
            None => out.push(PlanElement::Discontinuity),
        }
    }
    out
}

pub fn leg_input(leg: &Leg, prev: Option<&Leg>) -> Option<LegInput> {
    let ident = leg.ident.as_str();
    let fix = coord_of(leg);
    let zero = Coord::new(0., 0.);
    let spec_input = match leg.leg_type {
        LegType::IF => LegInput::new(ident, LegSpec::If, fix?),
        LegType::TF | LegType::RF => {
            let p = prev?;
            if !p.is_xf() && !p.is_hx() {
                return None;
            }
            LegInput::new(ident, LegSpec::Tf { from: coord_of(p)? }, fix?)
        }
        LegType::CF => LegInput::new(ident, LegSpec::Cf { course: true_course(leg), length: leg.definition.length.unwrap_or(0.) }, fix?),
        LegType::DF => LegInput::new(ident, LegSpec::Df, fix?),
        LegType::FC => LegInput::new(ident, LegSpec::Fc { course: true_course(leg), length: leg.definition.length.unwrap_or(0.) }, fix?),
        LegType::FM => LegInput::new(ident, LegSpec::Fm { course: true_course(leg) }, fix?),
        LegType::FA => LegInput::new(ident, LegSpec::Fa { course: true_course(leg), altitude: leg.definition.altitude1.unwrap_or(0.), start_altitude: 0. }, fix?),
        LegType::CA | LegType::VA => {
            let after_runway = prev.is_some_and(|p| p.leg_type == LegType::IF && p.is_runway());
            LegInput::new(ident, LegSpec::Ca { course: true_course(leg), altitude: leg.definition.altitude1.unwrap_or(0.), after_runway }, zero)
        }
        LegType::CI | LegType::VI => LegInput::new(ident, LegSpec::Ci { course: true_course(leg) }, zero),
        LegType::CR | LegType::VR => {
            let navaid = leg.definition.recommended_navaid.as_ref()?;
            let theta = (leg.definition.theta.unwrap_or(0.) + navaid.station_declination.unwrap_or(0.)).rem_euclid(360.);
            LegInput::new(ident, LegSpec::Cr { course: true_course(leg), navaid: Coord::new(navaid.location.lat, navaid.location.long), theta }, zero)
        }
        LegType::HA | LegType::HF | LegType::HM => {
            let kind = match leg.leg_type {
                LegType::HA => HoldKind::Ha,
                LegType::HF => HoldKind::Hf,
                _ => HoldKind::Hm,
            };
            let d = &leg.definition;
            LegInput::new(ident, LegSpec::Hold { kind, course: true_course(leg), length: d.length, length_time: d.length_time, altitude: d.altitude1 }, fix?)
        }
        LegType::VM => LegInput::new(ident, LegSpec::Vm { heading: true_course(leg) }, zero),
        LegType::CD | LegType::VD => {
            let dme = navaid_coord(leg)?;
            LegInput::new(ident, LegSpec::Cd { course: true_course(leg), distance: leg.definition.length.unwrap_or(0.), dme }, zero)
        }
        LegType::FD => {
            let dme = navaid_coord(leg)?;
            LegInput::new(ident, LegSpec::Fd { course: true_course(leg), distance: leg.definition.length.unwrap_or(0.), dme }, fix?)
        }
        LegType::AF => {
            let navaid = leg.definition.recommended_navaid.as_ref()?;
            let decl = navaid.station_declination.unwrap_or(0.);
            let coded = leg.definition.course.unwrap_or(0.);
            let spec = LegSpec::Af {
                centre: navaid_coord(leg)?,
                boundary_radial: (coded + decl).rem_euclid(360.),
                boundary_radial_coded: coded,
                termination_radial: (leg.definition.theta.unwrap_or(0.) + decl).rem_euclid(360.),
            };
            LegInput::new(ident, spec, fix?)
        }
        LegType::PI => LegInput::new(ident, LegSpec::Pi { course: true_course(leg) }, fix?),
    };
    let mut input = spec_input;
    input.waypoint_id = leg.termination_waypoint().map(|w| w.database_id.clone()).unwrap_or_else(|| ident.to_string());
    input.turn = turn_of(leg);
    input.overfly = leg.definition.overfly;
    input.enroute = false;
    input.airport_or_runway = leg.leg_type == LegType::IF && matches!(leg.definition.waypoint_descriptor, Some(fms::plan::DESCRIPTOR_AIRPORT | fms::plan::DESCRIPTOR_RUNWAY));
    Some(input)
}

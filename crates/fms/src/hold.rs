use serde::{Deserialize, Serialize};

use crate::plan::{Element, FlightPlan, FmsError, FmsResult, Leg, LegDefinition, LegType};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum HoldType {
    #[default]
    Computed = 0,
    Database = 1,
    Pilot = 2,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct HoldData {
    pub inbound_magnetic_course: Option<f64>,
    pub turn_direction: Option<char>,
    pub distance: Option<f64>,
    pub time: Option<f64>,
    pub hold_type: HoldType,
}

pub fn parse_hold_from_procedure_leg(definition: &LegDefinition) -> Option<HoldData> {
    match definition.leg_type {
        LegType::HA | LegType::HF | LegType::HM => Some(HoldData {
            inbound_magnetic_course: definition.course,
            turn_direction: definition.turn_direction,
            distance: definition.length,
            time: definition.length_time,
            hold_type: HoldType::Database,
        }),
        _ => None,
    }
}

impl Leg {
    pub fn manual_hold(waypoint: &crate::navdb::Fix, hold: &HoldData, mag_var: Option<f64>) -> Leg {
        let mut leg = Leg::from_enroute_fix(waypoint, None, LegType::HM);
        leg.definition.turn_direction = hold.turn_direction;
        leg.definition.course = hold.inbound_magnetic_course;
        leg.definition.length = hold.distance;
        leg.definition.length_time = hold.time;
        leg.definition.mag_var = mag_var;
        leg
    }

    pub fn hold(&self) -> Option<&HoldData> {
        self.modified_hold.as_ref().or(self.default_hold.as_ref())
    }
}

impl FlightPlan {
    pub fn add_or_edit_manual_hold(&mut self, at_index: usize, desired: &HoldData, modified: Option<HoldData>, default: HoldData) -> FmsResult<usize> {
        let (seg, in_seg) = self.segment_position(at_index).ok_or_else(|| FmsError(format!("[FPM] No leg at index {at_index}")))?;
        let Some(target) = self.segments[seg].legs[in_seg].leg_mut() else {
            return Err(FmsError("[FPM] Target leg of a direct to cannot be a discontinuity".into()));
        };
        if target.is_hx() {
            target.leg_type = LegType::HM;
            target.definition.turn_direction = desired.turn_direction;
            target.definition.course = desired.inbound_magnetic_course;
            target.definition.length = desired.distance;
            target.definition.length_time = desired.time;
            target.modified_hold = modified;
            if target.default_hold.is_none() {
                target.default_hold = Some(default);
            }
            return Ok(at_index);
        }
        let waypoint = target.termination_waypoint().cloned().ok_or_else(|| FmsError("[FPM] Cannot hold at a leg without a waypoint".into()))?;
        let mag_var = waypoint.mag_var.or(waypoint.station_declination).or(Some(0.));
        let mut hold_leg = Leg::manual_hold(&waypoint, desired, mag_var);
        hold_leg.modified_hold = modified;
        hold_leg.default_hold = Some(default);
        let at = (in_seg + 1).min(self.segments[seg].legs.len());
        self.segments[seg].legs.insert(at, Element::new_leg(hold_leg));
        self.increment_version();
        Ok(at_index + 1)
    }

    pub fn revert_hold_to_computed(&mut self, at_index: usize) -> FmsResult<()> {
        match self.leg_at_mut(at_index) {
            Some(l) if l.is_hx() => {
                l.modified_hold = None;
                Ok(())
            }
            _ => Err(FmsError("[FPM] Target leg of a direct to cannot be a discontinuity or a non-HX leg".into())),
        }
    }
}

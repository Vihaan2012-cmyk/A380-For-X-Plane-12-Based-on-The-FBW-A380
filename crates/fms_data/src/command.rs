use serde::{Deserialize, Serialize};

use crate::entry::Entry;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WindPhase {
    Climb,
    Cruise,
    Descent,
}

/// What the FMS service accepts from the MFD pages and the AI F/O. `plan` is the
/// FBW flight plan index (0 active, 1 temporary, 2 uplink, 3.. secondaries).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Command {
    Enter { plan: usize, entry: Entry },
    SetWind { plan: usize, phase: WindPhase, altitude: f64, direction: f64, magnitude: f64, cruise_index: Option<usize> },
    DeleteWind { plan: usize, phase: WindPhase, altitude: f64, cruise_index: Option<usize> },
    DeleteAllWinds { plan: usize, phase: WindPhase },
    SetAlternateWind { plan: usize, direction: Option<f64>, magnitude: Option<f64> },
    /// Origin set or changed: navdata elevation and transition altitude.
    OriginChanged { plan: usize, elevation_ft: Option<f64>, transition_altitude_ft: Option<f64> },
    /// Destination set or changed: navdata elevation and transition level.
    DestinationChanged { plan: usize, elevation_ft: Option<f64>, transition_level: Option<f64> },
    OriginRunwayChanged { plan: usize, runway: String },
    /// After any climb constraint edit, with the plan's lowest climb constraint (ft, infinite when none).
    ReconcileWithClimbConstraints { plan: usize, lowest_climb_constraint_ft: f64 },
    /// Secondary activated or swapped with the active plan.
    SecondaryActivated { zfw_diff_t: Option<f64>, zfw_cg_diff: Option<f64> },
    /// Destination entered on a plan with no cost index: CI 0 and LRC.
    DefaultCostIndexIfMissing { plan: usize },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum FmsMessage {
    CheckToData,
    VToDisagree,
    CheckZfw,
    LrcInUse,
    CrzFlAboveMaxFl,
    NewCrzAlt(f64),
}

impl FmsMessage {
    pub fn text(&self) -> String {
        match self {
            FmsMessage::CheckToData => "CHECK T.O. DATA".into(),
            FmsMessage::VToDisagree => "V1/VR/V2 DISAGREE".into(),
            FmsMessage::CheckZfw => "CHECK ZFW".into(),
            FmsMessage::LrcInUse => "LRC IN USE".into(),
            FmsMessage::CrzFlAboveMaxFl => "CRZ FL ABOVE MAX FL".into(),
            FmsMessage::NewCrzAlt(ft) => format!("NEW CRZ ALT - {ft:.0}"),
        }
    }
}

/// Side effects the service performs outside the performance data.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Effect {
    /// `L:A32NX_AIRLINER_TO_FLEX_TEMP` on the active plan.
    AirlinerFlexTemp(f64),
    /// `acInterface.onUpdateCruiseLevel`.
    CruiseLevelUpdated(f64),
    /// Descent wind entry at altitude 0 from the PERF APPR wind (direction magnetic at the destination).
    SetDescentGroundWind { magnetic_direction: Option<f64>, magnitude: Option<f64> },
    DeleteDescentWindAt(f64),
    DeleteDescentWinds,
    /// A validated wind command for the plan's `fms::winds::WindData`.
    Wind(Command),
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub enum Event {
    /// FBW publishes `flightPlan.setPerformanceData.<key>` for each of these.
    Changed(&'static str),
    Message(FmsMessage),
    /// The message can be removed from the queue.
    MessageCleared(FmsMessage),
    Effect(Effect),
}

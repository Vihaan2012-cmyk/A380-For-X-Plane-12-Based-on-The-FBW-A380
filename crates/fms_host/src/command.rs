use fms::service::Command as PlanCommand;
use fms::uplink::Ofp;
use fms_perf::FlightPhase;

/// Inputs from the MFD pages and the AI F/O.
#[derive(Clone, Debug)]
pub enum HostCommand {
    Plan(PlanCommand),
    Perf(fms_data::Command),
    UplinkSimbrief(Ofp),
    SetFlightNumber { plan: usize, number: String },
    SetPlanCentre { side: usize, fp_index: usize, index: usize, in_alternate: bool },
    SetSecRelatedPageOpen { side: usize, sec_index: Option<usize> },
    SetShownFplnLegs { side: usize, missed: bool, alternate: bool, alternate_missed: bool },
    ClearNdMessage { flag: u32 },
    ForcePhase(FlightPhase),
    PresetSpeed(Option<f64>),
    ResetSystem,
    #[doc(hidden)]
    InjectPanic,
}

#[derive(Clone, Debug)]
pub struct Envelope {
    pub id: u64,
    pub command: HostCommand,
}

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
    /// FBW `shouldBePreflightPhase`: a pilot opened the active INIT, FUEL&LOAD or PERF page;
    /// after the DONE reset this starts the next flight's PREFLIGHT.
    PreflightPageShown,
    /// MFD PERF ACTIVATE APPR, confirmed: FBW `tryGoInApproachPhase`.
    ActivateApproach,
    /// MFD POSITION/NAVAIDS: manual VOR tuning (`index` 0 = VOR1).
    TuneVor { index: usize, entry: NavaidEntry },
    VorCourse { index: usize, course: Option<f64> },
    /// MFD POSITION/NAVAIDS: manual LS tuning, both MMRs.
    TuneLs(NavaidEntry),
    LsCourse(Option<f64>),
    /// Deselect (`true`) or reselect a navaid by ident for FM position updating.
    DeselectNavaid { ident: String, deselect: bool },
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

/// What the pilot typed into a NAVAIDS ident/frequency field.
#[derive(Clone, Debug, PartialEq)]
pub enum NavaidEntry {
    Ident(String),
    Freq(f64),
    Clear,
}

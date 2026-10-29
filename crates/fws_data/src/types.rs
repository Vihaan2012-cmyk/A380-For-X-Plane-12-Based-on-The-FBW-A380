/// Source file of a procedure: one per `AbnormalSensed/*.ts` module of FBW's EcamMessages.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AtaFile {
    Ata212223,
    Ata24,
    Ata26,
    Ata27,
    Ata28,
    Ata2930,
    Ata313233,
    Ata34,
    Ata353642,
    Ata46495256,
    Ata70,
    Ata80Rest,
    SecondaryFailures,
}

impl AtaFile {
    pub const ALL: [AtaFile; 13] = [
        AtaFile::Ata212223,
        AtaFile::Ata24,
        AtaFile::Ata26,
        AtaFile::Ata27,
        AtaFile::Ata28,
        AtaFile::Ata2930,
        AtaFile::Ata313233,
        AtaFile::Ata34,
        AtaFile::Ata353642,
        AtaFile::Ata46495256,
        AtaFile::Ata70,
        AtaFile::Ata80Rest,
        AtaFile::SecondaryFailures,
    ];

    /// FBW's file name without extension, e.g. `ata31-32-33`.
    pub const fn file_name(self) -> &'static str {
        match self {
            AtaFile::Ata212223 => "ata21-22-23",
            AtaFile::Ata24 => "ata24",
            AtaFile::Ata26 => "ata26",
            AtaFile::Ata27 => "ata27",
            AtaFile::Ata28 => "ata28",
            AtaFile::Ata2930 => "ata29-30",
            AtaFile::Ata313233 => "ata31-32-33",
            AtaFile::Ata34 => "ata34",
            AtaFile::Ata353642 => "ata35-36-42",
            AtaFile::Ata46495256 => "ata46-49-52-56",
            AtaFile::Ata70 => "ata70",
            AtaFile::Ata80Rest => "ata80-rest",
            AtaFile::SecondaryFailures => "secondary-failures",
        }
    }
}

/// ATA chapter of a nine digit FBW message id (its first two digits).
pub const fn ata_chapter(id: u32) -> u32 {
    id / 10_000_000
}

/// Which table a procedure comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Id of the form `NN?8NNNNN`, triggered by a sensed fault.
    Sensed,
    /// Listed in `AbnormalNonSensedProceduresOverview`; selected manually from the ABN PROC menu.
    NonSensed,
    /// `EcamAbnormalSecondaryFailures` (ids `9998xxxxx`, titles like `*F/CTL`).
    Secondary,
}

/// Colour of the title, from the leading `\x1b<Nm` code of FBW's title string (2 red, 3 green, 4 amber, 5 cyan, 7 white).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Colour {
    None,
    Red,
    Green,
    Amber,
    Cyan,
    White,
}

/// `AbnormalProcedure.recommendation`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Recommendation {
    LandAsap,
    LandAnsa,
}

/// `ChecklistLineStyle` of FBW's EcamMessages.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    Standard,
    Cyan,
    Green,
    GreenTable,
    Amber,
    White,
    Red,
    Headline,
    SubHeadline,
    CenteredSubHeadline,
    SeparationLine,
    Empty,
    ChecklistItem,
    CompletedChecklist,
    CompletedDeferredProcedure,
    DeferredProcedure,
    OmissionDots,
    LandAsap,
    LandAnsa,
    ChecklistCondition,
    ChecklistItemInactive,
}

/// Kind of a procedure line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemKind {
    /// `ChecklistAction`: has `labelNotCompleted` (also timed actions).
    Action,
    /// `ChecklistCondition`: an "IF ..." line (`condition: true`, also timed conditions).
    Condition,
    /// A name-only line (`ChecklistSpecialItem`, headlines, notes).
    Plain,
}

/// One line of a procedure. Fields map one to one to FBW's `AbstractChecklistItem` and subtypes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Item {
    /// `name`: text at the start of the line, no leading dot, no leading "IF" for conditions.
    pub name: &'static str,
    pub kind: ItemKind,
    /// `sensed`: checked automatically; unsensed items get a checkbox the crew ticks.
    pub sensed: bool,
    /// `level`: indentation, 0 is the first level (omitted in FBW means 0).
    pub level: u8,
    /// `style`: omitted in FBW means `Style::Standard`.
    pub style: Style,
    /// `labelNotCompleted`: text at the end of the line while open ("" when absent).
    pub label: &'static str,
    /// `labelCompleted`: text after the name once completed, when it differs from `label`.
    pub label_completed: Option<&'static str>,
    /// `colonIfCompleted`: omitted in FBW means true.
    pub colon_if_completed: bool,
    /// `time`: seconds appended to the name (`AFTER 30 S`), for timed actions and conditions.
    pub time: Option<u32>,
    /// `appendTimeIfElapsed` of timed conditions.
    pub append_time_if_elapsed: bool,
}

/// An entry of `EcamAbnormalSensedProcedures` (sensed, non-sensed and secondary failures).
#[derive(Clone, Copy, Debug)]
pub struct Procedure {
    /// The nine digit message id (FBW's dictionary key).
    pub id: u32,
    /// Source file.
    pub ata: AtaFile,
    pub kind: Kind,
    /// `AbnormalProcedure.sensed`.
    pub sensed: bool,
    /// `title` exactly as FBW writes it, escape codes included (`\x1b<4m` colour, `\x1b4m` underline, `\x1bm` reset).
    pub title: &'static str,
    /// `title` with the escape codes removed.
    pub plain_title: &'static str,
    /// The underlined system name of the title (`HYD`, `ENG 1`), "" if none.
    pub system: &'static str,
    pub colour: Colour,
    pub recommendation: Option<Recommendation>,
    pub items: &'static [Item],
}

impl Procedure {
    pub const fn chapter(&self) -> u32 {
        ata_chapter(self.id)
    }
}

/// `DeferredProcedureType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeferredType {
    AllPhases,
    AtTopOfDescent,
    ForApproach,
    ForLanding,
}

/// An entry of `EcamDeferredProcedures`.
#[derive(Clone, Copy, Debug)]
pub struct Deferred {
    pub id: u32,
    pub ata: AtaFile,
    /// `fromAbnormalProcs`: the abnormal procedures that trigger this deferred procedure.
    pub from_abnormal: &'static [u32],
    pub title: Option<&'static str>,
    pub kind: DeferredType,
    pub items: &'static [Item],
}

/// Submenu of the non-sensed ABN PROC list (`AbnormalNonSensedCategory`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Category {
    None,
    Eng,
    FCtl,
    LG,
    Nav,
    Fuel,
    Miscellaneous,
}

/// An entry of `AbnormalNonSensedProceduresOverview`, in FBW's order.
#[derive(Clone, Copy, Debug)]
pub struct NonSensed {
    pub id: u32,
    pub category: Category,
}

/// Alert level, from the `failure` field of `EwdAbnormalItem`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    /// -1: not an alert; the entry only feeds logic and shows its procedure (e.g. audio function lost).
    Hidden,
    /// 0: no master light (deferred procedures).
    None,
    /// 1: amber alert line without master caution or aural.
    Advisory,
    /// 2: amber, master caution and single chime.
    Caution,
    /// 3: red, master warning and continuous repetitive chime.
    Warning,
}

impl Level {
    pub const fn from_failure(failure: i8) -> Level {
        match failure {
            i8::MIN..=-1 => Level::Hidden,
            0 => Level::None,
            1 => Level::Advisory,
            2 => Level::Caution,
            _ => Level::Warning,
        }
    }
}

/// Set of FWC flight phases (1 to 12, `FwcFlightPhase`) as a bit mask.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Phases(pub u16);

impl Phases {
    pub const NONE: Phases = Phases(0);

    pub const fn from_list(list: &[u8]) -> Phases {
        let mut mask = 0u16;
        let mut i = 0;
        while i < list.len() {
            mask |= 1 << list[i];
            i += 1;
        }
        Phases(mask)
    }

    pub const fn contains(self, phase: u8) -> bool {
        phase < 16 && self.0 & (1 << phase) != 0
    }

    pub fn list(self) -> Vec<u8> {
        (0..16u8).filter(|p| self.contains(*p)).collect()
    }
}

/// `SdPages` of FBW's EcamSystemPages: the system display page an alert calls.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SdPage {
    None,
    Eng,
    Apu,
    Bleed,
    Cond,
    Press,
    Door,
    ElecAc,
    ElecDc,
    Fuel,
    Wheel,
    Hyd,
    Fctl,
    Cb,
    Crz,
    Status,
    Video,
}

impl SdPage {
    /// FBW's numeric value (`None` is -1).
    pub const fn value(self) -> i32 {
        match self {
            SdPage::None => -1,
            SdPage::Eng => 0,
            SdPage::Apu => 1,
            SdPage::Bleed => 2,
            SdPage::Cond => 3,
            SdPage::Press => 4,
            SdPage::Door => 5,
            SdPage::ElecAc => 6,
            SdPage::ElecDc => 7,
            SdPage::Fuel => 8,
            SdPage::Wheel => 9,
            SdPage::Hyd => 10,
            SdPage::Fctl => 11,
            SdPage::Cb => 12,
            SdPage::Crz => 13,
            SdPage::Status => 14,
            SdPage::Video => 15,
        }
    }
}

/// `FwcAuralWarning`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuralKind {
    None,
    SingleChime,
    Crc,
    CavalryCharge,
}

/// The `auralWarning` field of an alert.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Aural {
    /// Field omitted: follows `simVarIsActive`, single chime for level 2, CRC for level 3.
    Default,
    /// `Subject.create(FwcAuralWarning.X)`.
    Constant(AuralKind),
    /// `this.fws.<name>.map((on) => on ? FwcAuralWarning.<on> : FwcAuralWarning.<off>)`.
    Signal {
        name: &'static str,
        on: AuralKind,
        off: AuralKind,
    },
    /// Any other expression: the `this.fws.*` names it reads and its source text.
    Expr {
        refs: &'static [&'static str],
        src: &'static str,
    },
}

/// Which FBW dictionary an alert is defined in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlertSource {
    /// `FwsAbnormalSensed.ewdAbnormalSensed`
    Sensed,
    /// `FwsAbnormalNonSensed.ewdAbnormalNonSensed`
    NonSensed,
    /// `ewdDeferredProcs` of either class.
    Deferred,
}

/// Shape of a `whichItems*` function: what can be said about it without running it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemsFn {
    /// Not defined.
    Absent,
    /// Not a plain array literal.
    Dynamic,
    /// An array literal of `len` elements (`None` if spread elements make it unknown); `literal` holds the values when every element is `true` or `false`.
    Array {
        len: Option<u32>,
        literal: Option<&'static [bool]>,
    },
}

/// Message ids returned by one of the STS page callbacks (`inopSysAllPhases`, `limitationsApprLdg`, ...).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IdList {
    pub ids: &'static [u32],
    /// The callback chooses among its ids at run time (conditionals or the `checked` argument).
    pub conditional: bool,
}

impl IdList {
    pub const NONE: IdList = IdList { ids: &[], conditional: false };
}

/// Static metadata of one `EwdAbnormalItem`. The condition logic itself is named, not translated: `active_refs` are the
/// `this.fws.<name>` fields `simVarIsActive` reads.
#[derive(Clone, Copy, Debug)]
pub struct Alert {
    /// Key of the entry: the id of the procedure it activates.
    pub id: u32,
    /// The `// ...` comment FBW writes at the top of the entry (the alert title).
    pub comment: &'static str,
    pub source: AlertSource,
    /// `flightPhaseInhib`: a new alert is not raised in these flight phases.
    pub phases_inhibited: Phases,
    /// `failure`.
    pub level: Level,
    /// `sysPage`.
    pub sd_page: SdPage,
    /// `auralWarning`.
    pub aural: Aural,
    /// `simVarIsActive` as source text.
    pub active_expr: &'static str,
    /// Names of the `this.fws.<name>` fields it reads.
    pub active_refs: &'static [&'static str],
    /// `L:`/`A:` variables read directly in the expression.
    pub active_vars: &'static [&'static str],
    /// `notActiveWhenItemActive`: this alert is suppressed while any of these alerts is active.
    pub suppressed_by: &'static [u32],
    /// `monitorConfirmTime` in seconds; `None` uses the FWS default (0.6 s for abnormal alerts).
    pub monitor_confirm_time: Option<f32>,
    /// `cancel`: omitted in FBW means true.
    pub cancel: bool,
    /// `nonProcedureKey`: the entry does not display a procedure on the EWD.
    pub non_procedure_key: bool,
    /// `whichItemsToShow`, `whichItemsChecked`, `whichItemsActive`, `whichItemsTimer`.
    pub items_to_show: ItemsFn,
    pub items_checked: ItemsFn,
    pub items_active: ItemsFn,
    pub items_timer: ItemsFn,
    pub inop_sys_all_phases: IdList,
    pub inop_sys_appr_ldg: IdList,
    pub redund_loss: IdList,
    pub limitations_all_phases: IdList,
    pub limitations_appr_ldg: IdList,
    pub limitations_pfd: IdList,
    pub info: IdList,
}

/// Number of entries FBW's own tables hold per source file, counted straight from the TypeScript.
#[derive(Clone, Copy, Debug)]
pub struct ExpectedCount {
    pub ata: AtaFile,
    pub procedures: usize,
    pub deferred: usize,
}

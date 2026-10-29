use crate::Phase;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StationKind {
    Delivery,
    Ground,
    Tower,
    Departure,
    Approach,
    Centre,
    Atis,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Station {
    pub kind: StationKind,
    pub khz: u32,
}

/// VHF COM range with 25 kHz or 8.33 kHz channel names (every 5 kHz).
pub fn valid_vhf_khz(khz: u32) -> bool {
    (118_000..=136_990).contains(&khz) && khz.is_multiple_of(5)
}

/// The station the PM preselects in standby during a phase, ahead of the handover.
pub fn next_kind(phase: Phase, current: Option<StationKind>) -> Option<StationKind> {
    use StationKind::*;
    match phase {
        Phase::Preflight => Some(if current == Some(Delivery) { Ground } else { Delivery }),
        Phase::TaxiOut => Some(Tower),
        Phase::TakeoffRoll | Phase::GoAround => Some(Departure),
        Phase::Climb => Some(if current == Some(Departure) { Centre } else { Departure }),
        Phase::Cruise => Some(Centre),
        Phase::Descent => Some(Approach),
        Phase::Approach => Some(Tower),
        Phase::Rollout | Phase::TaxiIn => Some(Ground),
        Phase::Parked => None,
    }
}

pub fn find(list: &[Station], kind: StationKind) -> Option<Station> {
    list.iter().copied().find(|s| s.kind == kind && valid_vhf_khz(s.khz))
}

pub fn kind_of(list: &[Station], khz: u32) -> Option<StationKind> {
    list.iter().find(|s| s.khz == khz).map(|s| s.kind)
}

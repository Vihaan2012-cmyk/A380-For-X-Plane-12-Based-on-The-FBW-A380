use crate::command::{Event, FmsMessage};
use crate::data::PerformanceData;
use crate::entry::FlightPhase;

/// FBW's FMGC-side take-off data state: the runway the V speeds were entered for,
/// the cleared speeds kept for confirmation, and the edge-triggered speed checks.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TakeoffChecks {
    pub v_speeds_for_runway: Option<String>,
    pub v1_to_be_confirmed: Option<f64>,
    pub vr_to_be_confirmed: Option<f64>,
    pub v2_to_be_confirmed: Option<f64>,
    v_speed_disagree: bool,
}

/// ARINC discrete word 3 bits FBW sets from the checks.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TakeoffSpeedBits {
    pub v_speed_disagree: bool,
    pub to_speeds_too_low: bool,
    pub to_speeds_not_inserted: bool,
}

impl TakeoffChecks {
    /// FBW `FmsFlightPlanPage.onNewDataChecks`: a new origin runway clears V1/VR/V2 of the active plan.
    pub fn on_origin_runway(&mut self, pd: &mut PerformanceData, runway: &str) -> Vec<Event> {
        match self.v_speeds_for_runway.as_deref() {
            None => {
                self.v_speeds_for_runway = Some(runway.to_string());
                Vec::new()
            }
            Some(r) if r == runway => Vec::new(),
            Some(_) => {
                self.v_speeds_for_runway = Some(runway.to_string());
                self.v1_to_be_confirmed = pd.v1.take();
                self.vr_to_be_confirmed = pd.vr.take();
                self.v2_to_be_confirmed = pd.v2.take();
                vec![Event::Changed("v1"), Event::Changed("vr"), Event::Changed("v2"), Event::Message(FmsMessage::CheckToData)]
            }
        }
    }

    /// FBW clears that CHECK T.O. DATA once VR is entered again.
    pub fn check_to_data_cleared(pd: &PerformanceData) -> bool {
        pd.vr.is_some()
    }

    /// FBW `FmcAircraftInterface.toSpeedsChecks`. FBW has the too-low check disabled (always false).
    pub fn speed_checks(&mut self, pd: &PerformanceData) -> (TakeoffSpeedBits, Vec<Event>) {
        let disagree = !pd.v_speeds_in_order();
        let mut ev = Vec::new();
        if disagree != self.v_speed_disagree {
            self.v_speed_disagree = disagree;
            ev.push(if disagree { Event::Message(FmsMessage::VToDisagree) } else { Event::MessageCleared(FmsMessage::VToDisagree) });
        }
        (TakeoffSpeedBits { v_speed_disagree: disagree, to_speeds_too_low: false, to_speeds_not_inserted: !pd.v_speeds_inserted() }, ev)
    }

    /// FBW `FlightManagementComputer.onSecondaryActivated`.
    pub fn on_secondary_activated(phase: FlightPhase, pd: &PerformanceData, zfw_diff_t: Option<f64>, zfw_cg_diff: Option<f64>) -> Vec<Event> {
        let mut ev = Vec::new();
        if matches!(phase, FlightPhase::Preflight | FlightPhase::Done) {
            ev.push(Event::Message(FmsMessage::CheckToData));
            let flex = match pd.flex_takeoff_temperature {
                Some(t) if t == 0.0 => 0.1,
                Some(t) => t,
                None => 0.0,
            };
            ev.push(Event::Effect(crate::command::Effect::AirlinerFlexTemp(flex)));
        }
        if zfw_diff_t.is_some_and(|d| d > 5.0) && zfw_cg_diff.is_some_and(|d| d > 0.5) {
            ev.push(Event::Message(FmsMessage::CheckZfw));
        }
        ev
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runway_change_clears_v_speeds_and_keeps_them_for_confirmation() {
        let mut c = TakeoffChecks::default();
        let mut pd = PerformanceData { v1: Some(140.0), vr: Some(150.0), v2: Some(160.0), ..Default::default() };
        assert!(c.on_origin_runway(&mut pd, "RW27L").is_empty());
        assert!(c.on_origin_runway(&mut pd, "RW27L").is_empty());
        assert_eq!(pd.v1, Some(140.0));
        let ev = c.on_origin_runway(&mut pd, "RW27R");
        assert_eq!((pd.v1, pd.vr, pd.v2), (None, None, None));
        assert_eq!(c.vr_to_be_confirmed, Some(150.0));
        assert!(ev.contains(&Event::Message(FmsMessage::CheckToData)));
        assert!(!TakeoffChecks::check_to_data_cleared(&pd));
    }

    #[test]
    fn v_speed_disagree_is_edge_triggered() {
        let mut c = TakeoffChecks::default();
        let mut pd = PerformanceData { v1: Some(150.0), vr: Some(145.0), ..Default::default() };
        let (bits, ev) = c.speed_checks(&pd);
        assert!(bits.v_speed_disagree && bits.to_speeds_not_inserted);
        assert_eq!(ev, vec![Event::Message(FmsMessage::VToDisagree)]);
        assert!(c.speed_checks(&pd).1.is_empty());
        pd.vr = Some(152.0);
        pd.v2 = Some(160.0);
        let (bits, ev) = c.speed_checks(&pd);
        assert!(!bits.v_speed_disagree && !bits.to_speeds_not_inserted);
        assert_eq!(ev, vec![Event::MessageCleared(FmsMessage::VToDisagree)]);
    }

    #[test]
    fn secondary_activation_messages() {
        let pd = PerformanceData { flex_takeoff_temperature: Some(0.0), ..Default::default() };
        let ev = TakeoffChecks::on_secondary_activated(FlightPhase::Preflight, &pd, Some(6.0), Some(0.6));
        assert!(ev.contains(&Event::Message(FmsMessage::CheckToData)));
        assert!(ev.contains(&Event::Message(FmsMessage::CheckZfw)));
        assert!(ev.contains(&Event::Effect(crate::command::Effect::AirlinerFlexTemp(0.1))));
        assert!(TakeoffChecks::on_secondary_activated(FlightPhase::Cruise, &pd, Some(1.0), None).is_empty());
    }
}

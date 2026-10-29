use serde::{Deserialize, Serialize};

use crate::command::{Effect, Event, FmsMessage};
use crate::data::*;
use crate::limits::*;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum FlightPhase {
    #[default]
    Preflight = 0,
    Takeoff = 1,
    Climb = 2,
    Cruise = 3,
    Descent = 4,
    Approach = 5,
    GoAround = 6,
    Done = 7,
}

#[derive(Clone, Debug, PartialEq)]
pub enum EntryError {
    OutOfRange { min: f64, max: f64 },
    NotAllowed,
    FormatError,
}

#[derive(Clone, Debug, Default)]
pub struct EntryContext {
    pub phase: FlightPhase,
    /// The plan is the active one, the temporary one, or a secondary copied from the active one.
    pub active_or_copy: bool,
    /// The plan is the active one (or temporary).
    pub active: bool,
    pub engines_started: bool,
    pub has_destination: bool,
    pub destination_elevation_ft: Option<f64>,
    pub fcu_altitude_ft: Option<f64>,
    pub max_zfw_kg: Option<f64>,
    pub rec_max_flight_level: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Entry {
    ZeroFuelWeightKg(Option<f64>),
    ZeroFuelWeightCg(Option<f64>),
    BlockFuelKg(Option<f64>),
    TaxiFuelKg(Option<f64>),
    RouteReserveFuelKg(Option<f64>),
    RouteReservePercent(Option<f64>),
    AlternateFuelKg(Option<f64>),
    FinalHoldingFuelKg(Option<f64>),
    FinalHoldingTimeMin(Option<f64>),
    MinDestFobKg(Option<f64>),
    PaxNumber(Option<f64>),
    TripWind(Option<f64>),

    CostIndex(Option<f64>),
    CostIndexMode(CostIndexMode),
    CruiseFlightLevel(f64),
    CruiseTemperature(Option<f64>),
    Tropopause(Option<f64>),

    TakeoffShift(Option<f64>),
    TakeoffFlaps(Option<u8>),
    ThsFor(Option<f64>),
    TakeoffPowerSetting(TakeoffPowerSetting),
    FlexTemperature(Option<f64>),
    TakeoffDerated(TakeoffDerated),
    TakeoffPacks(TakeoffPacks),
    TakeoffAntiIce(TakeoffAntiIce),
    V1(Option<f64>),
    Vr(Option<f64>),
    V2(Option<f64>),
    ThrustReductionAltitude(Option<f64>),
    AccelerationAltitude(Option<f64>),
    EngineOutAccelerationAltitude(Option<f64>),
    MissedThrustReductionAltitude(Option<f64>),
    MissedAccelerationAltitude(Option<f64>),
    MissedEngineOutAccelerationAltitude(Option<f64>),
    TransitionAltitude(Option<f64>),
    TransitionLevel(Option<f64>),
    NoiseEnabled(bool),
    NoiseN1(Option<f64>),
    NoiseSpeed(Option<f64>),
    NoiseEndAltitude(Option<f64>),

    PreselectedClimbSpeed(Option<f64>),
    PreselectedCruiseSpeed(Option<f64>),
    ManagedDescentSpeed(Option<f64>),
    ManagedDescentMach(Option<f64>),
    ClimbDerated(ClimbDerated),
    DescentCabinRate(Option<f64>),

    ApproachQnh(Option<f64>),
    ApproachTemperature(Option<f64>),
    ApproachWindDirection(Option<f64>),
    ApproachWindMagnitude(Option<f64>),
    ApproachBaroMinimum(Option<f64>),
    /// 0 is NO DH.
    ApproachRadioMinimum(Option<f64>),
    ApproachFlapsThree(bool),
    Vapp(Option<f64>),
}

fn range(v: Option<f64>, (min, max): (f64, f64)) -> Result<Option<f64>, EntryError> {
    match v {
        None => Ok(None),
        Some(x) if x.is_nan() => Err(EntryError::FormatError),
        Some(x) if x < min || x > max => Err(EntryError::OutOfRange { min, max }),
        Some(x) => Ok(Some(x)),
    }
}

fn kg_to_t(v: Option<f64>) -> Option<f64> {
    v.map(|x| x / 1000.0)
}

fn not_cleared(v: Option<f64>) -> Result<f64, EntryError> {
    v.ok_or(EntryError::NotAllowed)
}

fn qnh(v: Option<f64>) -> Result<Option<f64>, EntryError> {
    match v {
        None => Ok(None),
        Some(x) if (QNH_INHG.0..=QNH_INHG.1).contains(&x) || (QNH_HPA.0..=QNH_HPA.1).contains(&x) => Ok(Some(x)),
        Some(x) if x.is_nan() => Err(EntryError::FormatError),
        Some(_) => Err(EntryError::OutOfRange { min: QNH_HPA.0, max: QNH_HPA.1 }),
    }
}

impl Entry {
    fn on_takeoff_page(&self) -> bool {
        use Entry::*;
        matches!(
            self,
            TakeoffShift(_)
                | TakeoffFlaps(_)
                | ThsFor(_)
                | TakeoffPowerSetting(_)
                | FlexTemperature(_)
                | TakeoffDerated(_)
                | TakeoffPacks(_)
                | TakeoffAntiIce(_)
                | V1(_)
                | Vr(_)
                | V2(_)
                | ThrustReductionAltitude(_)
                | AccelerationAltitude(_)
                | EngineOutAccelerationAltitude(_)
                | NoiseEnabled(_)
                | NoiseN1(_)
                | NoiseSpeed(_)
                | NoiseEndAltitude(_)
        )
    }
}

/// Validates an entry the way FBW's A380X MFD fields do and writes it into `pd`.
pub fn apply(pd: &mut PerformanceData, ctx: &EntryContext, entry: Entry) -> Result<Vec<Event>, EntryError> {
    use Entry::*;
    if entry.on_takeoff_page() && ctx.active_or_copy && ctx.phase >= FlightPhase::Takeoff {
        return Err(EntryError::NotAllowed);
    }
    let mut ev = Vec::new();
    let mut changed = |key: &'static str| ev.push(Event::Changed(key));
    match entry {
        ZeroFuelWeightKg(v) => {
            let max = ctx.max_zfw_kg.unwrap_or(MAX_ZFW_KG);
            let v = range(Some(not_cleared(v)?), (MIN_ZFW_KG, max))?;
            pd.zero_fuel_weight = kg_to_t(v);
            changed("zeroFuelWeight");
        }
        ZeroFuelWeightCg(v) => {
            pd.zero_fuel_weight_center_of_gravity = range(Some(not_cleared(v)?), (MIN_ZFW_CG, MAX_ZFW_CG))?;
            changed("zeroFuelWeightCenterOfGravity");
        }
        BlockFuelKg(v) => {
            pd.block_fuel = kg_to_t(range(v, (0.0, MAX_BLOCK_FUEL_KG))?);
            changed("blockFuel");
        }
        TaxiFuelKg(v) => {
            pd.pilot_taxi_fuel = kg_to_t(range(v, (0.0, MAX_TAXI_FUEL_KG))?);
            changed("pilotTaxiFuel");
        }
        RouteReserveFuelKg(v) => {
            pd.pilot_route_reserve_fuel = kg_to_t(range(v, (0.0, MAX_RTE_RSV_FUEL_KG))?);
            pd.pilot_route_reserve_fuel_percentage = None;
            changed("pilotRouteReserveFuel");
            changed("pilotRouteReserveFuelPercentage");
        }
        RouteReservePercent(v) => {
            pd.pilot_route_reserve_fuel_percentage = range(v, (0.0, MAX_RTE_RSV_FUEL_PERC))?;
            pd.pilot_route_reserve_fuel = None;
            changed("pilotRouteReserveFuelPercentage");
            changed("pilotRouteReserveFuel");
        }
        AlternateFuelKg(v) => {
            pd.pilot_alternate_fuel = kg_to_t(range(v, (0.0, MAX_ALTN_FUEL_KG))?);
            changed("pilotAlternateFuel");
        }
        FinalHoldingFuelKg(v) => {
            pd.pilot_final_holding_fuel = kg_to_t(range(v, (0.0, MAX_FINAL_FUEL_KG))?);
            pd.pilot_final_holding_time = None;
            changed("pilotFinalHoldingFuel");
            changed("pilotFinalHoldingTime");
        }
        FinalHoldingTimeMin(v) => {
            pd.pilot_final_holding_time = range(v, (0.0, MAX_FINAL_HOLDING_TIME_MIN))?;
            pd.pilot_final_holding_fuel = None;
            changed("pilotFinalHoldingTime");
            changed("pilotFinalHoldingFuel");
        }
        MinDestFobKg(v) => {
            pd.pilot_minimum_destination_fuel_on_board = kg_to_t(range(v, (f64::NEG_INFINITY, MAX_MIN_DEST_FUEL_KG))?);
            changed("pilotMinimumDestinationFuelOnBoard");
        }
        PaxNumber(v) => {
            pd.pax_number = range(v, (0.0, MAX_PAX))?;
            changed("paxNumber");
        }
        TripWind(v) => {
            pd.pilot_trip_wind = range(v, TRIP_WIND)?;
            changed("pilotTripWind");
        }
        CostIndex(v) => {
            let lrc = pd.cost_index_mode == Some(crate::data::CostIndexMode::Lrc);
            if lrc || (ctx.active_or_copy && ctx.phase >= FlightPhase::Descent) {
                return Err(EntryError::NotAllowed);
            }
            pd.cost_index = range(v, COST_INDEX)?;
            changed("costIndex");
        }
        CostIndexMode(m) => {
            pd.cost_index_mode = Some(m);
            changed("costIndexMode");
        }
        CruiseFlightLevel(fl) => return set_cruise_level(pd, ctx, fl),
        CruiseTemperature(v) => {
            pd.cruise_temperature_pilot_entry = range(v, CRUISE_TEMPERATURE)?;
            changed("cruiseTemperaturePilotEntry");
        }
        Tropopause(v) => {
            pd.pilot_tropopause = range(v, TROPOPAUSE)?;
            changed("pilotTropopause");
        }
        TakeoffShift(v) => {
            pd.takeoff_shift = range(v, (0.0, f64::INFINITY))?;
            changed("takeoffShift");
        }
        TakeoffFlaps(f) => {
            if f.is_some_and(|f| !(1..=3).contains(&f)) {
                return Err(EntryError::OutOfRange { min: 1.0, max: 3.0 });
            }
            pd.takeoff_flaps = f;
            changed("takeoffFlaps");
        }
        ThsFor(v) => {
            if let Some(x) = range(v, THS_FOR)?.filter(|x| *x != 0.0) {
                pd.takeoff_ths_for = Some(x);
                changed("takeoffThsFor");
            }
        }
        TakeoffPowerSetting(s) => {
            if s == crate::data::TakeoffPowerSetting::Derated {
                return Err(EntryError::NotAllowed);
            }
            pd.takeoff_power_setting = s;
            changed("takeoffPowerSetting");
            ev.push(Event::Effect(Effect::AirlinerFlexTemp(pd.airliner_flex_lvar())));
        }
        FlexTemperature(v) => {
            pd.flex_takeoff_temperature = range(v, FLEX_TEMPERATURE)?;
            changed("flexTakeoffTemperature");
            ev.push(Event::Effect(Effect::AirlinerFlexTemp(match v {
                Some(t) if t == 0.0 => 0.1,
                Some(t) => t,
                None => 0.0,
            })));
        }
        TakeoffDerated(d) => {
            pd.takeoff_derated_setting = d;
            changed("takeoffDeratedSetting");
        }
        TakeoffPacks(p) => {
            pd.takeoff_packs = Some(p);
            changed("takeoffPacks");
            if ctx.engines_started {
                ev.push(Event::Message(FmsMessage::CheckToData));
            }
        }
        TakeoffAntiIce(a) => {
            pd.takeoff_anti_ice = Some(a);
            changed("takeoffAntiIce");
            if ctx.engines_started {
                ev.push(Event::Message(FmsMessage::CheckToData));
            }
        }
        V1(v) => {
            pd.v1 = range(v, SPEED_KNOTS)?;
            changed("v1");
        }
        Vr(v) => {
            pd.vr = range(v, SPEED_KNOTS)?;
            changed("vr");
        }
        V2(v) => {
            pd.v2 = range(v, SPEED_KNOTS)?;
            changed("v2");
        }
        ThrustReductionAltitude(v) => {
            pd.pilot_thrust_reduction_altitude = range(v, ALTITUDE)?;
            changed("pilotThrustReductionAltitude");
        }
        AccelerationAltitude(v) => {
            pd.pilot_acceleration_altitude = range(v, ALTITUDE)?;
            changed("pilotAccelerationAltitude");
        }
        EngineOutAccelerationAltitude(v) => {
            pd.pilot_engine_out_acceleration_altitude = range(v, ALTITUDE)?;
            changed("pilotEngineOutAccelerationAltitude");
        }
        MissedThrustReductionAltitude(v) => {
            pd.pilot_missed_thrust_reduction_altitude = range(v, ALTITUDE)?;
            changed("pilotMissedThrustReductionAltitude");
        }
        MissedAccelerationAltitude(v) => {
            pd.pilot_missed_acceleration_altitude = range(v, ALTITUDE)?;
            changed("pilotMissedAccelerationAltitude");
        }
        MissedEngineOutAccelerationAltitude(v) => {
            pd.pilot_missed_engine_out_acceleration_altitude = range(v, ALTITUDE)?;
            changed("pilotMissedEngineOutAccelerationAltitude");
        }
        TransitionAltitude(v) => {
            pd.pilot_transition_altitude = range(v, ALTITUDE)?;
            changed("pilotTransitionAltitude");
        }
        TransitionLevel(v) => {
            pd.pilot_transition_level = range(v, (0.0, MAX_CERTIFIED_ALT / 100.0))?;
            changed("pilotTransitionLevel");
        }
        NoiseEnabled(b) => {
            pd.noise_enabled = b;
            changed("noiseEnabled");
        }
        NoiseN1(v) => {
            pd.noise_n1 = range(v, NOISE_N1)?;
            changed("noiseN1");
        }
        NoiseSpeed(v) => {
            pd.noise_speed = range(v, SPEED_KNOTS)?;
            changed("noiseSpeed");
        }
        NoiseEndAltitude(v) => {
            pd.noise_end_altitude = range(v, ALTITUDE)?;
            changed("noiseEndAltitude");
        }
        PreselectedClimbSpeed(v) => {
            pd.preselected_climb_speed = range(v, SPEED_KNOTS)?;
            changed("preselectedClimbSpeed");
        }
        PreselectedCruiseSpeed(v) => {
            pd.preselected_cruise_speed = match v {
                Some(m) if m < 1.0 => range(Some(m), SPEED_MACH)?,
                v => range(v, SPEED_KNOTS)?,
            };
            changed("preselectedCruiseSpeed");
        }
        ManagedDescentSpeed(v) => {
            pd.pilot_managed_descent_speed = range(v, SPEED_KNOTS)?;
            changed("pilotManagedDescentSpeed");
        }
        ManagedDescentMach(v) => {
            pd.pilot_managed_descent_mach = range(v, SPEED_MACH)?;
            changed("pilotManagedDescentMach");
        }
        ClimbDerated(d) => {
            pd.climb_derated = Some(d);
            changed("climbDerated");
        }
        DescentCabinRate(v) => {
            pd.descent_cabin_rate = range(v, DESCENT_CABIN_RATE)?;
            changed("descentCabinRate");
        }
        ApproachQnh(v) => {
            if let Some(q) = qnh(v)?.filter(|q| *q != 0.0) {
                pd.approach_qnh = Some(q);
                changed("approachQnh");
            }
        }
        ApproachTemperature(v) => {
            pd.approach_temperature = range(v, APPROACH_TEMPERATURE)?;
            changed("approachTemperature");
        }
        ApproachWindDirection(v) => return set_approach_wind(pd, ctx, range(v, WIND_DIRECTION)?, None, v.is_none()),
        ApproachWindMagnitude(v) => return set_approach_wind(pd, ctx, None, range(v, WIND_SPEED)?, v.is_none()),
        ApproachBaroMinimum(v) => {
            pd.approach_baro_minimum = range(v, BARO_MINIMUM)?;
            changed("approachBaroMinimum");
        }
        ApproachRadioMinimum(v) => {
            pd.approach_radio_minimum = match v {
                Some(x) if x == RADIO_MINIMUM_NO_DH => Some(RADIO_MINIMUM_NO_DH),
                v => range(v, RADIO_MINIMUM)?,
            };
            changed("approachRadioMinimum");
        }
        ApproachFlapsThree(b) => {
            pd.approach_flaps_three_selected = b;
            changed("approachFlapsThreeSelected");
        }
        Vapp(v) => {
            pd.pilot_vapp = range(v, SPEED_KNOTS)?;
            changed("pilotVapp");
        }
    }
    Ok(ev)
}

/// FBW `FlightManagementComputer.trySetCruiseFl`.
pub fn set_cruise_level(pd: &mut PerformanceData, ctx: &EntryContext, fl: f64) -> Result<Vec<Event>, EntryError> {
    if !fl.is_finite() {
        return Err(EntryError::FormatError);
    }
    if fl <= 0.0 || fl > MAX_CERTIFIED_ALT / 100.0 {
        return Err(EntryError::OutOfRange { min: 0.0, max: MAX_CERTIFIED_ALT / 100.0 });
    }
    let mut level = fl;
    let mut new_crz_alt = false;
    if ctx.active && ctx.phase >= FlightPhase::Climb && ctx.phase < FlightPhase::Done {
        let fcu_fl = (ctx.fcu_altitude_ft.unwrap_or(0.0).max(0.0) / 100.0).floor();
        if fl < fcu_fl {
            level = fcu_fl;
            new_crz_alt = matches!(ctx.phase, FlightPhase::Cruise | FlightPhase::Descent | FlightPhase::Approach);
        }
    }
    pd.cruise_flight_level = Some(level);
    let mut ev = vec![Event::Changed("cruiseFlightLevel")];
    if ctx.rec_max_flight_level.is_some_and(|m| level > m) {
        ev.push(Event::Message(FmsMessage::CrzFlAboveMaxFl));
    }
    if ctx.active {
        ev.push(Event::Effect(Effect::CruiseLevelUpdated(level)));
        if new_crz_alt {
            ev.push(Event::Message(FmsMessage::NewCrzAlt(level * 100.0)));
        }
    }
    Ok(ev)
}

/// FBW `FlightPlanService.setApproachWind` / `deleteApproachWind`.
fn set_approach_wind(pd: &mut PerformanceData, ctx: &EntryContext, direction: Option<f64>, magnitude: Option<f64>, delete: bool) -> Result<Vec<Event>, EntryError> {
    if !ctx.has_destination {
        return Err(EntryError::NotAllowed);
    }
    if delete {
        pd.approach_wind_direction = None;
        pd.approach_wind_magnitude = None;
        pd.is_approach_wind_pilot_entered = false;
        let mut ev = vec![Event::Changed("approachWindDirection"), Event::Changed("approachWindMagnitude"), Event::Changed("isApproachWindPilotEntered")];
        if let Some(alt) = ctx.destination_elevation_ft {
            ev.push(Event::Effect(Effect::DeleteDescentWindAt(alt)));
        }
        return Ok(ev);
    }
    let dir = direction.map(|d| d % 360.0).or(pd.approach_wind_direction);
    let mag = magnitude.or(pd.approach_wind_magnitude);
    pd.approach_wind_direction = dir;
    pd.approach_wind_magnitude = mag;
    pd.is_approach_wind_pilot_entered = true;
    Ok(vec![
        Event::Changed("approachWindDirection"),
        Event::Changed("approachWindMagnitude"),
        Event::Changed("isApproachWindPilotEntered"),
        Event::Effect(Effect::SetDescentGroundWind { magnetic_direction: dir, magnitude: mag }),
    ])
}

pub fn validate_wind(direction: f64, magnitude: f64, altitude: f64, ground_altitude: Option<f64>) -> Result<f64, EntryError> {
    range(Some(direction), WIND_DIRECTION)?;
    range(Some(magnitude), WIND_SPEED)?;
    if altitude.is_nan() {
        return Err(EntryError::FormatError);
    }
    if altitude > MAX_CERTIFIED_ALT || altitude < 0.0 || (ground_altitude.is_none() && altitude <= WIND_GROUND_BAND_FT) {
        return Err(EntryError::OutOfRange { min: 0.0, max: MAX_CERTIFIED_ALT });
    }
    Ok(altitude.max(ground_altitude.unwrap_or(0.0)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> EntryContext {
        EntryContext { active_or_copy: true, active: true, has_destination: true, destination_elevation_ft: Some(13.0), ..Default::default() }
    }

    #[test]
    fn weights_are_entered_in_kg_and_stored_in_tonnes() {
        let mut pd = PerformanceData::default();
        apply(&mut pd, &ctx(), Entry::ZeroFuelWeightKg(Some(330_000.0))).unwrap();
        apply(&mut pd, &ctx(), Entry::BlockFuelKg(Some(95_000.0))).unwrap();
        assert_eq!(pd.zero_fuel_weight, Some(330.0));
        assert_eq!(pd.block_fuel, Some(95.0));
        assert_eq!(apply(&mut pd, &ctx(), Entry::ZeroFuelWeightKg(Some(269_999.0))), Err(EntryError::OutOfRange { min: MIN_ZFW_KG, max: MAX_ZFW_KG }));
        assert_eq!(apply(&mut pd, &ctx(), Entry::ZeroFuelWeightKg(None)), Err(EntryError::NotAllowed));
        assert_eq!(apply(&mut pd, &ctx(), Entry::ZeroFuelWeightCg(Some(48.1))), Err(EntryError::OutOfRange { min: 23.0, max: 48.0 }));
        let wv = EntryContext { max_zfw_kg: Some(366_000.0), ..ctx() };
        assert!(apply(&mut pd, &wv, Entry::ZeroFuelWeightKg(Some(370_000.0))).is_err());
    }

    #[test]
    fn takeoff_page_locks_from_takeoff_phase_on_active_plan_only() {
        let mut pd = PerformanceData::default();
        let to = EntryContext { phase: FlightPhase::Takeoff, ..ctx() };
        assert_eq!(apply(&mut pd, &to, Entry::V1(Some(140.0))), Err(EntryError::NotAllowed));
        let sec = EntryContext { phase: FlightPhase::Takeoff, active_or_copy: false, active: false, ..ctx() };
        assert!(apply(&mut pd, &sec, Entry::V1(Some(140.0))).is_ok());
        assert_eq!(apply(&mut pd, &ctx(), Entry::V1(Some(89.0))), Err(EntryError::OutOfRange { min: 90.0, max: VMO }));
    }

    #[test]
    fn flex_lvar_and_derated_selection() {
        let mut pd = PerformanceData::default();
        apply(&mut pd, &ctx(), Entry::FlexTemperature(Some(0.0))).unwrap();
        let ev = apply(&mut pd, &ctx(), Entry::TakeoffPowerSetting(TakeoffPowerSetting::Flex)).unwrap();
        assert!(ev.contains(&Event::Effect(Effect::AirlinerFlexTemp(0.1))));
        let ev = apply(&mut pd, &ctx(), Entry::TakeoffPowerSetting(TakeoffPowerSetting::Toga)).unwrap();
        assert!(ev.contains(&Event::Effect(Effect::AirlinerFlexTemp(0.0))));
        assert_eq!(apply(&mut pd, &ctx(), Entry::TakeoffPowerSetting(TakeoffPowerSetting::Derated)), Err(EntryError::NotAllowed));
        assert!(apply(&mut pd, &ctx(), Entry::FlexTemperature(Some(100.0))).is_err());
    }

    #[test]
    fn packs_after_engine_start_ask_to_check_to_data() {
        let mut pd = PerformanceData::default();
        assert!(apply(&mut pd, &ctx(), Entry::TakeoffPacks(TakeoffPacks::OffApu)).unwrap().iter().all(|e| !matches!(e, Event::Message(_))));
        let started = EntryContext { engines_started: true, ..ctx() };
        assert!(apply(&mut pd, &started, Entry::TakeoffAntiIce(TakeoffAntiIce::EngOnly)).unwrap().contains(&Event::Message(FmsMessage::CheckToData)));
    }

    #[test]
    fn cost_index_range_and_lrc_lock() {
        let mut pd = PerformanceData::default();
        apply(&mut pd, &ctx(), Entry::CostIndex(Some(999.0))).unwrap();
        assert!(apply(&mut pd, &ctx(), Entry::CostIndex(Some(1000.0))).is_err());
        apply(&mut pd, &ctx(), Entry::CostIndexMode(CostIndexMode::Lrc)).unwrap();
        assert_eq!(apply(&mut pd, &ctx(), Entry::CostIndex(Some(30.0))), Err(EntryError::NotAllowed));
        pd.cost_index_mode = Some(CostIndexMode::Econ);
        let des = EntryContext { phase: FlightPhase::Descent, ..ctx() };
        assert_eq!(apply(&mut pd, &des, Entry::CostIndex(Some(30.0))), Err(EntryError::NotAllowed));
    }

    #[test]
    fn cruise_level_is_held_at_fcu_altitude_once_airborne() {
        let mut pd = PerformanceData::default();
        assert!(apply(&mut pd, &ctx(), Entry::CruiseFlightLevel(0.0)).is_err());
        assert!(apply(&mut pd, &ctx(), Entry::CruiseFlightLevel(432.0)).is_err());
        apply(&mut pd, &ctx(), Entry::CruiseFlightLevel(360.0)).unwrap();
        assert_eq!(pd.cruise_flight_level, Some(360.0));
        let crz = EntryContext { phase: FlightPhase::Cruise, fcu_altitude_ft: Some(38_000.0), ..ctx() };
        let ev = apply(&mut pd, &crz, Entry::CruiseFlightLevel(340.0)).unwrap();
        assert_eq!(pd.cruise_flight_level, Some(380.0));
        assert!(ev.contains(&Event::Message(FmsMessage::NewCrzAlt(38_000.0))));
        let clb = EntryContext { phase: FlightPhase::Climb, fcu_altitude_ft: Some(38_000.0), rec_max_flight_level: Some(390.0), ..ctx() };
        let ev = apply(&mut pd, &clb, Entry::CruiseFlightLevel(400.0)).unwrap();
        assert!(ev.contains(&Event::Message(FmsMessage::CrzFlAboveMaxFl)));
        assert!(!ev.iter().any(|e| matches!(e, Event::Message(FmsMessage::NewCrzAlt(_)))));
    }

    #[test]
    fn approach_entries() {
        let mut pd = PerformanceData::default();
        apply(&mut pd, &ctx(), Entry::ApproachQnh(Some(1013.0))).unwrap();
        apply(&mut pd, &ctx(), Entry::ApproachQnh(Some(29.92))).unwrap();
        assert_eq!(pd.approach_qnh, Some(29.92));
        assert!(apply(&mut pd, &ctx(), Entry::ApproachQnh(Some(500.0))).is_err());
        apply(&mut pd, &ctx(), Entry::ApproachQnh(None)).unwrap();
        assert_eq!(pd.approach_qnh, Some(29.92));
        apply(&mut pd, &ctx(), Entry::ApproachWindDirection(Some(220.0))).unwrap();
        let ev = apply(&mut pd, &ctx(), Entry::ApproachWindMagnitude(Some(12.0))).unwrap();
        assert_eq!((pd.approach_wind_direction, pd.approach_wind_magnitude), (Some(220.0), Some(12.0)));
        assert!(ev.contains(&Event::Effect(Effect::SetDescentGroundWind { magnetic_direction: Some(220.0), magnitude: Some(12.0) })));
        let ev = apply(&mut pd, &ctx(), Entry::ApproachWindMagnitude(None)).unwrap();
        assert!(!pd.is_approach_wind_pilot_entered && pd.approach_wind_direction.is_none());
        assert!(ev.contains(&Event::Effect(Effect::DeleteDescentWindAt(13.0))));
        let no_dest = EntryContext { has_destination: false, ..ctx() };
        assert_eq!(apply(&mut pd, &no_dest, Entry::ApproachWindDirection(Some(10.0))), Err(EntryError::NotAllowed));
        apply(&mut pd, &ctx(), Entry::ApproachRadioMinimum(Some(0.0))).unwrap();
        assert_eq!(pd.approach_radio_minimum, Some(0.0));
        apply(&mut pd, &ctx(), Entry::ApproachBaroMinimum(Some(0.0))).unwrap();
        assert!(apply(&mut pd, &ctx(), Entry::ApproachTemperature(Some(-100.0))).is_err());
    }

    #[test]
    fn wind_altitude_rules() {
        assert_eq!(validate_wind(270.0, 50.0, 300.0, None), Err(EntryError::OutOfRange { min: 0.0, max: MAX_CERTIFIED_ALT }));
        assert_eq!(validate_wind(270.0, 50.0, 300.0, Some(500.0)), Ok(500.0));
        assert!(validate_wind(360.0, 50.0, 10_000.0, None).is_err());
        assert!(validate_wind(0.0, 251.0, 10_000.0, None).is_err());
    }

    #[test]
    fn ths_for_ignores_clear_and_zero() {
        let mut pd = PerformanceData::default();
        apply(&mut pd, &ctx(), Entry::ThsFor(Some(30.5))).unwrap();
        apply(&mut pd, &ctx(), Entry::ThsFor(None)).unwrap();
        apply(&mut pd, &ctx(), Entry::ThsFor(Some(0.0))).unwrap();
        assert_eq!(pd.takeoff_ths_for, Some(30.5));
    }
}

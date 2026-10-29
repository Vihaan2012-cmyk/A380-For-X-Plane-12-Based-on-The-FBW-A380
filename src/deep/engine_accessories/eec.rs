//! EEC (electronic engine control, the Trent's FADEC): dual, fully
//! independent channels (A/B), each with its own complete set of N1/N2/N3/
//! TGT/P30 sensors -- not a single sensor set duplicated in software, a
//! second physically separate measurement chain for every parameter the
//! control law needs, exactly like the fuel flow transmitter's own two
//! pick-off coils (`fuel::flow_transmitter`) this module treats as a sixth
//! voted parameter rather than re-modelling. Normal operation runs on one
//! channel (swapped at each engine start) with the other in hot standby; a channel
//! declared faulted hands control to the healthy one, and only losing
//! *both* leaves no valid channel at all -- the same redundant-restraint
//! shape `thrust_reverser.rs`'s three locks use, here across two channels
//! instead of three.
//!
//! "Sensor drift/disagree voting" is exactly what it says: each channel's
//! sensors independently lag and can carry a bias/frozen fault (reusing the
//! same first-order-lag-plus-bias-plus-freeze shape as every other sensor
//! in this directory, `fuel::flow_transmitter::PickoffFaults`); this module
//! then compares the two channels' readings for each parameter and flags a
//! disagreement once it exceeds a generic per-parameter threshold, which is
//! the actual physical basis for a real EEC's channel-disagree monitor.
//!
//! No Trent-900 EEC sensor figures are public. Every threshold/bias/time
//! constant below is **GENERIC**, chosen to be a plausible order of
//! magnitude for a digital engine sensor chain, not measured.

/// The five parameters each channel senses independently (fuel flow is
/// handled separately by `fuel::flow_transmitter` and folded into this
/// module's voting only as precomputed channel readings, see `step`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Param {
    N1,
    N2,
    N3,
    Tgt,
    P30,
}
pub const PARAMS: [Param; 5] = [Param::N1, Param::N2, Param::N3, Param::Tgt, Param::P30];

fn index(p: Param) -> usize {
    match p {
        Param::N1 => 0,
        Param::N2 => 1,
        Param::N3 => 2,
        Param::Tgt => 3,
        Param::P30 => 4,
    }
}

/// Full-scale bias magnitude at `bias == 1.0`, and the disagree threshold,
/// in each parameter's own natural unit (percent for N1/N2/N3, K for TGT,
/// Pa for P30). **GENERIC**.
const FULL_SCALE_BIAS: [f64; 5] = [5.0, 5.0, 5.0, 50.0, 3.0e5];
const DISAGREE_THRESHOLD: [f64; 5] = [2.0, 2.0, 2.0, 15.0, 1.5e5];
/// Sensor electronics lag, s (**GENERIC**, a fast digital sensor chain).
const SENSOR_TIME_CONSTANT_S: f64 = 0.1;
/// Fuel-flow channel disagree threshold, kg/s (mirrors
/// `fuel::flow_transmitter`'s own bias scale; kept here too so this
/// module's voting logic is self-contained for all six parameters).
pub const FUEL_FLOW_DISAGREE_THRESHOLD_KG_S: f64 = 1.0;

/// HP rotor speed, percent, above which the gearbox-driven FADEC alternator
/// powers the EEC, and below which the aircraft network takes it back
/// through the EIPM (FCOM DSC-70-20-10).
pub const ALTERNATOR_ON_HP_PCT: f64 = 12.5;
pub const ALTERNATOR_OFF_HP_PCT: f64 = 10.5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PowerSource {
    Alternator,
    Aircraft,
    Unpowered,
}

#[derive(Clone, Copy, Debug)]
pub struct EecPowerInputs {
    pub hp_rotor_pct: f64,
    /// The aircraft network supply through this engine's EIPM is live.
    pub aircraft_supply: bool,
    /// Starter engaged this frame; the controlling channel swaps on each new start.
    pub start_requested: bool,
    /// 0 healthy .. 1 dead.
    pub alternator_fault: f64,
}

/// One sensor channel's fault state, 0 (healthy) .. 1 (fully failed).
#[derive(Clone, Copy, Debug, Default)]
pub struct SensorFaults {
    pub bias: f64,
    pub frozen: f64,
}

/// Faults the whole EEC can carry.
#[derive(Clone, Copy, Debug)]
pub struct EecFaults {
    /// Whole-channel electronics failure, 0 healthy .. 1 dead: a dead
    /// channel's sensors are ignored entirely by the voting/selection logic
    /// below, not merely biased.
    pub channel_a_fault: f64,
    pub channel_b_fault: f64,
    pub sensor_a: [SensorFaults; 5],
    pub sensor_b: [SensorFaults; 5],
}

impl Default for EecFaults {
    fn default() -> Self {
        Self { channel_a_fault: 0.0, channel_b_fault: 0.0, sensor_a: [SensorFaults::default(); 5], sensor_b: [SensorFaults::default(); 5] }
    }
}

#[derive(Clone, Copy, Debug)]
struct Channel {
    held: [f64; 5],
}

impl Channel {
    fn new() -> Self {
        Self { held: [0.0; 5] }
    }

    fn step(&mut self, true_values: [f64; 5], faults: &[SensorFaults; 5], dt_s: f64) -> [f64; 5] {
        let dt = dt_s.max(0.0);
        let k = 1.0 - (-dt / SENSOR_TIME_CONSTANT_S).exp();
        for i in 0..5 {
            let biased_target = true_values[i] + faults[i].bias.clamp(-1.0, 1.0) * FULL_SCALE_BIAS[i];
            let lagged = self.held[i] + (biased_target - self.held[i]) * k;
            let frozen = faults[i].frozen.clamp(0.0, 1.0);
            self.held[i] = self.held[i] * frozen + lagged * (1.0 - frozen);
        }
        self.held
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActiveChannel {
    A,
    B,
    /// Both channels declared faulted: no valid EEC control channel
    /// remains (a real aircraft would revert to a degraded/alternate mode
    /// this directory does not model further).
    None,
}

#[derive(Clone, Copy, Debug)]
pub struct EecState {
    /// The five sensed parameters as the active channel (or, with no valid
    /// channel, the average of both -- a degraded last resort, not a
    /// control-quality signal) reports them.
    pub selected: [f64; 5],
    pub disagree: [bool; 5],
    pub active: ActiveChannel,
    pub fuel_flow_disagree: bool,
    /// Each channel's own serviceability, as the EEC's built-in test
    /// equipment declares it -- the same discrete verdict [`Eec::select`]
    /// already derives to decide which channel flies the engine, reported
    /// rather than thrown away.
    ///
    /// Without this, losing the *standby* channel is invisible: selection
    /// keeps the healthy channel in control and nothing downstream can
    /// tell a dual-channel EEC from one running single-channel, which is
    /// precisely the condition an EEC CHANNEL FAULT annunciates.
    pub channel_a_serviceable: bool,
    pub channel_b_serviceable: bool,
    pub power: PowerSource,
}

impl EecState {
    pub fn powered(&self) -> bool {
        self.power != PowerSource::Unpowered
    }

    pub fn value(&self, p: Param) -> f64 {
        self.selected[index(p)]
    }
    pub fn disagrees(&self, p: Param) -> bool {
        self.disagree[index(p)]
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Eec {
    channel_a: Channel,
    channel_b: Channel,
    preferred_b: bool,
    on_alternator: bool,
    start_was_requested: bool,
    power: PowerSource,
}

impl Eec {
    pub fn new() -> Self {
        Self { channel_a: Channel::new(), channel_b: Channel::new(), preferred_b: false, on_alternator: false, start_was_requested: false, power: PowerSource::Aircraft }
    }

    pub fn saved(&self) -> [u8; 4] {
        let power = match self.power {
            PowerSource::Alternator => 0,
            PowerSource::Aircraft => 1,
            PowerSource::Unpowered => 2,
        };
        [self.preferred_b as u8, self.on_alternator as u8, self.start_was_requested as u8, power]
    }

    pub fn restore(&mut self, [preferred_b, on_alternator, start_was_requested, power]: [u8; 4]) {
        self.preferred_b = preferred_b != 0;
        self.on_alternator = on_alternator != 0;
        self.start_was_requested = start_was_requested != 0;
        self.power = match power {
            0 => PowerSource::Alternator,
            2 => PowerSource::Unpowered,
            _ => PowerSource::Aircraft,
        };
    }

    pub fn preferred_channel(&self) -> ActiveChannel {
        if self.preferred_b {
            ActiveChannel::B
        } else {
            ActiveChannel::A
        }
    }

    /// Updates the supply and the start-to-start channel swap; call before [`Eec::step`].
    pub fn step_power(&mut self, inputs: &EecPowerInputs) -> PowerSource {
        if inputs.start_requested && !self.start_was_requested {
            self.preferred_b = !self.preferred_b;
        }
        self.start_was_requested = inputs.start_requested;

        let alternator_ok = inputs.alternator_fault.clamp(0.0, 1.0) < 0.5;
        if inputs.hp_rotor_pct >= ALTERNATOR_ON_HP_PCT {
            self.on_alternator = true;
        } else if inputs.hp_rotor_pct < ALTERNATOR_OFF_HP_PCT {
            self.on_alternator = false;
        }
        self.power = if self.on_alternator && alternator_ok {
            PowerSource::Alternator
        } else if inputs.aircraft_supply {
            PowerSource::Aircraft
        } else {
            PowerSource::Unpowered
        };
        self.power
    }

    /// Whether a channel's electronics are serviceable at all: below half
    /// a fully-dead channel it still controls, above it the EEC declares
    /// it lost and ignores it entirely rather than merely distrusting it.
    fn channel_serviceable(fault: f64) -> bool {
        fault.clamp(0.0, 1.0) < 0.5
    }

    fn select(a_fault: f64, b_fault: f64, preferred_b: bool, powered: bool) -> ActiveChannel {
        let a_ok = powered && Self::channel_serviceable(a_fault);
        let b_ok = powered && Self::channel_serviceable(b_fault);
        match (preferred_b, a_ok, b_ok) {
            (false, true, _) | (true, true, false) => ActiveChannel::A,
            (_, _, true) => ActiveChannel::B,
            _ => ActiveChannel::None,
        }
    }

    /// One step. `true_values` are the five real physical values (N1%,
    /// N2%, N3%, TGT K, P30 Pa) both channels' sensors are trying to
    /// measure; `fuel_flow_a_kg_s`/`fuel_flow_b_kg_s` are the already-
    /// computed dual pick-off readings from `fuel::flow_transmitter`.
    pub fn step(&mut self, true_values: [f64; 5], fuel_flow_a_kg_s: f64, fuel_flow_b_kg_s: f64, faults: &EecFaults, dt_s: f64) -> EecState {
        let reads_a = self.channel_a.step(true_values, &faults.sensor_a, dt_s);
        let reads_b = self.channel_b.step(true_values, &faults.sensor_b, dt_s);
        let active = Self::select(faults.channel_a_fault, faults.channel_b_fault, self.preferred_b, self.power != PowerSource::Unpowered);

        let selected = match active {
            ActiveChannel::A => reads_a,
            ActiveChannel::B => reads_b,
            ActiveChannel::None => {
                let mut mid = [0.0; 5];
                for i in 0..5 {
                    mid[i] = 0.5 * (reads_a[i] + reads_b[i]);
                }
                mid
            }
        };
        let mut disagree = [false; 5];
        for i in 0..5 {
            disagree[i] = (reads_a[i] - reads_b[i]).abs() > DISAGREE_THRESHOLD[i];
        }

        EecState {
            selected,
            disagree,
            active,
            fuel_flow_disagree: (fuel_flow_a_kg_s - fuel_flow_b_kg_s).abs() > FUEL_FLOW_DISAGREE_THRESHOLD_KG_S,
            channel_a_serviceable: Self::channel_serviceable(faults.channel_a_fault),
            channel_b_serviceable: Self::channel_serviceable(faults.channel_b_fault),
            power: self.power,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn truth() -> [f64; 5] {
        [19.0, 55.0, 62.0, 650.0, 1.2e6]
    }

    fn settle(eec: &mut Eec, faults: &EecFaults, seconds: f64) -> EecState {
        let dt = 0.02;
        let mut out = eec.step(truth(), 3.4, 3.4, faults, 0.0);
        for _ in 0..(seconds / dt) as usize {
            out = eec.step(truth(), 3.4, 3.4, faults, dt);
        }
        out
    }

    #[test]
    fn a_healthy_eec_settles_on_the_true_values_from_channel_a_with_no_disagree() {
        let mut eec = Eec::new();
        let s = settle(&mut eec, &EecFaults::default(), 3.0);
        assert_eq!(s.active, ActiveChannel::A);
        for (i, t) in truth().iter().enumerate() {
            assert!((s.selected[i] - t).abs() < 0.5, "param {i}: {} vs {}", s.selected[i], t);
        }
        assert!(s.disagree.iter().all(|d| !d));
        assert!(!s.fuel_flow_disagree);
    }

    #[test]
    fn a_dead_channel_a_hands_control_to_channel_b() {
        let mut eec = Eec::new();
        let faults = EecFaults { channel_a_fault: 1.0, ..EecFaults::default() };
        let s = settle(&mut eec, &faults, 3.0);
        assert_eq!(s.active, ActiveChannel::B);
        assert!((s.selected[0] - truth()[0]).abs() < 0.5, "B is healthy and should still report the truth");
    }

    #[test]
    fn both_channels_dead_gives_no_valid_channel() {
        let mut eec = Eec::new();
        let faults = EecFaults { channel_a_fault: 1.0, channel_b_fault: 1.0, ..EecFaults::default() };
        let s = settle(&mut eec, &faults, 3.0);
        assert_eq!(s.active, ActiveChannel::None);
    }

    #[test]
    fn a_biased_channel_a_sensor_disagrees_with_a_healthy_channel_b() {
        let mut eec = Eec::new();
        let mut faults = EecFaults::default();
        faults.sensor_a[super::index(Param::N1)] = SensorFaults { bias: 1.0, ..Default::default() };
        let s = settle(&mut eec, &faults, 3.0);
        assert!(s.disagrees(Param::N1));
        assert!(!s.disagrees(Param::N2), "only the biased parameter should disagree");
    }

    #[test]
    fn control_still_uses_the_biased_channel_a_reading_while_it_is_not_declared_dead() {
        // A biased sensor alone does not fail the whole channel in this
        // model (that is a separate, coarser `channel_a_fault`); the
        // disagree flag exists for annunciation/maintenance, control
        // authority stays with the nominally-active channel.
        let mut eec = Eec::new();
        let mut faults = EecFaults::default();
        faults.sensor_a[super::index(Param::N1)] = SensorFaults { bias: 1.0, ..Default::default() };
        let s = settle(&mut eec, &faults, 3.0);
        assert!((s.value(Param::N1) - (truth()[0] + FULL_SCALE_BIAS[0])).abs() < 0.5);
    }

    #[test]
    fn a_frozen_sensor_stops_updating() {
        let mut eec = Eec::new();
        eec.step(truth(), 3.4, 3.4, &EecFaults::default(), 1.0);
        let mut faults = EecFaults::default();
        faults.sensor_a[super::index(Param::Tgt)] = SensorFaults { frozen: 1.0, ..Default::default() };
        let before = eec.step(truth(), 3.4, 3.4, &faults, 0.02).selected[3];
        let hotter = [truth()[0], truth()[1], truth()[2], truth()[3] + 100.0, truth()[4]];
        let after = eec.step(hotter, 3.4, 3.4, &faults, 5.0).selected[3];
        assert!((after - before).abs() < 1e-6);
    }

    #[test]
    fn losing_the_standby_channel_is_reported_even_though_the_other_keeps_control() {
        // A dual-channel EEC running on one channel is an annunciated
        // condition, and which channel died does not change that. With
        // only `active` to go on, a dead channel B was invisible.
        let mut eec = Eec::new();
        let healthy = settle(&mut eec, &EecFaults::default(), 1.0);
        assert!(healthy.channel_a_serviceable && healthy.channel_b_serviceable);

        let mut eec = Eec::new();
        let b_dead = settle(&mut eec, &EecFaults { channel_b_fault: 1.0, ..EecFaults::default() }, 1.0);
        assert_eq!(b_dead.active, ActiveChannel::A, "A still flies the engine");
        assert!(b_dead.channel_a_serviceable);
        assert!(!b_dead.channel_b_serviceable, "but the EEC knows it has lost its standby channel");

        let mut eec = Eec::new();
        let a_dead = settle(&mut eec, &EecFaults { channel_a_fault: 1.0, ..EecFaults::default() }, 1.0);
        assert!(!a_dead.channel_a_serviceable);
        assert!(a_dead.channel_b_serviceable);
    }

    #[test]
    fn a_fuel_flow_disagree_is_flagged_independently_of_the_other_five_parameters() {
        let mut eec = Eec::new();
        let dt = 0.02;
        let mut out = EecState { selected: [0.0; 5], disagree: [false; 5], active: ActiveChannel::A, fuel_flow_disagree: false, channel_a_serviceable: true, channel_b_serviceable: true, power: PowerSource::Aircraft };
        for _ in 0..150 {
            out = eec.step(truth(), 3.4, 1.0, &EecFaults::default(), dt);
        }
        assert!(out.fuel_flow_disagree);
        assert!(out.disagree.iter().all(|d| !d));
    }

    fn power(hp: f64, aircraft: bool, start: bool, alt_fault: f64) -> EecPowerInputs {
        EecPowerInputs { hp_rotor_pct: hp, aircraft_supply: aircraft, start_requested: start, alternator_fault: alt_fault }
    }

    #[test]
    fn the_alternator_takes_over_above_12_5_pct_and_hands_back_below_10_5_pct() {
        let mut eec = Eec::new();
        assert_eq!(eec.step_power(&power(0.0, true, false, 0.0)), PowerSource::Aircraft);
        assert_eq!(eec.step_power(&power(12.0, true, false, 0.0)), PowerSource::Aircraft);
        assert_eq!(eec.step_power(&power(12.5, true, false, 0.0)), PowerSource::Alternator);
        assert_eq!(eec.step_power(&power(11.0, true, false, 0.0)), PowerSource::Alternator, "hysteresis band keeps the alternator");
        assert_eq!(eec.step_power(&power(10.4, true, false, 0.0)), PowerSource::Aircraft);
    }

    #[test]
    fn a_running_engine_keeps_its_fadec_with_the_aircraft_network_dead() {
        let mut eec = Eec::new();
        assert_eq!(eec.step_power(&power(60.0, false, false, 0.0)), PowerSource::Alternator);
        let s = eec.step(truth(), 3.4, 3.4, &EecFaults::default(), 0.02);
        assert!(s.powered());
        assert_ne!(s.active, ActiveChannel::None);
    }

    #[test]
    fn a_failed_alternator_falls_back_to_the_aircraft_network_then_to_nothing() {
        let mut eec = Eec::new();
        assert_eq!(eec.step_power(&power(60.0, true, false, 1.0)), PowerSource::Aircraft);
        assert_eq!(eec.step_power(&power(60.0, false, false, 1.0)), PowerSource::Unpowered);
        let s = eec.step(truth(), 3.4, 3.4, &EecFaults::default(), 0.02);
        assert!(!s.powered());
        assert_eq!(s.active, ActiveChannel::None);
        assert!(s.channel_a_serviceable && s.channel_b_serviceable, "losing power is not a channel fault");
    }

    #[test]
    fn the_controlling_channel_alternates_at_each_engine_start() {
        let mut eec = Eec::new();
        let mut active_after_start = Vec::new();
        for _ in 0..3 {
            eec.step_power(&power(0.0, true, true, 0.0));
            eec.step_power(&power(30.0, true, true, 0.0));
            eec.step_power(&power(60.0, true, false, 0.0));
            active_after_start.push(eec.step(truth(), 3.4, 3.4, &EecFaults::default(), 0.02).active);
        }
        assert_eq!(active_after_start, vec![ActiveChannel::B, ActiveChannel::A, ActiveChannel::B]);
    }

    #[test]
    fn a_failed_controlling_channel_hands_over_whichever_one_was_in_control() {
        let mut eec = Eec::new();
        eec.step_power(&power(0.0, true, true, 0.0));
        eec.step_power(&power(60.0, true, false, 0.0));
        assert_eq!(eec.preferred_channel(), ActiveChannel::B);
        let s = eec.step(truth(), 3.4, 3.4, &EecFaults { channel_b_fault: 1.0, ..EecFaults::default() }, 0.02);
        assert_eq!(s.active, ActiveChannel::A);
    }

    #[test]
    fn zero_dt_gives_no_nan() {
        let mut eec = Eec::new();
        let s = eec.step(truth(), 3.4, 3.4, &EecFaults::default(), 0.0);
        assert!(!s.selected[0].is_nan());
    }
}

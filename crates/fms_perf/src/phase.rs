use crate::speeds::FlightPhase;

pub mod vertical_mode {
    pub const NONE: i32 = 0;
    pub const ALT: i32 = 10;
    pub const ALT_CPT: i32 = 11;
    pub const OP_CLB: i32 = 12;
    pub const OP_DES: i32 = 13;
    pub const VS: i32 = 14;
    pub const FPA: i32 = 15;
    pub const ALT_CST: i32 = 20;
    pub const ALT_CST_CPT: i32 = 21;
    pub const CLB: i32 = 22;
    pub const DES: i32 = 23;
    pub const FINAL: i32 = 24;
    pub const GS_CPT: i32 = 30;
    pub const GS_TRACK: i32 = 31;
    pub const LAND: i32 = 32;
    pub const FLARE: i32 = 33;
    pub const ROLL_OUT: i32 = 34;
    pub const SRS: i32 = 40;
    pub const SRS_GA: i32 = 41;
    pub const TCAS: i32 = 50;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ConfirmationNode {
    delay_ms: f64,
    timer_ms: f64,
    output: bool,
}

impl ConfirmationNode {
    pub fn new(delay_ms: f64) -> Self {
        ConfirmationNode { delay_ms, timer_ms: 0.0, output: false }
    }

    pub fn update(&mut self, input: bool, dt_ms: f64) -> bool {
        if input {
            self.timer_ms += dt_ms;
            self.output = self.timer_ms >= self.delay_ms;
        } else {
            self.timer_ms = 0.0;
            self.output = false;
        }
        self.output
    }

    pub fn output(&self) -> bool {
        self.output
    }
}

#[derive(Clone, Copy, Debug)]
pub struct PhaseInputs {
    pub ready: bool,
    pub slew_active: bool,
    pub on_ground: bool,
    pub n1_pct: [f64; 4],
    pub n2_pct: [f64; 4],
    pub vertical_mode: i32,
    pub ground_speed_kt: f64,
    pub pressure_altitude_ft: Option<f64>,
    pub baro_corrected_altitude_ft: Option<f64>,
    pub cas_kt: Option<f64>,
    pub engine_out: bool,
    pub green_dot_kt: f64,
    pub cruise_altitude_ft: f64,
    pub fcu_selected_altitude_ft: f64,
    pub enable_approach_phase: bool,
    pub altitude_above_ground_ft: f64,
    pub indicated_altitude_ft: f64,
    pub fm_acc_alt_ft: Option<f64>,
    pub fm_eo_acc_alt_ft: Option<f64>,
    pub config_accel_alt_ft: f64,
}

impl Default for PhaseInputs {
    fn default() -> Self {
        PhaseInputs {
            ready: true,
            slew_active: false,
            on_ground: true,
            n1_pct: [0.0; 4],
            n2_pct: [0.0; 4],
            vertical_mode: vertical_mode::NONE,
            ground_speed_kt: 0.0,
            pressure_altitude_ft: None,
            baro_corrected_altitude_ft: None,
            cas_kt: None,
            engine_out: false,
            green_dot_kt: 0.0,
            cruise_altitude_ft: 0.0,
            fcu_selected_altitude_ft: 0.0,
            enable_approach_phase: false,
            altitude_above_ground_ft: 0.0,
            indicated_altitude_ft: 0.0,
            fm_acc_alt_ft: None,
            fm_eo_acc_alt_ft: None,
            config_accel_alt_ft: 1500.0,
        }
    }
}

impl PhaseInputs {
    fn engine_on(&self, index: usize) -> bool {
        self.n2_pct[index - 1] > 20.0
    }

    fn engine_on_takeoff_thrust(&self, index: usize) -> bool {
        self.n1_pct[index - 1] >= 70.0
    }

    pub fn an_engine_on(&self) -> bool {
        self.engine_on(2) || self.engine_on(3)
    }

    pub fn condition_take_off(&self) -> bool {
        (self.vertical_mode == vertical_mode::SRS
            && self.engine_on_takeoff_thrust(1)
            && self.engine_on_takeoff_thrust(2))
            || self.ground_speed_kt.abs() > 90.0
    }

    fn can_initiate_des(&self, distance_to_destination: f64) -> bool {
        let fl = (self.indicated_altitude_ft / 100.0 + 0.5).floor();
        let fcu_sel_fl = self.fcu_selected_altitude_ft / 100.0;
        let cruise_fl = self.cruise_altitude_ft / 100.0;
        cruise_fl > 0.0
            && (((distance_to_destination < 200.0 || fl < 200.0) && fcu_sel_fl < cruise_fl && fcu_sel_fl < fl)
                || (distance_to_destination >= 200.0 && fl > 200.0 && fcu_sel_fl <= 200.0))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PhaseChange {
    pub previous: FlightPhase,
    pub next: FlightPhase,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VsKnobResult {
    None,
    Descent,
    StepClimbDescent,
}

#[derive(Clone, Debug)]
pub struct FlightPhaseManager {
    phase: FlightPhase,
    next_phase: FlightPhase,
    on_ground_confirmation: ConfirmationNode,
    takeoff_confirmation: ConfirmationNode,
    landing_confirmation: ConfirmationNode,
    acceleration_altitude_msl: f64,
    acceleration_altitude_msl_eo: f64,
    transitions: Vec<PhaseChange>,
    flex_temp_cleared: bool,
    cold_and_dark_cleared: bool,
}

impl Default for FlightPhaseManager {
    fn default() -> Self {
        Self::new()
    }
}

impl FlightPhaseManager {
    pub fn new() -> Self {
        FlightPhaseManager {
            phase: FlightPhase::Preflight,
            next_phase: FlightPhase::Takeoff,
            on_ground_confirmation: ConfirmationNode::new(30_000.0),
            takeoff_confirmation: ConfirmationNode::new(200.0),
            landing_confirmation: ConfirmationNode::new(30_000.0),
            acceleration_altitude_msl: 0.0,
            acceleration_altitude_msl_eo: 0.0,
            transitions: Vec::new(),
            flex_temp_cleared: false,
            cold_and_dark_cleared: false,
        }
    }

    pub fn phase(&self) -> FlightPhase {
        self.phase
    }

    pub fn fmgc_flight_phase_lvar(&self) -> f64 {
        self.phase as u8 as f64
    }

    pub fn airliner_flight_phase_lvar(&self) -> f64 {
        if self.phase < FlightPhase::Takeoff {
            FlightPhase::Preflight as u8 as f64
        } else {
            (self.phase as u8 + 1) as f64
        }
    }

    pub fn take_transitions(&mut self) -> Vec<PhaseChange> {
        std::mem::take(&mut self.transitions)
    }

    pub fn take_flex_temp_cleared(&mut self) -> bool {
        std::mem::take(&mut self.flex_temp_cleared)
    }

    pub fn take_cold_and_dark_cleared(&mut self) -> bool {
        std::mem::take(&mut self.cold_and_dark_cleared)
    }

    pub fn init(&mut self, inputs: &PhaseInputs) {
        let p = self.phase;
        self.init_phase(p, inputs);
    }

    pub fn spawn(&mut self, gear_on_ground: bool, plane_altitude_ft: f64, flaps_handle_index: i32, inputs: &PhaseInputs) {
        let target = if gear_on_ground {
            FlightPhase::Preflight
        } else if plane_altitude_ft > 18_000.0 {
            FlightPhase::Cruise
        } else if flaps_handle_index > 0 {
            FlightPhase::Approach
        } else {
            FlightPhase::Climb
        };
        self.change_phase(target, inputs);
    }

    fn init_phase(&mut self, phase: FlightPhase, inputs: &PhaseInputs) {
        match phase {
            FlightPhase::Preflight => self.next_phase = FlightPhase::Takeoff,
            FlightPhase::Takeoff => {
                self.next_phase = FlightPhase::Climb;
                self.cold_and_dark_cleared = true;
                self.acceleration_altitude_msl = inputs
                    .fm_acc_alt_ft
                    .unwrap_or(inputs.indicated_altitude_ft + inputs.config_accel_alt_ft);
                self.acceleration_altitude_msl_eo = inputs
                    .fm_eo_acc_alt_ft
                    .unwrap_or(inputs.indicated_altitude_ft + inputs.config_accel_alt_ft);
            }
            FlightPhase::Climb => self.next_phase = FlightPhase::Cruise,
            FlightPhase::Cruise => self.next_phase = FlightPhase::Cruise,
            FlightPhase::Descent => self.next_phase = FlightPhase::Approach,
            FlightPhase::Approach => {
                self.flex_temp_cleared = true;
                self.next_phase = FlightPhase::Done;
            }
            FlightPhase::GoAround => {
                self.flex_temp_cleared = true;
                self.next_phase = FlightPhase::GoAround;
            }
            FlightPhase::Done => {
                self.flex_temp_cleared = true;
                self.next_phase = FlightPhase::Done;
            }
        }
    }

    pub fn change_phase(&mut self, new_phase: FlightPhase, inputs: &PhaseInputs) {
        let prev = self.phase;
        self.phase = new_phase;
        self.init_phase(new_phase, inputs);
        self.transitions.push(PhaseChange { previous: prev, next: new_phase });
        self.update(0.0, inputs);
    }

    fn should_activate_done_phase(&mut self, dt_ms: f64, inputs: &PhaseInputs) -> bool {
        let confirmed = self.on_ground_confirmation.update(inputs.on_ground, dt_ms);
        confirmed && !inputs.an_engine_on() && self.phase != FlightPhase::Done && self.phase != FlightPhase::Preflight
    }

    fn should_activate_next_phase(&mut self, dt_ms: f64, inputs: &PhaseInputs) -> bool {
        match self.phase {
            FlightPhase::Preflight => self.takeoff_confirmation.update(inputs.condition_take_off(), dt_ms),
            FlightPhase::Takeoff => {
                if let Some(baro) = inputs.baro_corrected_altitude_ft {
                    if inputs.engine_out {
                        let cas = inputs.cas_kt.unwrap_or(0.0);
                        if cas >= inputs.green_dot_kt && baro > self.acceleration_altitude_msl_eo {
                            return true;
                        }
                    } else {
                        return baro > self.acceleration_altitude_msl;
                    }
                }
                matches!(
                    inputs.vertical_mode,
                    vertical_mode::VS
                        | vertical_mode::FPA
                        | vertical_mode::ALT
                        | vertical_mode::ALT_CPT
                        | vertical_mode::ALT_CST
                        | vertical_mode::ALT_CST_CPT
                        | vertical_mode::OP_CLB
                        | vertical_mode::CLB
                )
            }
            FlightPhase::Climb => {
                let Some(pa) = inputs.pressure_altitude_ft else { return false };
                let cruise_fl = inputs.cruise_altitude_ft / 100.0;
                let fl = (pa / 100.0 + 0.5).floor();
                cruise_fl > 0.0 && fl >= cruise_fl
            }
            FlightPhase::Cruise => false,
            FlightPhase::Descent => {
                let fl = inputs.pressure_altitude_ft.map(|p| (p / 100.0 + 0.5).floor());
                let fcu_sel_fl = inputs.fcu_selected_altitude_ft / 100.0;
                let cruise_fl = inputs.cruise_altitude_ft / 100.0;
                if let Some(fl) = fl {
                    if cruise_fl > 0.0 && fl == cruise_fl && fcu_sel_fl == fl {
                        self.next_phase = FlightPhase::Cruise;
                        return true;
                    }
                }
                inputs.enable_approach_phase
            }
            FlightPhase::Approach => {
                if inputs.vertical_mode == vertical_mode::SRS_GA {
                    self.next_phase = FlightPhase::GoAround;
                    return true;
                }
                let landed = self.landing_confirmation.update(inputs.on_ground, dt_ms);
                landed || !inputs.an_engine_on()
            }
            FlightPhase::GoAround | FlightPhase::Done => false,
        }
    }

    pub fn update(&mut self, dt_ms: f64, inputs: &PhaseInputs) {
        if inputs.ready && !inputs.slew_active {
            if self.should_activate_done_phase(dt_ms, inputs) {
                self.change_phase(FlightPhase::Done, inputs);
            } else if self.should_activate_next_phase(dt_ms, inputs) {
                let next = self.next_phase;
                self.change_phase(next, inputs);
            }
        } else if inputs.ready && inputs.slew_active {
            self.handle_slew_situation(inputs);
        }
    }

    fn handle_slew_situation(&mut self, inputs: &PhaseInputs) {
        match self.phase {
            FlightPhase::Preflight | FlightPhase::Takeoff | FlightPhase::Done => {
                if inputs.altitude_above_ground_ft >= 1500.0 {
                    self.change_phase(FlightPhase::Climb, inputs);
                }
            }
            _ => {}
        }
    }

    pub fn handle_fcu_alt_knob_push_pull(&mut self, distance_to_destination: f64, inputs: &PhaseInputs) {
        match self.phase {
            FlightPhase::Takeoff => self.change_phase(FlightPhase::Climb, inputs),
            FlightPhase::Climb | FlightPhase::Cruise => {
                if inputs.can_initiate_des(distance_to_destination) {
                    self.change_phase(FlightPhase::Descent, inputs);
                }
            }
            _ => {}
        }
    }

    pub fn handle_fcu_alt_knob_turn(&mut self, distance_to_destination: f64, vs_selected: f64, fpa_selected: f64, inputs: &PhaseInputs) {
        if self.phase == FlightPhase::Cruise {
            let m = inputs.vertical_mode;
            if (m == vertical_mode::OP_DES
                || (m == vertical_mode::VS && vs_selected < 0.0)
                || (m == vertical_mode::FPA && fpa_selected < 0.0)
                || m == vertical_mode::DES)
                && inputs.can_initiate_des(distance_to_destination)
            {
                self.change_phase(FlightPhase::Descent, inputs);
            }
        }
    }

    pub fn handle_fcu_vs_knob(
        &mut self,
        distance_to_destination: f64,
        vs_selected: f64,
        fpa_selected: f64,
        inputs: &PhaseInputs,
    ) -> VsKnobResult {
        if self.phase == FlightPhase::Climb || self.phase == FlightPhase::Cruise {
            let m = inputs.vertical_mode;
            if (m == vertical_mode::VS && vs_selected < 0.0) || (m == vertical_mode::FPA && fpa_selected < 0.0) {
                if inputs.can_initiate_des(distance_to_destination) {
                    self.change_phase(FlightPhase::Descent, inputs);
                    return VsKnobResult::Descent;
                }
                return VsKnobResult::StepClimbDescent;
            }
        }
        VsKnobResult::None
    }

    pub fn handle_new_cruise_altitude_entered(&mut self, new_cruise_flight_level: f64, inputs: &PhaseInputs) {
        let current_fl = inputs.pressure_altitude_ft.map(|p| (p / 100.0 + 0.5).floor());
        if self.phase == FlightPhase::Approach {
            self.change_phase(FlightPhase::Climb, inputs);
        } else if let Some(fl) = current_fl {
            if fl < new_cruise_flight_level && self.phase == FlightPhase::Descent {
                self.change_phase(FlightPhase::Climb, inputs);
            } else if fl > new_cruise_flight_level
                && (self.phase == FlightPhase::Climb || self.phase == FlightPhase::Descent)
            {
                self.change_phase(FlightPhase::Cruise, inputs);
            }
        }
    }

    pub fn handle_new_destination_airport_entered(&mut self, inputs: &PhaseInputs) {
        if self.phase == FlightPhase::GoAround {
            let acc = if inputs.engine_out { inputs.fm_eo_acc_alt_ft } else { inputs.fm_acc_alt_ft };
            if inputs.indicated_altitude_ft > acc.unwrap_or(0.0) {
                self.change_phase(FlightPhase::Climb, inputs);
            }
        }
    }

    pub fn try_go_in_approach_phase(&mut self, inputs: &PhaseInputs) -> bool {
        if matches!(self.phase, FlightPhase::Preflight | FlightPhase::Takeoff | FlightPhase::Done) {
            return false;
        }
        if self.phase != FlightPhase::Approach {
            self.change_phase(FlightPhase::Approach, inputs);
        }
        true
    }
}

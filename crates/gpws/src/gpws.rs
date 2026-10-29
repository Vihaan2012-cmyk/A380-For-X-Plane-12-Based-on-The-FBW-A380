use crate::arinc::{DiscreteWord, Ssm, Word};
use crate::callouts::{CalloutContext, Callouts, Event, DEFAULT_AUTO_CALL_OUTS};
use crate::sound::{self, Sound, SoundManager};
use crate::throttle::Throttler;

pub mod phase {
    pub const PREFLIGHT: f64 = 0.;
    pub const TAKEOFF: f64 = 1.;
    pub const CLIMB: f64 = 2.;
    pub const CRUISE: f64 = 3.;
    pub const DESCENT: f64 = 4.;
    pub const APPROACH: f64 = 5.;
    pub const GO_AROUND: f64 = 6.;
    pub const DONE: f64 = 7.;
}

#[derive(Clone, Debug)]
pub struct Inputs {
    pub baro_alt: Word,
    pub radio_alt_1: Word,
    pub radio_alt_2: Word,
    pub on_ground: bool,
    pub minimum_descent_altitude: f64,
    pub decision_height: f64,
    pub flight_phase: f64,
    pub taws_selected: f64,
    pub gpws_failed: [bool; 2],
    pub gpws_sys_off: bool,
    pub landing_conf3: bool,
    pub flaps_handle_index: f64,
    pub vertical_speed_fpm: f64,
    pub airspeed_kt: f64,
    pub gear_pct_extended: f64,
    pub plane_altitude_ft: f64,
    pub gpws_flap_off: bool,
    pub gpws_flaps_off: bool,
    pub gpws_gs_off: bool,
    pub gs_valid: bool,
    pub gs_deviation: f64,
    pub autopilot_active: bool,
    pub tla: [f64; 4],
    pub f_factor: f64,
}

impl Default for Inputs {
    fn default() -> Inputs {
        Inputs {
            baro_alt: Word::failed(),
            radio_alt_1: Word::failed(),
            radio_alt_2: Word::failed(),
            on_ground: false,
            minimum_descent_altitude: 0.,
            decision_height: 0.,
            flight_phase: phase::PREFLIGHT,
            taws_selected: 0.,
            gpws_failed: [false; 2],
            gpws_sys_off: false,
            landing_conf3: false,
            flaps_handle_index: 0.,
            vertical_speed_fpm: 0.,
            airspeed_kt: 0.,
            gear_pct_extended: 0.,
            plane_altitude_ft: 0.,
            gpws_flap_off: false,
            gpws_flaps_off: false,
            gpws_gs_off: false,
            gs_valid: false,
            gs_deviation: 0.,
            autopilot_active: false,
            tla: [0.; 4],
            f_factor: 0.,
        }
    }
}

#[derive(Clone, Copy, Default, Debug)]
struct ModeType {
    sound: Option<Sound>,
    period: f64,
    light: bool,
    pull_up: bool,
}

const NONE: ModeType = ModeType { sound: None, period: 0., light: false, pull_up: false };
const LIGHT: ModeType = ModeType { sound: None, period: 0., light: true, pull_up: false };
const PULL_UP: ModeType = ModeType { sound: None, period: 0., light: true, pull_up: true };

const fn spoken(sound: Sound) -> ModeType {
    ModeType { sound: Some(sound), period: 1.1, light: true, pull_up: false }
}

const MODE_1_TYPES: [ModeType; 3] = [NONE, spoken(sound::SINK_RATE), PULL_UP];
const MODE_2_TYPES: [ModeType; 3] = [NONE, LIGHT, PULL_UP];
const MODE_3_TYPES: [ModeType; 2] = [NONE, spoken(sound::DONT_SINK)];
const MODE_4_TYPES: [ModeType; 4] =
    [NONE, spoken(sound::TOO_LOW_GEAR), spoken(sound::TOO_LOW_FLAPS), spoken(sound::TOO_LOW_TERRAIN)];
const MODE_5_TYPES: [ModeType; 3] = [NONE, NONE, NONE];

#[derive(Clone, Copy, Default, Debug)]
struct Mode {
    current: usize,
    previous: usize,
}

#[derive(Clone, Copy, Debug)]
pub struct Outputs {
    pub gpws_warning_active: bool,
    pub gs_warning_active: bool,
    pub alert_word_1: f64,
    pub alert_word_2: f64,
}

#[derive(Clone, Debug)]
pub struct Gpws {
    throttler: Throttler,
    pub sound: SoundManager,
    pub auto_call_out_pins: u32,
    minimums_state: u8,
    mode_3_max_baro_alt: f64,
    mode_4_max_ra_alt: f64,
    mode_2_boundary_leave_alt: f64,
    mode_2_num_terrain: u32,
    mode_2_num_frames_in_boundary: u32,
    radio_alt_rate: f64,
    prev_radio_alt: f64,
    prev_radio_alt_2: f64,
    modes: [Mode; 5],
    prev_should_pull_up_play: bool,
    callouts: Callouts,
    word_1: DiscreteWord,
    word_2: DiscreteWord,
    windshear_warning: bool,
    gpws_warning_active: bool,
    gs_warning_active: bool,
    events: Vec<Event>,
}

impl Gpws {
    pub const MODE_2_ENABLED: bool = false;

    pub fn new() -> Gpws {
        Gpws::with_throttle_offsets(0., 0.)
    }

    pub fn with_throttle_offsets(gpws_offset_ms: f64, sound_offset_ms: f64) -> Gpws {
        let mut g = Gpws {
            throttler: Throttler::new(125., gpws_offset_ms),
            sound: SoundManager::with_throttle_offset(sound_offset_ms),
            auto_call_out_pins: DEFAULT_AUTO_CALL_OUTS,
            minimums_state: 0,
            mode_3_max_baro_alt: f64::NAN,
            mode_4_max_ra_alt: f64::NAN,
            mode_2_boundary_leave_alt: f64::NAN,
            mode_2_num_terrain: 0,
            mode_2_num_frames_in_boundary: 0,
            radio_alt_rate: f64::NAN,
            prev_radio_alt: f64::NAN,
            prev_radio_alt_2: f64::NAN,
            modes: [Mode::default(); 5],
            prev_should_pull_up_play: false,
            callouts: Callouts::new(),
            word_1: DiscreteWord::empty(),
            word_2: DiscreteWord::empty(),
            windshear_warning: false,
            gpws_warning_active: false,
            gs_warning_active: false,
            events: Vec::new(),
        };
        g.init();
        g
    }

    fn init(&mut self) {
        self.set_glide_slope_warning(false);
        self.set_gpws_warning(false);
        self.word_1.ssm = Ssm::NormalOperation;
        self.word_1.set_bit(12, false);
    }

    pub fn outputs(&self) -> Outputs {
        Outputs {
            gpws_warning_active: self.gpws_warning_active,
            gs_warning_active: self.gs_warning_active,
            alert_word_1: self.word_1.raw(),
            alert_word_2: self.word_2.raw(),
        }
    }

    pub fn take_events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.events)
    }

    pub fn take_played_sounds(&mut self) -> Vec<&'static str> {
        self.sound.take_played()
    }

    pub fn mode_states(&self) -> [usize; 5] {
        [self.modes[0].current, self.modes[1].current, self.modes[2].current, self.modes[3].current, self.modes[4].current]
    }

    pub fn windshear_warning(&self) -> bool {
        self.windshear_warning
    }

    pub fn minimums_state(&self) -> u8 {
        self.minimums_state
    }

    pub fn update(&mut self, delta_ms: f64, inputs: &Inputs) {
        self.sound.update(delta_ms);
        let throttled = self.throttler.can_update(delta_ms);
        if throttled > 0. {
            self.gpws(throttled, inputs);
        }
    }

    fn set_glide_slope_warning(&mut self, state: bool) {
        self.gs_warning_active = state;
        self.word_2.set_bit(11, state);
    }

    fn set_gpws_warning(&mut self, state: bool) {
        self.gpws_warning_active = state;
        self.word_2.set_bit(12, state);
        self.word_2.set_bit(13, state);
    }

    fn update_discrete_words(&mut self) {
        let m = self.mode_states();
        self.word_1.ssm = Ssm::NormalOperation;
        self.word_1.set_bit(11, m[0] == 1);
        self.word_1.set_bit(12, m[0] == 2);
        self.word_1.set_bit(13, m[1] == 1);
        self.word_1.set_bit(12, m[1] == 2);
        self.word_1.set_bit(14, m[2] == 1);
        self.word_1.set_bit(15, m[3] == 1);
        self.word_1.set_bit(16, m[3] == 2);
        self.word_1.set_bit(17, m[3] == 3);
        self.word_1.set_bit(18, m[4] == 1);

        self.word_2.ssm = Ssm::NormalOperation;
        self.word_2.set_bit(14, false);
        self.word_2.set_bit(15, self.windshear_warning);
    }

    pub fn gpws(&mut self, delta_ms: f64, inputs: &Inputs) {
        let baro_alt = inputs.baro_alt;
        let radio_alt = if inputs.radio_alt_1.is_failure_warning() || inputs.radio_alt_1.is_no_computed_data() {
            inputs.radio_alt_2
        } else {
            inputs.radio_alt_1
        };
        let radio_alt_valid = radio_alt.is_normal_operation();
        let ra_value = f64::from(radio_alt.value);
        let on_ground = inputs.on_ground;
        let phase = inputs.flight_phase;

        self.update_alt_state(if radio_alt_valid { ra_value } else { f64::NAN }, inputs);
        self.differentiate_radio_alt(if radio_alt_valid { ra_value } else { f64::NAN }, delta_ms);

        let mda = inputs.minimum_descent_altitude;
        let dh = inputs.decision_height;

        if mda != 0. || (dh != -1. && dh != -2. && phase == phase::APPROACH) {
            let (minimums_da, minimums_ia) = if dh >= 0. {
                (dh, if radio_alt.is_normal_operation() || radio_alt.is_functional_test() { ra_value } else { f64::NAN })
            } else {
                (
                    mda,
                    if baro_alt.is_normal_operation() || baro_alt.is_functional_test() {
                        f64::from(baro_alt.value)
                    } else {
                        f64::NAN
                    },
                )
            };
            if minimums_da.is_finite() && minimums_ia.is_finite() {
                self.gpws_minimums(minimums_da, minimums_ia);
            }
        }

        let gpws_failed = if inputs.taws_selected == 1. {
            inputs.gpws_failed[0]
        } else if inputs.taws_selected == 2. {
            inputs.gpws_failed[1]
        } else {
            true
        };

        if radio_alt_valid && ra_value >= 10. && ra_value <= 2450. && !inputs.gpws_sys_off && !gpws_failed {
            let flap_position = inputs.flaps_handle_index;
            let flaps_in_landing_config = if inputs.landing_conf3 { flap_position == 3. } else { flap_position == 4. };
            let gear_extended = inputs.gear_pct_extended > 0.9;

            self.update_max_ra(ra_value, on_ground, phase);

            self.mode_1(ra_value, inputs.vertical_speed_fpm);
            self.mode_3(ra_value, phase, inputs.plane_altitude_ft);
            self.mode_4(ra_value, inputs.airspeed_kt, flaps_in_landing_config, gear_extended, phase, inputs.gpws_flap_off || inputs.gpws_flaps_off);
            self.mode_5(ra_value, inputs);
        } else {
            for mode in self.modes.iter_mut() {
                mode.current = 0;
            }

            self.mode_3_max_baro_alt = f64::NAN;
            if on_ground || (radio_alt_valid && ra_value < 10.) {
                self.mode_4_max_ra_alt = f64::NAN;
            }

            self.set_glide_slope_warning(false);
            self.set_gpws_warning(false);
        }

        let flaps_extended = inputs.flaps_handle_index > 0.;
        self.windshear_warning = inputs.f_factor > 0.105
            && !on_ground
            && radio_alt_valid
            && ra_value <= 1300.
            && flaps_extended
            && (ra_value >= 50. || inputs.vertical_speed_fpm > 0.);

        self.compute_lights_and_callouts();
        self.update_discrete_words();
    }

    fn differentiate_radio_alt(&mut self, radio_alt: f64, delta_ms: f64) {
        if !self.prev_radio_alt_2.is_nan() && !radio_alt.is_nan() {
            self.radio_alt_rate = (radio_alt - self.prev_radio_alt_2) / (delta_ms / 1000. / 60.) / 2.;
            self.prev_radio_alt_2 = self.prev_radio_alt;
            self.prev_radio_alt = radio_alt;
        } else if !self.prev_radio_alt.is_nan() && !radio_alt.is_nan() {
            self.prev_radio_alt_2 = self.prev_radio_alt;
            self.prev_radio_alt = radio_alt;
        } else {
            self.prev_radio_alt_2 = radio_alt;
        }
    }

    fn update_max_ra(&mut self, radio_alt: f64, on_ground: bool, phase: f64) {
        if on_ground || phase == phase::GO_AROUND {
            self.mode_4_max_ra_alt = f64::NAN;
        } else if self.mode_4_max_ra_alt < radio_alt || self.mode_4_max_ra_alt.is_nan() {
            self.mode_4_max_ra_alt = radio_alt;
        }
    }

    fn gpws_minimums(&mut self, minimums_da: f64, minimums_ia: f64) {
        let (over_minimums, over_100_above) = if minimums_da <= 90. {
            (minimums_ia >= minimums_da + 15., minimums_ia >= minimums_da + 115.)
        } else {
            (minimums_ia >= minimums_da + 5., minimums_ia >= minimums_da + 105.)
        };
        if self.minimums_state == 0 && over_minimums {
            self.minimums_state = 1;
        } else if self.minimums_state == 1 && over_100_above {
            self.minimums_state = 2;
        } else if self.minimums_state == 2 && !over_100_above {
            self.events.push(Event::Enqueue("hundred_above"));
            self.minimums_state = 1;
        } else if self.minimums_state == 1 && !over_minimums {
            self.events.push(Event::Enqueue("minimums"));
            self.minimums_state = 0;
        }
    }

    fn compute_lights_and_callouts(&mut self) {
        let tables: [&[ModeType]; 5] = [&MODE_1_TYPES, &MODE_2_TYPES, &MODE_3_TYPES, &MODE_4_TYPES, &MODE_5_TYPES];
        for i in 0..5 {
            let (current, previous) = (self.modes[i].current, self.modes[i].previous);
            if current == previous {
                continue;
            }

            self.sound.remove_periodic_sound(tables[i][previous].sound);
            let t = tables[i][current];
            self.sound.add_periodic_sound(t.sound, t.period);

            if i == 4 {
                self.set_glide_slope_warning(current >= 1);
            }

            self.modes[i].previous = current;
        }

        let should_pull_up_play = (0..5).any(|i| tables[i][self.modes[i].current].pull_up);
        if should_pull_up_play != self.prev_should_pull_up_play {
            if should_pull_up_play {
                self.sound.add_periodic_sound(Some(sound::PULL_UP), 1.1);
            } else {
                self.sound.remove_periodic_sound(Some(sound::PULL_UP));
            }
            self.prev_should_pull_up_play = should_pull_up_play;
        }

        let light = (0..5).any(|i| tables[i][self.modes[i].current].light);
        self.set_gpws_warning(light);
    }

    fn mode_1(&mut self, radio_alt: f64, v_speed: f64) {
        let sinkrate = -v_speed;

        if sinkrate <= 1000. {
            self.modes[0].current = 0;
            return;
        }

        let max_sinkrate_alt = 0.61 * sinkrate - 600.;
        let max_pull_up_alt = if sinkrate < 1700. { 1.3 * sinkrate - 1940. } else { 0.4 * sinkrate - 410. };

        self.modes[0].current = if radio_alt <= max_pull_up_alt {
            2
        } else if radio_alt <= max_sinkrate_alt {
            1
        } else {
            0
        };
    }

    pub fn mode_2(&mut self, radio_alt: f64, speed: f64, flaps_in_landing_config: bool, gear_extended: bool) {
        let mut is_in_boundary = false;
        let upper_boundary_rate = if -self.radio_alt_rate < 3500. {
            0.7937 * -self.radio_alt_rate - 1557.5
        } else {
            0.19166 * -self.radio_alt_rate + 610.
        };
        let upper_boundary_speed = 1650f64.max(2450f64.min(8.8888 * speed - 305.555));

        if !flaps_in_landing_config && -self.radio_alt_rate > 2000. {
            if radio_alt < upper_boundary_speed && radio_alt < upper_boundary_rate {
                self.mode_2_num_frames_in_boundary += 1;
            } else {
                self.mode_2_num_frames_in_boundary = 0;
            }
        } else if flaps_in_landing_config && -self.radio_alt_rate > 2000. {
            if radio_alt < 775. && radio_alt < upper_boundary_rate && -self.radio_alt_rate < 10000. {
                self.mode_2_num_frames_in_boundary += 1;
            } else {
                self.mode_2_num_frames_in_boundary = 0;
            }
        }
        if self.mode_2_num_frames_in_boundary > 5 {
            is_in_boundary = true;
        }

        if is_in_boundary {
            self.mode_2_boundary_leave_alt = -1.;
            if self.mode_2_num_terrain < 2 || gear_extended {
                if self.sound.try_play_sound(sound::TOO_LOW_TERRAIN) {
                    self.mode_2_num_terrain += 1;
                }
                self.modes[1].current = 1;
            } else if !gear_extended {
                self.modes[1].current = 2;
            }
        } else if self.mode_2_boundary_leave_alt == -1. {
            self.mode_2_boundary_leave_alt = radio_alt;
        } else if self.mode_2_boundary_leave_alt + 300. > radio_alt {
            self.modes[1].current = 1;
            self.sound.try_play_sound(sound::TOO_LOW_TERRAIN);
        } else if self.mode_2_boundary_leave_alt + 300. <= radio_alt {
            self.modes[1].current = 0;
            self.mode_2_num_terrain = 0;
            self.mode_2_boundary_leave_alt = f64::NAN;
        }
    }

    fn mode_3(&mut self, radio_alt: f64, phase: f64, baro_alt: f64) {
        if !(phase == phase::TAKEOFF || phase == phase::GO_AROUND) || radio_alt > 1500. || radio_alt < 10. {
            self.mode_3_max_baro_alt = f64::NAN;
            self.modes[2].current = 0;
            return;
        }

        let max_alt_loss = 0.09 * radio_alt + 7.1;

        if baro_alt > self.mode_3_max_baro_alt || self.mode_3_max_baro_alt.is_nan() {
            self.mode_3_max_baro_alt = baro_alt;
            self.modes[2].current = 0;
        } else if self.mode_3_max_baro_alt - baro_alt > max_alt_loss {
            self.modes[2].current = 1;
        } else {
            self.modes[2].current = 0;
        }
    }

    fn mode_4(
        &mut self,
        radio_alt: f64,
        speed: f64,
        flaps_in_landing_config: bool,
        gear_extended: bool,
        phase: f64,
        flap_mode_off: bool,
    ) {
        if radio_alt < 30. || radio_alt > 1000. {
            self.modes[3].current = 0;
            return;
        }

        if !gear_extended && phase == phase::APPROACH {
            if speed < 190. && radio_alt < 500. {
                self.modes[3].current = 1;
            } else if speed >= 190. {
                let max_warn_alt = 8.333 * speed - 1083.333;
                self.modes[3].current = if radio_alt < max_warn_alt { 3 } else { 0 };
            }
        } else if !flaps_in_landing_config && !flap_mode_off && phase == phase::APPROACH {
            if speed < 159. && radio_alt < 245. {
                self.modes[3].current = 2;
            } else if speed >= 159. {
                let max_warn_alt = 8.2967 * speed - 1074.18;
                self.modes[3].current = if radio_alt < max_warn_alt { 3 } else { 0 };
            }
        } else {
            self.modes[3].current = 0;
        }
        if !flaps_in_landing_config || !gear_extended {
            let max_warn_alt_speed = 500f64.max(1000f64.min(8.3333 * speed - 1083.33));
            let max_warn_alt = 0.750751 * self.mode_4_max_ra_alt - 0.750751;

            if self.mode_4_max_ra_alt > 100. && radio_alt < max_warn_alt_speed && radio_alt < max_warn_alt {
                self.modes[3].current = 3;
            }
        }
    }

    fn mode_5(&mut self, radio_alt: f64, inputs: &Inputs) {
        if radio_alt > 1000. || radio_alt < 30. || inputs.gpws_gs_off {
            self.modes[4].current = 0;
            return;
        }
        if !inputs.gs_valid {
            self.modes[4].current = 0;
            return;
        }
        let dots = -inputs.gs_deviation * 2.5;

        let min_alt_for_warning = if dots < 2.9 { -75. * dots + 247.5 } else { 30. };
        let min_alt_for_hard_warning = if dots < 3.8 { -66.66 * dots + 283.33 } else { 30. };

        self.modes[4].current = if dots > 2. && radio_alt > min_alt_for_hard_warning && radio_alt < 350. {
            2
        } else if dots > 1.3 && radio_alt > min_alt_for_warning {
            1
        } else {
            0
        };
    }

    fn update_alt_state(&mut self, radio_alt: f64, inputs: &Inputs) {
        let ctx = CalloutContext {
            auto_call_out_pins: self.auto_call_out_pins,
            autopilot_active: inputs.autopilot_active,
            tla: inputs.tla,
            go_around_phase: inputs.flight_phase == phase::GO_AROUND,
        };
        let mut events = std::mem::take(&mut self.events);
        self.callouts.update(radio_alt, &ctx, &mut events);
        self.events = events;
    }
}

impl Default for Gpws {
    fn default() -> Self {
        Gpws::new()
    }
}

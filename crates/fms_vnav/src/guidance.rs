// Translated from FlyByWire's fmgc guidance/vnav/descent/AircraftToProfileRelation.ts, SpeedMargin.ts, TodGuidance.ts,
// DescentGuidance.ts and LatchedDescentGuidance.ts (GPL-3.0-or-later). SimVar reads come in through `GuidanceInputs`,
// SimVar writes go to `GuidanceOutputs`, which keeps the last written value of each L:var like the sim does.

use crate::config::AircraftConfig;
use crate::manager::DescentProfileRelation;
use crate::modes::{lateral, vertical, FcuModes};
use crate::profile::{Checkpoint, ComputationParameters, FlightPhase, Profile, Reason};

#[derive(Clone, Debug, Default)]
pub struct AircraftToDescentProfileRelation {
    pub is_valid: bool,
    top_of_descent: Option<Checkpoint>,
    geometric_path_start: Option<Checkpoint>,
    distance_to_end: f64,
    pub total_flight_plan_distance: f64,
}

impl AircraftToDescentProfileRelation {
    pub fn distance_from_start(&self) -> f64 {
        self.total_flight_plan_distance - self.distance_to_end
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn update_profile(&mut self, profile: Option<&Profile>) {
        let find = |r: Reason| profile.and_then(|p| p.checkpoints.iter().find(|c| c.reason == r).copied());
        let tod = find(Reason::TopOfDescent);
        let gps = find(Reason::GeometricPathStart);
        let (Some(tod), Some(gps), Some(profile)) = (tod, gps, profile) else {
            self.invalidate();
            return;
        };
        self.is_valid = true;
        self.top_of_descent = Some(tod);
        self.geometric_path_start = Some(gps);
        self.total_flight_plan_distance = profile.total_flight_plan_distance;
        self.distance_to_end = profile.total_flight_plan_distance - profile.distance_to_present_position;
    }

    fn invalidate(&mut self) {
        self.is_valid = false;
        self.top_of_descent = None;
    }

    pub fn update(&mut self, distance_to_end: f64) {
        if !self.is_valid {
            return;
        }
        if !distance_to_end.is_finite() {
            self.invalidate();
            return;
        }
        self.distance_to_end = distance_to_end;
    }

    pub fn is_past_top_of_descent(&self) -> bool {
        self.distance_to_top_of_descent().is_some_and(|d| d < 0.0)
    }

    pub fn distance_to_top_of_descent(&self) -> Option<f64> {
        self.top_of_descent.map(|t| t.distance_from_start - self.distance_from_start())
    }

    pub fn is_on_geometric_path(&self) -> bool {
        self.geometric_path_start.is_some_and(|g| self.distance_from_start() > g.distance_from_start)
    }

    pub fn compute_linear_deviation(&self, profile: &Profile, present_altitude: f64) -> f64 {
        present_altitude - self.current_target_altitude(profile)
    }

    pub fn current_target_altitude(&self, profile: &Profile) -> f64 {
        profile.interpolate_altitude_at_distance(self.distance_from_start())
    }

    pub fn current_target_path_angle(&self, profile: &Profile) -> f64 {
        profile.interpolate_path_angle_at_distance(self.distance_from_start())
    }

    pub fn current_target_vertical_speed(&self, profile: &Profile, ground_speed: f64) -> f64 {
        101.269 * ground_speed * self.current_target_path_angle(profile).to_radians().tan()
    }

    pub fn is_above_speed_limit_altitude(&self, p: &ComputationParameters) -> bool {
        p.descent_speed_limit.is_some_and(|l| p.present_altitude > l.under_altitude)
    }

    pub fn is_close_to_airfield_elevation(&self, p: &ComputationParameters) -> bool {
        p.present_altitude < p.destination_elevation + 5000.0
    }

    pub fn current_distance_to_end(&self) -> f64 {
        self.distance_to_end
    }

    /// The values the profile manager reads from the relation while it builds the tactical paths.
    pub fn snapshot(&self, profile: Option<&Profile>, p: &ComputationParameters) -> DescentProfileRelation {
        let Some(profile) = profile.filter(|_| self.is_valid) else {
            return DescentProfileRelation { distance_from_start: self.distance_from_start(), ..Default::default() };
        };
        DescentProfileRelation {
            is_valid: true,
            distance_from_start: self.distance_from_start(),
            past_top_of_descent: self.is_past_top_of_descent(),
            linear_deviation: self.compute_linear_deviation(profile, p.present_altitude),
            on_geometric_path: self.is_on_geometric_path(),
            current_target_path_angle: self.current_target_path_angle(profile),
            above_speed_limit_altitude: self.is_above_speed_limit_altitude(p),
            close_to_airfield_elevation: self.is_close_to_airfield_elevation(p),
        }
    }
}

/// Speeds the guidance reads from the sim (FAC speeds, flaps lever) and the sim's "FROM MACH TO KIAS" conversion.
#[derive(Clone, Copy)]
pub struct SpeedData<'f> {
    pub vls: f64,
    pub vmax: f64,
    pub green_dot: f64,
    pub s_speed: f64,
    pub f_speed: f64,
    pub flaps_handle_index: i32,
    pub mach_to_kias: &'f dyn Fn(f64) -> f64,
}

impl SpeedData<'_> {
    pub fn vman(&self, approach_speed: f64) -> f64 {
        match self.flaps_handle_index {
            0 => self.green_dot,
            1 => self.s_speed,
            2 => self.f_speed,
            3 | 4 => approach_speed,
            _ => self.vls,
        }
    }
}

pub struct SpeedMargin {
    vmo: f64,
    mmo: f64,
}

impl SpeedMargin {
    pub fn new(config: &AircraftConfig) -> Self {
        Self { vmo: config.vnav.vmo, mmo: config.vnav.mmo }
    }

    pub fn get_target(&self, indicated_airspeed: f64, target_speed: f64, p: &ComputationParameters, s: &SpeedData) -> f64 {
        let (lower, upper) = self.get_margins(target_speed, p, s);
        indicated_airspeed.min(upper).max(lower)
    }

    pub fn get_margins(&self, current_target: f64, p: &ComputationParameters, s: &SpeedData) -> (f64, f64) {
        let vman = s.vman(p.approach_speed);
        let max_mach_as_ias = (s.mach_to_kias)(self.mmo - 0.006);
        let is_mach_target = p.managed_descent_speed - (s.mach_to_kias)(p.managed_descent_speed_mach) > 1.0;
        let to_lower = 20.0;
        let to_upper = if !is_mach_target && p.managed_descent_speed - current_target > 1.0 { 5.0 } else { 20.0 };
        let cap = |x: f64| s.vls.max(vman).max(x.min(s.vmax).min(self.vmo - 3.0).min(max_mach_as_ias));
        (cap(current_target - to_lower), cap(current_target + to_upper))
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum RequestedVerticalMode {
    #[default]
    None = 0,
    SpeedThrust = 1,
    VpathThrust = 2,
    VpathSpeed = 3,
    FpaSpeed = 4,
    VsSpeed = 5,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TodPopup {
    TopOfDescentPause,
    AutopilotProtection,
    Hide,
}

/// The L:vars the descent guidance writes, holding the last written value.
#[derive(Clone, Debug, Default)]
pub struct GuidanceOutputs {
    pub fg_requested_vertical_mode: RequestedVerticalMode,
    pub fg_target_altitude: f64,
    pub fg_target_vertical_speed: f64,
    pub pfd_target_altitude: f64,
    pub pfd_linear_deviation_active: bool,
    pub pfd_vertical_profile_latched: bool,
    pub speeds_managed_athr: Option<f64>,
    pub speeds_managed_pfd: Option<f64>,
    pub pfd_show_speed_margins: bool,
    pub pfd_lower_speed_margin: f64,
    pub pfd_upper_speed_margin: f64,
    pub pfd_msg_td_reached: bool,
    pub pause_at_tod_armed: bool,
    pub popups: Vec<TodPopup>,
}

/// What the guidance reads from the sim each update.
#[derive(Clone, Copy)]
pub struct GuidanceInputs<'f> {
    pub modes: FcuModes,
    pub speeds: SpeedData<'f>,
    pub autopilot_airspeed_managed: bool,
    pub manual_hold_active: bool,
    pub inertial_vertical_speed: Option<f64>,
    pub barometric_vertical_speed: Option<f64>,
    pub ground_speed: f64,
    pub current_airspeed: f64,
    pub current_pressure_altitude: f64,
    pub current_altitude: Option<f64>,
    pub speeds_managed_pfd: f64,
    pub speeds_managed_athr: f64,
    pub origin_transition_altitude: f64,
    pub autopilot_active: bool,
    pub fma_lateral_mode: i32,
    pub pause_at_tod_enabled: bool,
    pub pause_at_tod_distance: f64,
}

const TOD_PAUSE_TIMEOUT_MS: f64 = 10_000.0;

#[derive(Clone, Debug, Default)]
pub struct TodGuidance {
    td_reached: bool,
    td_paused: bool,
    ap_engaged: bool,
    cooldown: f64,
}

impl TodGuidance {
    pub fn update(&mut self, dt_ms: f64, rel: &AircraftToDescentProfileRelation, p: &ComputationParameters, i: &GuidanceInputs, out: &mut GuidanceOutputs) {
        self.update_td_reached(rel, p, out);
        if i.pause_at_tod_enabled {
            self.update_td_pause(dt_ms, rel, p, i, out);
        }
    }

    fn show_pause_popup(&mut self, popup: TodPopup, out: &mut GuidanceOutputs) {
        self.cooldown = TOD_PAUSE_TIMEOUT_MS;
        out.popups.push(popup);
    }

    fn update_td_pause(&mut self, dt_ms: f64, rel: &AircraftToDescentProfileRelation, p: &ComputationParameters, i: &GuidanceInputs, out: &mut GuidanceOutputs) {
        out.pause_at_tod_armed = self.cooldown <= 0.0 && !self.td_paused && p.flight_phase >= FlightPhase::Climb && p.flight_phase <= FlightPhase::Cruise;
        if out.pause_at_tod_armed {
            if rel.distance_to_top_of_descent().unwrap_or(f64::INFINITY) < i.pause_at_tod_distance {
                self.td_paused = true;
                self.show_pause_popup(TodPopup::TopOfDescentPause, out);
            } else if i.current_altitude.is_some_and(|a| a != 0.0 && a > i.origin_transition_altitude) {
                let ap_active = i.autopilot_active && i.fma_lateral_mode == lateral::NAV;
                if self.ap_engaged && !ap_active {
                    self.show_pause_popup(TodPopup::AutopilotProtection, out);
                }
                self.ap_engaged = ap_active;
            }
        }
        if p.flight_phase == FlightPhase::Done || p.flight_phase == FlightPhase::Preflight {
            self.td_paused = false;
            self.ap_engaged = false;
            out.popups.push(TodPopup::Hide);
        }
        if self.cooldown > 0.0 {
            self.cooldown = (self.cooldown - dt_ms).max(0.0);
        }
    }

    fn update_td_reached(&mut self, rel: &AircraftToDescentProfileRelation, p: &ComputationParameters, out: &mut GuidanceOutputs) {
        let reached = p.flight_phase >= FlightPhase::Climb && p.flight_phase <= FlightPhase::Cruise && rel.is_past_top_of_descent();
        if reached != self.td_reached {
            self.td_reached = reached;
            out.pfd_msg_td_reached = reached;
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
enum VerticalState {
    #[default]
    InvalidProfile,
    ProvidingGuidance,
    Observing,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
enum SpeedState {
    #[default]
    NotInDescentPhase,
    TargetOnly,
    TargetAndMargins,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum PathCaptureState {
    #[default]
    OffPath,
    OnPath,
    InPathCapture,
}

pub struct PathCaptureProfile {
    pub path_capture_gain: f64,
    pub path_disengagement_gain: f64,
    pub fallback_path_capture_deviation: f64,
    pub max_on_path_deviation: f64,
    pub min_capture_deviation: f64,
    pub max_capture_deviation: f64,
}

pub const VPATH_CAPTURE_PROFILE: PathCaptureProfile = PathCaptureProfile {
    path_capture_gain: 0.1,
    path_disengagement_gain: 0.2,
    fallback_path_capture_deviation: 100.0,
    max_on_path_deviation: 100.0,
    min_capture_deviation: 50.0,
    max_capture_deviation: 500.0,
};

fn ias_or_mach(ias: f64, mach: f64, s: &SpeedData) -> f64 {
    let mach_as_ias = (s.mach_to_kias)(mach);
    if ias > mach_as_ias {
        mach_as_ias
    } else {
        ias
    }
}

fn js_round(x: f64) -> f64 {
    (x + 0.5).floor()
}

fn change_speed_state(state: &mut SpeedState, new_state: SpeedState, out: &mut GuidanceOutputs) {
    if *state == new_state {
        return;
    }
    if *state == SpeedState::TargetAndMargins {
        out.pfd_show_speed_margins = false;
        out.pfd_lower_speed_margin = 0.0;
        out.pfd_upper_speed_margin = 0.0;
    } else if new_state == SpeedState::TargetAndMargins {
        out.pfd_show_speed_margins = true;
    }
    *state = new_state;
}

pub struct DescentGuidance {
    vertical_state: VerticalState,
    speed_state: SpeedState,
    requested_vertical_mode: RequestedVerticalMode,
    target_altitude: f64,
    target_altitude_guidance: f64,
    target_vertical_speed: f64,
    show_linear_deviation_on_pfd: bool,
    show_descent_latch_on_pfd: bool,
    speed_margin: SpeedMargin,
    tod_guidance: TodGuidance,
    speed_target: f64,
    is_in_overspeed_condition: bool,
    is_in_underspeed_condition: bool,
    path_capture_state: PathCaptureState,
    pub out: GuidanceOutputs,
}

impl DescentGuidance {
    pub fn new(config: &AircraftConfig) -> Self {
        let mut g = Self {
            vertical_state: VerticalState::InvalidProfile,
            speed_state: SpeedState::NotInDescentPhase,
            requested_vertical_mode: RequestedVerticalMode::None,
            target_altitude: 0.0,
            target_altitude_guidance: 0.0,
            target_vertical_speed: 0.0,
            show_linear_deviation_on_pfd: false,
            show_descent_latch_on_pfd: false,
            speed_margin: SpeedMargin::new(config),
            tod_guidance: TodGuidance::default(),
            speed_target: f64::NAN,
            is_in_overspeed_condition: false,
            is_in_underspeed_condition: false,
            path_capture_state: PathCaptureState::OffPath,
            out: GuidanceOutputs::default(),
        };
        g.write_to_sim_vars();
        g
    }

    pub fn path_capture_state(&self) -> PathCaptureState {
        self.path_capture_state
    }

    pub fn update_profile(&mut self, rel: &mut AircraftToDescentProfileRelation, profile: Option<&Profile>) {
        rel.update_profile(profile);
        if self.vertical_state != VerticalState::InvalidProfile && !rel.is_valid {
            self.change_state(VerticalState::InvalidProfile);
        } else if self.vertical_state == VerticalState::InvalidProfile && rel.is_valid {
            self.change_state(VerticalState::Observing);
        }
    }

    fn change_state(&mut self, new_state: VerticalState) {
        if self.vertical_state == new_state {
            return;
        }
        if new_state == VerticalState::InvalidProfile {
            self.reset();
            self.write_to_sim_vars();
        } else if new_state == VerticalState::Observing {
            self.path_capture_state = PathCaptureState::OffPath;
        }
        self.vertical_state = new_state;
    }

    pub fn reset(&mut self) {
        self.requested_vertical_mode = RequestedVerticalMode::None;
        self.target_altitude = 0.0;
        self.target_vertical_speed = 0.0;
        self.show_linear_deviation_on_pfd = false;
        self.show_descent_latch_on_pfd = false;
        self.is_in_overspeed_condition = false;
        self.path_capture_state = PathCaptureState::OffPath;
    }

    pub fn update(
        &mut self,
        dt_ms: f64,
        distance_to_end: f64,
        rel: &mut AircraftToDescentProfileRelation,
        profile: Option<&Profile>,
        p: &ComputationParameters,
        i: &GuidanceInputs,
    ) {
        rel.update(distance_to_end);
        let profile = match profile {
            Some(profile) if rel.is_valid => profile,
            _ => {
                self.change_state(VerticalState::InvalidProfile);
                return;
            }
        };
        if (i.modes.vertical == vertical::DES) != (self.vertical_state == VerticalState::ProvidingGuidance) {
            self.change_state(if self.vertical_state == VerticalState::ProvidingGuidance { VerticalState::Observing } else { VerticalState::ProvidingGuidance });
        }
        self.update_speed_margin_state(p, i);
        self.update_speed_target(p, i);
        self.update_speed_guidance(rel, p, i);
        self.update_over_underspeed_condition(p, i);
        self.update_linear_deviation(rel, profile, p, i);
        if self.vertical_state == VerticalState::ProvidingGuidance {
            self.update_des_mode_guidance(rel, profile, p, i);
        }
        self.write_to_sim_vars();
        self.tod_guidance.update(dt_ms, rel, p, i, &mut self.out);
    }

    fn update_linear_deviation(&mut self, rel: &AircraftToDescentProfileRelation, profile: &Profile, p: &ComputationParameters, i: &GuidanceInputs) {
        use vertical::*;
        self.target_altitude = rel.current_target_altitude(profile);
        self.show_linear_deviation_on_pfd = p.flight_phase < FlightPhase::GoAround
            && (p.flight_phase >= FlightPhase::Descent || rel.is_past_top_of_descent())
            && ![GS_CPT, GS_TRACK, LAND, FLARE, ROLL_OUT].contains(&i.modes.vertical);
    }

    fn update_des_mode_guidance(&mut self, rel: &AircraftToDescentProfileRelation, profile: &Profile, p: &ComputationParameters, i: &GuidanceInputs) {
        let on_geometric_path = rel.is_on_geometric_path();
        let above_speed_limit_altitude = rel.is_above_speed_limit_altitude(p);
        let close_to_airfield_elevation = rel.is_close_to_airfield_elevation(p);
        let before_top_of_descent = !rel.is_past_top_of_descent();
        let linear_deviation = rel.compute_linear_deviation(profile, p.present_altitude);
        let speed_auto = i.autopilot_airspeed_managed;
        let approach_phase = p.flight_phase == FlightPhase::Approach;
        let hold_active = i.manual_hold_active;
        let target_vertical_speed = rel.current_target_vertical_speed(profile, i.ground_speed);

        self.target_altitude_guidance = i.current_pressure_altitude - linear_deviation;
        self.update_path_capture_state(linear_deviation, target_vertical_speed, i);
        let go_off_path = self.path_capture_state == PathCaptureState::OffPath;

        if (!hold_active && go_off_path && linear_deviation > 0.0) || self.is_in_overspeed_condition {
            self.requested_vertical_mode = RequestedVerticalMode::SpeedThrust;
        } else if go_off_path || before_top_of_descent || hold_active {
            if hold_active {
                self.requested_vertical_mode = RequestedVerticalMode::VsSpeed;
                self.target_vertical_speed = -1000.0;
            } else if on_geometric_path {
                self.requested_vertical_mode = RequestedVerticalMode::FpaSpeed;
                self.target_vertical_speed = rel.current_target_path_angle(profile) / 2.0;
            } else {
                self.requested_vertical_mode = RequestedVerticalMode::VsSpeed;
                self.target_vertical_speed = if above_speed_limit_altitude && !close_to_airfield_elevation { -1000.0 } else { -500.0 };
            }
        } else if !on_geometric_path && speed_auto && !self.is_in_underspeed_condition && !approach_phase {
            self.requested_vertical_mode = RequestedVerticalMode::VpathThrust;
            self.target_vertical_speed = target_vertical_speed;
        } else {
            self.requested_vertical_mode = RequestedVerticalMode::VpathSpeed;
            self.target_vertical_speed = target_vertical_speed;
        }
    }

    fn update_path_capture_state(&mut self, linear_deviation: f64, target_vertical_speed: f64, i: &GuidanceInputs) {
        let c = &VPATH_CAPTURE_PROFILE;
        let allow = self.is_path_capture_condition_met(linear_deviation, target_vertical_speed, c.path_capture_gain, i);
        match self.path_capture_state {
            PathCaptureState::OffPath => {
                if allow {
                    self.path_capture_state = PathCaptureState::InPathCapture;
                }
            }
            PathCaptureState::OnPath => {
                if linear_deviation.abs() > c.min_capture_deviation && !allow {
                    self.path_capture_state = PathCaptureState::OffPath;
                }
            }
            PathCaptureState::InPathCapture => {
                let disengage = !self.is_path_capture_condition_met(linear_deviation, target_vertical_speed, c.path_capture_gain, i);
                if disengage {
                    self.path_capture_state = PathCaptureState::OffPath;
                } else if linear_deviation.abs() < c.min_capture_deviation {
                    self.path_capture_state = PathCaptureState::OnPath;
                }
            }
        }
    }

    fn is_path_capture_condition_met(&self, linear_deviation: f64, target_vertical_speed: f64, gain: f64, i: &GuidanceInputs) -> bool {
        let c = &VPATH_CAPTURE_PROFILE;
        // FBW tests `!verticalSpeed`, so an exact 0 ft/min also takes the fallback.
        let vs = i.inertial_vertical_speed.or(i.barometric_vertical_speed).filter(|v| *v != 0.0 && !v.is_nan());
        let Some(vs) = vs else {
            return linear_deviation.abs() < c.fallback_path_capture_deviation;
        };
        linear_deviation.abs() < c.max_capture_deviation.min(c.min_capture_deviation.max(gain * (vs - target_vertical_speed).abs()))
    }

    fn update_speed_target(&mut self, p: &ComputationParameters, i: &GuidanceInputs) {
        let managed = if p.flight_phase == FlightPhase::Approach {
            i.speeds_managed_athr
        } else {
            js_round(ias_or_mach(i.speeds_managed_pfd, p.managed_descent_speed_mach, &i.speeds))
        };
        let fcu_speed_ias = if p.fcu_speed < 1.0 { (i.speeds.mach_to_kias)(p.fcu_speed) } else { p.fcu_speed };
        self.speed_target = if i.autopilot_airspeed_managed { managed } else { fcu_speed_ias };
    }

    fn write_to_sim_vars(&mut self) {
        self.out.fg_requested_vertical_mode = self.requested_vertical_mode;
        self.out.fg_target_altitude = self.target_altitude_guidance;
        self.out.fg_target_vertical_speed = self.target_vertical_speed;
        self.out.pfd_target_altitude = self.target_altitude;
        self.out.pfd_linear_deviation_active = self.show_linear_deviation_on_pfd;
        self.out.pfd_vertical_profile_latched = self.show_descent_latch_on_pfd;
    }

    fn update_speed_guidance(&mut self, rel: &AircraftToDescentProfileRelation, p: &ComputationParameters, i: &GuidanceInputs) {
        match self.speed_state {
            SpeedState::NotInDescentPhase => return,
            SpeedState::TargetOnly => {
                self.out.speeds_managed_athr = Some(self.speed_target);
                return;
            }
            SpeedState::TargetAndMargins => {}
        }
        let (lower, upper) = self.speed_margin.get_margins(self.speed_target, p, &i.speeds);
        let on_geometric_path = rel.is_on_geometric_path();
        let mut target = self.speed_target;
        if self.requested_vertical_mode == RequestedVerticalMode::SpeedThrust && !self.is_in_overspeed_condition {
            target = upper;
        } else if self.requested_vertical_mode == RequestedVerticalMode::VpathThrust
            || (self.requested_vertical_mode == RequestedVerticalMode::VpathSpeed && !on_geometric_path)
        {
            target = lower;
        }
        self.out.speeds_managed_athr = Some(target);
        self.out.pfd_lower_speed_margin = lower;
        self.out.pfd_upper_speed_margin = upper;
    }

    fn update_speed_margin_state(&mut self, p: &ComputationParameters, i: &GuidanceInputs) {
        if p.flight_phase != FlightPhase::Descent {
            change_speed_state(&mut self.speed_state, SpeedState::NotInDescentPhase, &mut self.out);
            return;
        }
        let show = !i.manual_hold_active && self.vertical_state == VerticalState::ProvidingGuidance && i.autopilot_airspeed_managed;
        change_speed_state(&mut self.speed_state, if show { SpeedState::TargetAndMargins } else { SpeedState::TargetOnly }, &mut self.out);
    }

    fn update_over_underspeed_condition(&mut self, p: &ComputationParameters, i: &GuidanceInputs) {
        let airspeed = i.current_airspeed;
        let mut upper_limit = self.speed_target;
        let mut lower_limit = self.speed_target - 5.0;
        if self.speed_state == SpeedState::TargetAndMargins {
            let (lower, upper) = self.speed_margin.get_margins(self.speed_target, p, &i.speeds);
            lower_limit = lower;
            upper_limit = upper;
        }
        if self.is_in_overspeed_condition && airspeed < upper_limit {
            self.is_in_overspeed_condition = false;
        } else if !self.is_in_overspeed_condition && airspeed > upper_limit + 5.0 {
            self.is_in_overspeed_condition = true;
            self.is_in_underspeed_condition = false;
        }
        if !self.is_in_underspeed_condition && airspeed < lower_limit {
            self.is_in_underspeed_condition = true;
            self.is_in_overspeed_condition = false;
        } else if self.is_in_underspeed_condition && airspeed > lower_limit + 5.0 {
            self.is_in_underspeed_condition = false;
        }
    }

    pub fn des_submode(&self) -> RequestedVerticalMode {
        self.requested_vertical_mode
    }

    pub fn target_vertical_speed(&self) -> f64 {
        self.target_vertical_speed
    }
}

pub struct LatchedDescentGuidance {
    vertical_state: VerticalState,
    speed_state: SpeedState,
    requested_vertical_mode: RequestedVerticalMode,
    target_altitude: f64,
    target_altitude_guidance: f64,
    target_vertical_speed: f64,
    show_linear_deviation_on_pfd: bool,
    show_descent_latch_on_pfd: bool,
    speed_margin: SpeedMargin,
    tod_guidance: TodGuidance,
    speed_target: f64,
    is_in_overspeed_condition: bool,
    pub out: GuidanceOutputs,
}

impl LatchedDescentGuidance {
    pub fn new(config: &AircraftConfig) -> Self {
        let mut g = Self {
            vertical_state: VerticalState::InvalidProfile,
            speed_state: SpeedState::NotInDescentPhase,
            requested_vertical_mode: RequestedVerticalMode::None,
            target_altitude: 0.0,
            target_altitude_guidance: 0.0,
            target_vertical_speed: 0.0,
            show_linear_deviation_on_pfd: false,
            show_descent_latch_on_pfd: false,
            speed_margin: SpeedMargin::new(config),
            tod_guidance: TodGuidance::default(),
            speed_target: f64::NAN,
            is_in_overspeed_condition: false,
            out: GuidanceOutputs::default(),
        };
        g.write_to_sim_vars();
        g
    }

    pub fn update_profile(&mut self, rel: &mut AircraftToDescentProfileRelation, profile: Option<&Profile>) {
        rel.update_profile(profile);
        if !rel.is_valid {
            self.change_state(VerticalState::InvalidProfile);
        }
    }

    fn change_state(&mut self, new_state: VerticalState) {
        if self.vertical_state == new_state {
            return;
        }
        if self.vertical_state != VerticalState::InvalidProfile && new_state == VerticalState::InvalidProfile {
            self.reset();
            self.write_to_sim_vars();
        }
        self.vertical_state = new_state;
    }

    pub fn reset(&mut self) {
        self.requested_vertical_mode = RequestedVerticalMode::None;
        self.target_altitude = 0.0;
        self.target_vertical_speed = 0.0;
        self.show_linear_deviation_on_pfd = false;
        self.show_descent_latch_on_pfd = false;
        self.is_in_overspeed_condition = false;
    }

    pub fn update(
        &mut self,
        dt_ms: f64,
        distance_to_end: f64,
        rel: &mut AircraftToDescentProfileRelation,
        profile: Option<&Profile>,
        p: &ComputationParameters,
        i: &GuidanceInputs,
    ) {
        rel.update(distance_to_end);
        let profile = match profile {
            Some(profile) if rel.is_valid => profile,
            _ => {
                self.change_state(VerticalState::InvalidProfile);
                return;
            }
        };
        if (i.modes.vertical == vertical::DES) != (self.vertical_state == VerticalState::ProvidingGuidance) {
            self.change_state(if self.vertical_state == VerticalState::ProvidingGuidance { VerticalState::Observing } else { VerticalState::ProvidingGuidance });
        }
        self.update_speed_margin_state(p, i);
        self.update_speed_target(p, i);
        self.update_speed_guidance(p, i);
        self.update_overspeed_condition(p, i);
        self.target_altitude = rel.current_target_altitude(profile);
        self.show_linear_deviation_on_pfd = p.flight_phase >= FlightPhase::Descent || rel.is_past_top_of_descent();
        if self.vertical_state == VerticalState::ProvidingGuidance {
            self.update_des_mode_guidance(rel, profile, p, i);
        }
        self.write_to_sim_vars();
        self.tod_guidance.update(dt_ms, rel, p, i, &mut self.out);
    }

    fn update_des_mode_guidance(&mut self, rel: &AircraftToDescentProfileRelation, profile: &Profile, p: &ComputationParameters, i: &GuidanceInputs) {
        let above_speed_limit_altitude = rel.is_above_speed_limit_altitude(p);
        let before_top_of_descent = !rel.is_past_top_of_descent();
        let linear_deviation = rel.compute_linear_deviation(profile, p.present_altitude);
        let close_to_airfield_elevation = rel.is_close_to_airfield_elevation(p);
        let hold_active = i.manual_hold_active;

        self.target_altitude_guidance = i.current_pressure_altitude - linear_deviation;
        if (!hold_active && linear_deviation > 200.0) || self.is_in_overspeed_condition {
            self.requested_vertical_mode = RequestedVerticalMode::SpeedThrust;
            self.show_descent_latch_on_pfd = false;
        } else if before_top_of_descent || linear_deviation < -200.0 || hold_active {
            self.requested_vertical_mode = RequestedVerticalMode::VsSpeed;
            if hold_active {
                self.target_vertical_speed = -1000.0;
            } else {
                self.target_vertical_speed = if above_speed_limit_altitude && !close_to_airfield_elevation { -1000.0 } else { -500.0 };
                self.show_descent_latch_on_pfd = false;
            }
        } else {
            self.requested_vertical_mode = RequestedVerticalMode::VpathSpeed;
            self.target_vertical_speed = rel.current_target_vertical_speed(profile, i.ground_speed);
            self.show_descent_latch_on_pfd = true;
        }
    }

    fn update_speed_target(&mut self, p: &ComputationParameters, i: &GuidanceInputs) {
        self.speed_target = if i.autopilot_airspeed_managed {
            js_round(ias_or_mach(i.speeds_managed_pfd, p.managed_descent_speed_mach, &i.speeds))
        } else {
            p.fcu_speed
        };
    }

    fn write_to_sim_vars(&mut self) {
        self.out.fg_requested_vertical_mode = self.requested_vertical_mode;
        self.out.fg_target_altitude = self.target_altitude_guidance;
        self.out.fg_target_vertical_speed = self.target_vertical_speed;
        self.out.pfd_target_altitude = self.target_altitude;
        self.out.pfd_linear_deviation_active = self.show_linear_deviation_on_pfd;
        self.out.pfd_vertical_profile_latched = self.show_descent_latch_on_pfd;
    }

    fn update_speed_guidance(&mut self, p: &ComputationParameters, i: &GuidanceInputs) {
        if self.speed_state == SpeedState::NotInDescentPhase {
            return;
        }
        self.out.speeds_managed_pfd = Some(self.speed_target);
        let margins = self.speed_margin.get_margins(self.speed_target, p, &i.speeds);
        if self.speed_state == SpeedState::TargetAndMargins {
            self.out.speeds_managed_athr = Some(margins.1);
            self.out.pfd_lower_speed_margin = margins.0;
            self.out.pfd_upper_speed_margin = margins.1;
        } else {
            self.out.speeds_managed_athr = Some(self.speed_target);
        }
    }

    fn update_speed_margin_state(&mut self, p: &ComputationParameters, i: &GuidanceInputs) {
        if p.flight_phase != FlightPhase::Descent {
            change_speed_state(&mut self.speed_state, SpeedState::NotInDescentPhase, &mut self.out);
            return;
        }
        let show = !i.manual_hold_active
            && self.vertical_state == VerticalState::ProvidingGuidance
            && i.autopilot_airspeed_managed
            && !self.show_descent_latch_on_pfd;
        change_speed_state(&mut self.speed_state, if show { SpeedState::TargetAndMargins } else { SpeedState::TargetOnly }, &mut self.out);
    }

    fn update_overspeed_condition(&mut self, p: &ComputationParameters, i: &GuidanceInputs) {
        let airspeed = i.current_airspeed;
        let mut resolution = self.speed_target;
        if self.speed_state == SpeedState::TargetAndMargins {
            resolution = self.speed_margin.get_margins(self.speed_target, p, &i.speeds).1;
        }
        let trigger = ias_or_mach(345.0, 0.81, &i.speeds);
        if self.is_in_overspeed_condition && airspeed < resolution {
            self.is_in_overspeed_condition = false;
        }
        if !self.is_in_overspeed_condition && airspeed > trigger {
            self.is_in_overspeed_condition = true;
        }
    }

    pub fn des_submode(&self) -> RequestedVerticalMode {
        self.requested_vertical_mode
    }

    pub fn target_vertical_speed(&self) -> f64 {
        self.target_vertical_speed
    }
}

pub enum AnyDescentGuidance {
    Normal(DescentGuidance),
    Latched(LatchedDescentGuidance),
}

impl AnyDescentGuidance {
    pub fn new(config: &AircraftConfig, latched: bool) -> Self {
        if latched {
            Self::Latched(LatchedDescentGuidance::new(config))
        } else {
            Self::Normal(DescentGuidance::new(config))
        }
    }

    pub fn update_profile(&mut self, rel: &mut AircraftToDescentProfileRelation, profile: Option<&Profile>) {
        match self {
            Self::Normal(g) => g.update_profile(rel, profile),
            Self::Latched(g) => g.update_profile(rel, profile),
        }
    }

    pub fn update(&mut self, dt_ms: f64, distance_to_end: f64, rel: &mut AircraftToDescentProfileRelation, profile: Option<&Profile>, p: &ComputationParameters, i: &GuidanceInputs) {
        match self {
            Self::Normal(g) => g.update(dt_ms, distance_to_end, rel, profile, p, i),
            Self::Latched(g) => g.update(dt_ms, distance_to_end, rel, profile, p, i),
        }
    }

    pub fn reset(&mut self) {
        match self {
            Self::Normal(g) => g.reset(),
            Self::Latched(g) => g.reset(),
        }
    }

    pub fn out(&self) -> &GuidanceOutputs {
        match self {
            Self::Normal(g) => &g.out,
            Self::Latched(g) => &g.out,
        }
    }

    pub fn out_mut(&mut self) -> &mut GuidanceOutputs {
        match self {
            Self::Normal(g) => &mut g.out,
            Self::Latched(g) => &mut g.out,
        }
    }

    pub fn des_submode(&self) -> RequestedVerticalMode {
        match self {
            Self::Normal(g) => g.des_submode(),
            Self::Latched(g) => g.des_submode(),
        }
    }
}

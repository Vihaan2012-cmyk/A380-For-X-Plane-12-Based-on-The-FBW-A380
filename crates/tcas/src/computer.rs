use crate::consts::*;
use crate::util::*;

#[derive(Clone, Debug)]
pub struct RawTraffic {
    pub uid: f64,
    pub lat: f64,
    pub lon: f64,
    pub alt: f64,
    pub heading: f64,
}

#[derive(Clone, Debug)]
pub struct Inputs {
    pub lat: f64,
    pub lon: f64,
    pub alt_ft: f64,
    pub vertical_speed: f64,
    pub alt_above_ground: f64,
    pub ac_ess_powered: bool,
    pub ac_2_powered: bool,
    pub xpdr_system: f64,
    pub transponder_state: [f64; 3],
    pub air_data_knob: f64,
    pub adr_altitude: [f64; 3],
    pub radio_alt: [f64; 2],
    pub att_hdg_knob: f64,
    pub ir_true_heading: [f64; 3],
    pub slew_active: bool,
    pub gpws_warning: bool,
    pub traffic_left_efis: bool,
    pub traffic_right_efis: bool,
    pub alert_level: u8,
    pub alt_select: u8,
}

impl Default for Inputs {
    fn default() -> Self {
        let nominal = |v: f32| Arinc429::to_raw(v, Ssm::NormalOperation);
        Self {
            lat: 0.0,
            lon: 0.0,
            alt_ft: 0.0,
            vertical_speed: 0.0,
            alt_above_ground: 0.0,
            ac_ess_powered: true,
            ac_2_powered: true,
            xpdr_system: 0.0,
            transponder_state: [4.0; 3],
            air_data_knob: 1.0,
            adr_altitude: [nominal(0.0); 3],
            radio_alt: [nominal(0.0); 2],
            att_hdg_knob: 1.0,
            ir_true_heading: [nominal(0.0); 3],
            slew_active: false,
            gpws_warning: false,
            traffic_left_efis: false,
            traffic_right_efis: false,
            alert_level: MODE_TARA,
            alt_select: 0,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Vars {
    pub mode: u8,
    pub state: u8,
    pub fault: bool,
    pub ta_only: bool,
    pub ra_corrective: bool,
    pub ra_type: u8,
    pub rate_to_maintain: f64,
    pub up_advisory: u8,
    pub down_advisory: u8,
    pub sensitivity: u8,
    pub vspeed_red: [f64; 2],
    pub vspeed_green: [f64; 2],
}

#[derive(Clone, Debug, PartialEq)]
pub struct NdTraffic {
    pub id: String,
    pub lat: f64,
    pub lon: f64,
    pub relative_alt: f64,
    pub intrusion_level: u8,
    pub vert_speed: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SoundCall {
    pub name: &'static str,
    pub repeat: Option<bool>,
}

#[derive(Clone, Debug)]
pub struct TrackDiag {
    pub id: String,
    pub alive: bool,
    pub intrusion_level: u8,
    pub ta_tau: f64,
    pub ra_tau: f64,
    pub v_tau: f64,
    pub closure_rate: f64,
    pub slant_distance: f64,
    pub vert_speed: f64,
    pub ground_speed: f64,
    pub is_displayed: bool,
}

#[derive(Clone, Debug)]
pub struct Update {
    pub vars: Vars,
    pub traffic_l: Option<Vec<NdTraffic>>,
    pub traffic_r: Option<Vec<NdTraffic>>,
    pub sounds: Vec<SoundCall>,
    pub tracks: Vec<TrackDiag>,
}

#[derive(Clone, Debug)]
struct Traffic {
    alive: bool,
    id: String,
    lat: f64,
    lon: f64,
    alt: f64,
    vert_speed: f64,
    ground_speed: f64,
    relative_alt: f64,
    slant_distance: f64,
    hrz_distance: f64,
    closure_rate: f64,
    closure_accel: f64,
    intrusion: u8,
    is_displayed: bool,
    ta_tau: f64,
    ra_tau: f64,
    v_tau: f64,
    seconds_since_last_ta: f64,
}

#[derive(Clone, Debug)]
struct ResAdvisory {
    info: Option<&'static RaParams>,
    is_reversal: bool,
    seconds_since_start: f64,
    has_been_announced: bool,
}

impl ResAdvisory {
    fn empty() -> Self {
        Self { info: None, is_reversal: false, seconds_since_start: 0.0, has_been_announced: false }
    }
}

struct Sensed {
    ppos_lat: f64,
    ppos_lon: f64,
    plane_alt: f64,
    vertical_speed: f64,
    pressure_alt: Arinc429,
    radio_alt: Arinc429,
    ir_true_heading: Arinc429,
    tcas_power: bool,
    xpdr_state: f64,
}

pub struct Tcas {
    vars: Vars,
    air_traffic: Vec<Traffic>,
    holes: usize,
    ra_traffic: Vec<usize>,
    send_air_traffic: Vec<NdTraffic>,
    inhibitions: Inhibit,
    advisory_state: TcasState,
    active_ra: ResAdvisory,
    new_ra: ResAdvisory,
    s: Sensed,
    inputs: Inputs,
    sounds: Vec<SoundCall>,
    traf_l: Option<Vec<NdTraffic>>,
    traf_r: Option<Vec<NdTraffic>>,
    ta_only: Vec<String>,
}

fn clamp_sens(s: u8) -> usize {
    (s as usize).clamp(1, 8) - 1
}

impl Default for Tcas {
    fn default() -> Self {
        Self::new()
    }
}

impl Tcas {
    pub fn new() -> Self {
        let unused = Arinc429::from_raw(0.0);
        Self {
            vars: Vars {
                mode: 0,
                state: 0,
                fault: false,
                ta_only: false,
                ra_corrective: false,
                ra_type: 0,
                rate_to_maintain: 0.0,
                up_advisory: 0,
                down_advisory: 0,
                sensitivity: 1,
                vspeed_red: [0.0; 2],
                vspeed_green: [0.0; 2],
            },
            air_traffic: Vec::new(),
            holes: 0,
            ra_traffic: Vec::new(),
            send_air_traffic: Vec::new(),
            inhibitions: Inhibit::None,
            advisory_state: TcasState::None,
            active_ra: ResAdvisory::empty(),
            new_ra: ResAdvisory::empty(),
            s: Sensed {
                ppos_lat: f64::NAN,
                ppos_lon: f64::NAN,
                plane_alt: 0.0,
                vertical_speed: 0.0,
                pressure_alt: unused,
                radio_alt: unused,
                ir_true_heading: unused,
                tcas_power: false,
                xpdr_state: 0.0,
            },
            inputs: Inputs::default(),
            sounds: Vec::new(),
            traf_l: None,
            traf_r: None,
            ta_only: Vec::new(),
        }
    }

    pub fn set_ta_only(&mut self, uids: &[f64]) {
        self.ta_only = uids.iter().map(|u| format!("{u:.0}")).collect();
    }

    pub fn vars(&self) -> &Vars {
        &self.vars
    }

    fn sens(&self) -> usize {
        clamp_sens(self.vars.sensitivity)
    }

    fn play(&mut self, name: &'static str, repeat: Option<bool>) {
        self.sounds.push(SoundCall { name, repeat });
    }

    pub fn update(&mut self, dt_ms: f64, inputs: &Inputs, air_traffic: &[RawTraffic]) -> Update {
        self.sounds.clear();
        self.traf_l = None;
        self.traf_r = None;
        self.inputs = inputs.clone();
        self.update_vars();
        self.update_inhibitions();
        self.update_status_faults();
        if self.vars.mode == MODE_STBY {
            self.advisory_state = TcasState::None;
            self.vars.state = 0;
            self.vars.ra_corrective = false;
            self.vars.ra_type = RaType2::None as u8;
            self.vars.rate_to_maintain = 0.0;
            self.vars.up_advisory = UpDown::NoAdvisory as u8;
            self.vars.down_advisory = UpDown::NoAdvisory as u8;
            self.vars.vspeed_red = [0.0; 2];
            self.vars.vspeed_green = [0.0; 2];
            if !self.send_air_traffic.is_empty() {
                self.reset_display();
            }
            return self.finish();
        }
        if !self.vars.fault {
            self.update_traffic(dt_ms);
            self.update_ra(dt_ms);
            self.emit_display();
            self.fetch_raw_traffic(dt_ms, air_traffic);
        }
        self.finish()
    }

    fn finish(&mut self) -> Update {
        Update {
            vars: self.vars.clone(),
            traffic_l: self.traf_l.take(),
            traffic_r: self.traf_r.take(),
            sounds: std::mem::take(&mut self.sounds),
            tracks: self
                .air_traffic
                .iter()
                .map(|t| TrackDiag {
                    id: t.id.clone(),
                    alive: t.alive,
                    intrusion_level: t.intrusion,
                    ta_tau: t.ta_tau,
                    ra_tau: t.ra_tau,
                    v_tau: t.v_tau,
                    closure_rate: t.closure_rate,
                    slant_distance: t.slant_distance,
                    vert_speed: t.vert_speed,
                    ground_speed: t.ground_speed,
                    is_displayed: t.is_displayed,
                })
                .collect(),
        }
    }

    fn update_vars(&mut self) {
        let i = &self.inputs;
        self.s.vertical_speed = i.vertical_speed;
        self.s.ppos_lat = i.lat;
        self.s.ppos_lon = i.lon;
        self.s.plane_alt = i.alt_ft;
        self.s.tcas_power = i.ac_ess_powered || i.ac_2_powered;
        let xpdr = i.xpdr_system;
        let at = |a: &[f64], k: f64| -> f64 {
            if k >= 0.0 && (k as usize) < a.len() && k.fract() == 0.0 {
                a[k as usize]
            } else {
                0.0
            }
        };
        self.s.xpdr_state = at(&i.transponder_state, xpdr);

        let alternate_air_data = i.air_data_knob == if xpdr == 0.0 { 0.0 } else { 2.0 };
        self.s.pressure_alt = Arinc429::from_raw(if alternate_air_data {
            i.adr_altitude[2]
        } else {
            at(&i.adr_altitude, xpdr)
        });

        let ra1 = Arinc429::from_raw(i.radio_alt[0]);
        let ra2 = Arinc429::from_raw(i.radio_alt[1]);
        self.s.radio_alt = if (ra1.is_failure_warning() || ra1.is_no_computed_data())
            && !(!ra1.is_no_computed_data() && ra2.is_failure_warning())
        {
            ra2
        } else {
            ra1
        };

        let ir_to_use = if i.att_hdg_knob == 1.0 { xpdr + 1.0 } else { 3.0 };
        self.s.ir_true_heading = Arinc429::from_raw(at(&i.ir_true_heading, ir_to_use - 1.0));

        self.vars.mode = if self.s.xpdr_state == XPDR_STBY || !self.s.tcas_power || self.vars.fault {
            MODE_STBY
        } else {
            self.inputs.alert_level
        };
    }

    fn update_inhibitions(&mut self) {
        let ra = self.s.radio_alt;
        let mode = self.vars.mode;
        self.inhibitions = if ra.is_failure_warning()
            || (!ra.is_no_computed_data() && ra.value < 500.0)
            || self.inputs.gpws_warning
            || mode == MODE_STBY
        {
            Inhibit::AllRaAuralTa
        } else if (!ra.is_no_computed_data() && ra.value < 1000.0) || mode == MODE_TA {
            Inhibit::AllRa
        } else if !ra.is_no_computed_data() && ra.value < 1100.0 {
            Inhibit::AllDescRa
        } else if !ra.is_no_computed_data() && ra.value < 1550.0 {
            Inhibit::AllIncrDescRa
        } else if self.s.pressure_alt.value > 39000.0 {
            Inhibit::AllClimbRa
        } else {
            Inhibit::None
        };
    }

    fn update_status_faults(&mut self) {
        let s = &self.s;
        if !s.ir_true_heading.is_normal_operation()
            || s.ir_true_heading.is_no_computed_data()
            || (!s.pressure_alt.is_normal_operation() && !s.pressure_alt.is_functional_test())
            || s.radio_alt.is_failure_warning()
            || !s.tcas_power
        {
            self.reset_display();
            self.vars.fault = true;
            return;
        }
        self.vars.fault = false;

        if self.vars.mode == MODE_STBY {
            self.vars.ta_only = false;
            self.vars.fault = false;
            self.vars.sensitivity = 1;
            self.active_ra = ResAdvisory::empty();
            self.new_ra = ResAdvisory::empty();
            return;
        }

        let ta_only = self.inhibitions == Inhibit::AllRa || self.inhibitions == Inhibit::AllRaAuralTa;
        self.vars.ta_only = ta_only;

        if self.active_ra.info.is_none() {
            let ra = self.s.radio_alt;
            let pa = self.s.pressure_alt.value;
            let in_band = |k: usize| pa > SENSE[k - 3][0] && pa <= SENSE[k - 3][1];
            self.vars.sensitivity = if ta_only {
                2
            } else if !ra.is_failure_warning()
                && !ra.is_no_computed_data()
                && ra.value > SENSE[0][0]
                && ra.value <= SENSE[0][1]
            {
                3
            } else if in_band(4) {
                4
            } else if in_band(5) {
                5
            } else if in_band(6) {
                6
            } else if in_band(7) {
                7
            } else {
                8
            };
        }
    }

    fn fetch_raw_traffic(&mut self, dt_ms: f64, raw: &[RawTraffic]) {
        let ppos_lat = self.s.ppos_lat;
        let ppos_lon = self.s.ppos_lon;
        let plane_alt = self.s.plane_alt;
        let vs = self.s.vertical_speed;
        let sens = self.sens();
        for t in self.air_traffic.iter_mut() {
            t.alive = false;
        }
        for tf in raw {
            if falsy(tf.lat) && falsy(tf.lon) && falsy(tf.alt) && falsy(tf.heading) {
                continue;
            }
            let id = format!("{:.0}", tf.uid);
            let pos = self.air_traffic.iter().position(|p| p.id == id);
            let idx = match pos {
                Some(p) => p,
                None => {
                    self.air_traffic.push(Traffic {
                        alive: true,
                        id,
                        lat: tf.lat,
                        lon: tf.lon,
                        alt: tf.alt * 3.281,
                        vert_speed: 0.0,
                        ground_speed: 0.0,
                        relative_alt: tf.alt * 3.281 - plane_alt,
                        slant_distance: distance_3d(tf.lat, tf.lon, tf.alt * 3.281, ppos_lat, ppos_lon, plane_alt),
                        hrz_distance: great_circle_distance(ppos_lat, ppos_lon, tf.lat, tf.lon),
                        closure_rate: 0.0,
                        closure_accel: f64::NAN,
                        intrusion: INTR_TRAFFIC,
                        is_displayed: false,
                        ta_tau: f64::INFINITY,
                        ra_tau: f64::INFINITY,
                        v_tau: f64::INFINITY,
                        seconds_since_last_ta: 0.0,
                    });
                    self.air_traffic.len() - 1
                }
            };
            let t = &mut self.air_traffic[idx];
            t.alive = true;
            let new_alt = tf.alt * 3.281;
            let dt_s = dt_ms / 1000.0;
            t.vert_speed = ((new_alt - t.alt) / dt_s) * 60.0;
            t.ground_speed = ((great_circle_distance(tf.lat, tf.lon, t.lat, t.lon) / dt_s) * 3600.0).abs();
            let new_slant = distance_3d(t.lat, t.lon, t.alt, ppos_lat, ppos_lon, plane_alt);
            let new_closure_rate = ((t.slant_distance - new_slant) / dt_s) * 3600.0;
            t.closure_accel = (new_closure_rate - t.closure_rate) / dt_s;
            t.closure_rate = new_closure_rate;
            t.slant_distance = new_slant;
            t.lat = tf.lat;
            t.lon = tf.lon;
            t.alt = tf.alt * 3.281;
            t.relative_alt = new_alt - plane_alt;

            let mut ta_tau = ((t.slant_distance - DMOD[sens][TA].powi(2) / t.slant_distance) / t.closure_rate) * 3600.0;
            let mut ra_tau = ((t.slant_distance - DMOD[sens][RA].powi(2) / t.slant_distance) / t.closure_rate) * 3600.0;
            let mut v_tau = (t.relative_alt / (vs - t.vert_speed)) * 60.0;
            if ra_tau < 0.0 {
                ta_tau = f64::INFINITY;
                ra_tau = f64::INFINITY;
            }
            if v_tau < 0.0 {
                v_tau = f64::INFINITY;
            }
            t.ta_tau = ta_tau;
            t.ra_tau = ra_tau;
            t.v_tau = v_tau;
        }

        if self.air_traffic.len() + self.holes > MEMORY_MAX {
            let mut kept: Vec<Traffic> = self.air_traffic.iter().filter(|t| t.alive).cloned().collect();
            js_sort(&mut kept, |a, b| a.ra_tau - b.ra_tau);
            kept.truncate(MEMORY_MAX);
            self.holes = MEMORY_MAX - kept.len();
            self.air_traffic = kept;
        }
    }

    fn update_traffic(&mut self, dt_ms: f64) {
        let sens = self.sens();
        let vs = self.s.vertical_speed;
        let ground_alt = self.s.plane_alt - self.inputs.alt_above_ground;
        let alt_select = (self.inputs.alt_select as usize).min(2);
        let slew = self.inputs.slew_active;
        let ppos_lat = self.s.ppos_lat;
        let ppos_lon = self.s.ppos_lon;
        let heading = self.s.ir_true_heading.value;
        let active_ra_none = self.active_ra.info.is_none();
        let active_secs = self.active_ra.seconds_since_start;

        for t in self.air_traffic.iter_mut() {
            if t.vert_speed.abs() >= 6000.0 || t.ground_speed >= 600.0 {
                t.seconds_since_last_ta = 0.0;
                t.intrusion = INTR_TRAFFIC;
                continue;
            }

            let on_ground = t.alt < ground_alt + 360.0 || t.ground_speed < 30.0;
            let mut is_displayed = false;
            if !on_ground
                && t.ground_speed >= 30.0
                && t.relative_alt >= THREAT[alt_select][0]
                && t.relative_alt <= THREAT[alt_select][1]
            {
                is_displayed = true;
            }

            if is_displayed {
                let bearing = great_circle_heading(ppos_lat, ppos_lon, t.lat, t.lon) - heading + 90.0;
                let x = t.hrz_distance * (bearing * std::f64::consts::PI / 180.0).cos();
                let y = t.hrz_distance * (bearing * std::f64::consts::PI / 180.0).sin();
                if !point_in_ellipse(x, y, RANGE_SIDE, RANGE_FORWARD_MIN, RANGE_SIDE, RANGE_BACK)
                    || t.relative_alt.abs() > RANGE_ALT
                {
                    is_displayed = false;
                    t.ta_tau = f64::INFINITY;
                    t.ra_tau = f64::INFINITY;
                }
            }
            t.is_displayed = is_displayed;

            let range_test = if t.ra_tau < TAU[sens][RA] || t.slant_distance < DMOD[sens][RA] {
                INTR_RA
            } else if t.ta_tau < TAU[sens][TA] || t.slant_distance < DMOD[sens][TA] {
                INTR_TA
            } else if t.hrz_distance < 6.0 {
                INTR_PROXIMITY
            } else {
                0
            };

            let v_thr = if vs.abs() <= 600.0 { TVTHR[sens] } else { TAU[sens][RA] };
            let alt_test = if t.v_tau < v_thr || t.relative_alt.abs() < ZTHR[sens][RA] {
                INTR_RA
            } else if t.v_tau < TAU[sens][TA] || t.relative_alt.abs() < ZTHR[sens][TA] {
                INTR_TA
            } else if t.relative_alt.abs() < 1200.0 {
                INTR_PROXIMITY
            } else {
                0
            };

            let accel_test = if t.closure_accel.abs() <= ACCEL[sens][RA] {
                INTR_RA
            } else if t.closure_accel.abs() <= ACCEL[sens][TA] {
                INTR_TA
            } else {
                INTR_PROXIMITY
            };

            let desired = range_test.min(alt_test).min(accel_test);
            match t.intrusion {
                INTR_RA => {
                    if active_ra_none
                        || active_secs < MIN_RA_DURATION
                        || (t.ta_tau > TAU[sens][TA] * VOL_BOOST && t.slant_distance > DMOD[sens][TA] * VOL_BOOST)
                        || (t.v_tau > TAU[sens][TA] * VOL_BOOST && t.relative_alt.abs() > ZTHR[sens][TA] * VOL_BOOST)
                        || t.closure_rate < CLOSURE_RATE_THRESH
                    {
                        t.intrusion = desired;
                    }
                }
                INTR_TA => match desired {
                    INTR_RA => {
                        if !slew {
                            t.intrusion = desired;
                        }
                    }
                    INTR_TA => {
                        t.seconds_since_last_ta = 0.0;
                    }
                    _ => {
                        if t.seconds_since_last_ta >= TA_EXPIRATION_DELAY {
                            t.seconds_since_last_ta = 0.0;
                            t.intrusion = desired;
                        } else if t.seconds_since_last_ta < 10.0 {
                            t.seconds_since_last_ta += dt_ms / 1000.0;
                        }
                    }
                },
                _ => {
                    if !slew {
                        t.intrusion = desired;
                    }
                }
            }
            if t.intrusion == INTR_RA && self.ta_only.contains(&t.id) {
                t.intrusion = INTR_TA;
                t.seconds_since_last_ta = 0.0;
            }
        }
    }

    fn update_ra(&mut self, dt_ms: f64) {
        self.get_ra();
        self.update_advisory_state(dt_ms);
    }

    fn calculate_trajectory(&self, target_vs: f64, ra_tau: f64, delay: f64, accel: f64) -> f64 {
        let vs = self.s.vertical_speed;
        let accel = if target_vs < vs { -1.0 * accel } else { accel };
        let time_to_accelerate = js_min(ra_tau - delay, (target_vs - vs) / 60.0 / accel);
        let remaining_time = ra_tau - (delay + time_to_accelerate);
        self.s.plane_alt
            + js_round(vs / 60.0) * (delay + time_to_accelerate)
            + 0.5 * accel * time_to_accelerate.powi(2)
            + (target_vs / 60.0) * remaining_time
    }

    fn get_predicted_sep(&self) -> f64 {
        let mut min_separation = REALLY_BIG_NUMBER;
        for &i in &self.ra_traffic {
            let t = &self.air_traffic[i];
            let traffic_alt_at_cpa = t.alt + (t.vert_speed / 60.0) * t.ra_tau;
            let my_alt_at_cpa = self.s.plane_alt + (self.s.vertical_speed / 60.0) * t.ra_tau;
            let sep = (my_alt_at_cpa - traffic_alt_at_cpa).abs();
            if sep < min_separation {
                min_separation = sep;
            }
        }
        min_separation
    }

    fn get_vertical_sep(&self, sense: RaSense, target_vs: f64, delay: f64, accel: f64) -> (f64, bool) {
        let mut is_crossing = false;
        let mut min_separation = REALLY_BIG_NUMBER;
        let vs = self.s.vertical_speed;
        let plane_alt = self.s.plane_alt;
        for &i in &self.ra_traffic {
            let t = &self.air_traffic[i];
            let traffic_alt_at_cpa = t.alt + (t.vert_speed / 60.0) * t.ra_tau;
            let sep;
            match sense {
                RaSense::Up => {
                    let d = if vs < target_vs { js_min(t.ra_tau, delay) } else { 0.0 };
                    sep = js_max(self.calculate_trajectory(target_vs, t.ra_tau, d, accel) - traffic_alt_at_cpa, 0.0);
                    if !is_crossing && plane_alt + 100.0 < t.alt {
                        is_crossing = true;
                    }
                }
                RaSense::Down => {
                    let d = if vs > target_vs { js_min(t.ra_tau, delay) } else { 0.0 };
                    sep = js_max(traffic_alt_at_cpa - self.calculate_trajectory(target_vs, t.ra_tau, d, accel), 0.0);
                    if !is_crossing && plane_alt - 100.0 > t.alt {
                        is_crossing = true;
                    }
                }
            }
            if sep < min_separation {
                min_separation = sep;
            }
        }
        (min_separation, is_crossing)
    }

    #[allow(unused_assignments)]
    fn get_ra(&mut self) {
        if self.vars.fault {
            self.new_ra.info = None;
            self.active_ra.info = None;
            return;
        }

        let mut idx: Vec<usize> = (0..self.air_traffic.len())
            .filter(|&i| self.air_traffic[i].intrusion == INTR_RA && self.air_traffic[i].alive)
            .collect();
        {
            let tr = &self.air_traffic;
            js_sort(&mut idx, |&a, &b| tr[a].ra_tau - tr[b].ra_tau);
        }
        self.ra_traffic = idx;
        self.new_ra = ResAdvisory::empty();
        let previous = self.active_ra.clone();
        let sens = self.sens();
        let alim = ALIM[sens];
        let vs = self.s.vertical_speed;

        if self.ra_traffic.is_empty() {
            return;
        }

        if previous.info.is_none() {
            let (up_sep, up_cross) = self.get_vertical_sep(RaSense::Up, 1500.0, INITIAL_DELAY, INITIAL_ACCEL);
            let (down_sep, down_cross) = self.get_vertical_sep(RaSense::Down, -1500.0, INITIAL_DELAY, INITIAL_ACCEL);

            let mut sense;
            if self.inhibitions == Inhibit::AllDescRa {
                sense = RaSense::Up;
            } else if self.inhibitions == Inhibit::AllClimbRa {
                sense = RaSense::Down;
            } else {
                sense = RaSense::Up;
                if up_sep >= alim && down_sep >= alim {
                    if up_cross && !down_cross {
                        sense = RaSense::Down;
                    } else if !up_cross && down_cross {
                        sense = RaSense::Up;
                    } else {
                        sense = if up_sep > down_sep { RaSense::Up } else { RaSense::Down };
                    }
                }
                if up_sep < alim && down_sep < alim {
                    sense = if up_sep > down_sep { RaSense::Up } else { RaSense::Down };
                }
                if up_sep >= alim && down_sep < alim {
                    sense = RaSense::Up;
                } else {
                    sense = RaSense::Down;
                }
            }

            let (level_sep, _) = self.get_vertical_sep(sense, 0.0, INITIAL_DELAY, INITIAL_ACCEL);
            let up = sense == RaSense::Up;
            if vs.abs() < 1500.0 || (vs <= -1500.0 && up) || (vs >= 1500.0 && !up) {
                let predicted_sep = self.get_predicted_sep();
                if predicted_sep >= alim {
                    let mul = if up { -1.0 } else { 1.0 };
                    let (sep500, _) = self.get_vertical_sep(sense, mul * 500.0, INITIAL_DELAY, INITIAL_ACCEL);
                    let (sep1000, _) = self.get_vertical_sep(sense, mul * 1000.0, INITIAL_DELAY, INITIAL_ACCEL);
                    let (sep2000, _) = self.get_vertical_sep(sense, mul * 2000.0, INITIAL_DELAY, INITIAL_ACCEL);
                    self.new_ra.info = Some(if sep2000 >= alim {
                        if up {
                            &MONITOR_VS_CLIMB_2000
                        } else {
                            &MONITOR_VS_DESCEND_2000
                        }
                    } else if sep1000 >= alim {
                        if up {
                            &MONITOR_VS_CLIMB_1000
                        } else {
                            &MONITOR_VS_DESCEND_1000
                        }
                    } else if sep500 >= alim {
                        if up {
                            &MONITOR_VS_CLIMB_500
                        } else {
                            &MONITOR_VS_DESCEND_500
                        }
                    } else if level_sep >= alim {
                        if up {
                            &MONITOR_VS_CLIMB_0
                        } else {
                            &MONITOR_VS_DESCEND_0
                        }
                    } else if up {
                        if up_cross {
                            &CLIMB_CROSS
                        } else {
                            &CLIMB
                        }
                    } else if down_cross {
                        &DESCEND_CROSS
                    } else {
                        &DESCEND
                    });
                } else {
                    let nominal_sep = if up { up_sep } else { down_sep };
                    self.new_ra.info = Some(if nominal_sep > level_sep {
                        if up {
                            if up_cross {
                                &CLIMB_CROSS
                            } else {
                                &CLIMB
                            }
                        } else if down_cross {
                            &DESCEND_CROSS
                        } else {
                            &DESCEND
                        }
                    } else if up {
                        &LEVEL_OFF_300_ABOVE
                    } else {
                        &LEVEL_OFF_300_BELOW
                    });
                }
            } else {
                let nominal_sep = if up { up_sep } else { down_sep };
                self.new_ra.info = Some(if nominal_sep > level_sep {
                    if up {
                        if up_cross {
                            &CLIMB_MAINTAIN_VS_CROSSING
                        } else {
                            &CLIMB_MAINTAIN_VS
                        }
                    } else if down_cross {
                        &DESCEND_MAINTAIN_VS_CROSSING
                    } else {
                        &DESCEND_MAINTAIN_VS
                    }
                } else if up {
                    &LEVEL_OFF_300_ABOVE
                } else {
                    &LEVEL_OFF_300_BELOW
                });
            }
        } else {
            let prev_info = previous.info.unwrap();
            let mut already_achieved_ta_zthr = true;
            let mut min_time_to_cpa = REALLY_BIG_NUMBER;
            for &i in &self.ra_traffic {
                let t = &self.air_traffic[i];
                if (self.s.plane_alt - t.alt).abs() < ZTHR[sens][TA] {
                    already_achieved_ta_zthr = false;
                }
                if t.ra_tau < min_time_to_cpa {
                    min_time_to_cpa = t.ra_tau;
                }
            }

            let sense = prev_info.sense;
            self.new_ra.is_reversal = previous.is_reversal;
            self.new_ra.seconds_since_start = previous.seconds_since_start;

            if already_achieved_ta_zthr {
                if previous.seconds_since_start >= 10.0
                    && prev_info.callout.id != CALLOUT_ID_LEVEL_OFF
                    && prev_info.callout.id != CALLOUT_ID_MONITOR_VS
                {
                    self.new_ra.info = Some(if prev_info.sense == RaSense::Up {
                        &LEVEL_OFF_300_ABOVE
                    } else {
                        &LEVEL_OFF_300_BELOW
                    });
                } else {
                    self.new_ra.info = previous.info;
                    self.new_ra.has_been_announced = true;
                }
            } else {
                let predicted_sep = self.get_predicted_sep();
                let mut strengthen: Option<&'static RaParams> = None;
                if predicted_sep < alim {
                    let up = sense == RaSense::Up;
                    let mul = if up { 1.0 } else { -1.0 };
                    let mut increase_sep = 0.0;
                    match prev_info.callout.id {
                        CALLOUT_ID_LEVEL_OFF | CALLOUT_ID_MONITOR_VS => {
                            let (s, cross) = self.get_vertical_sep(sense, mul * 1500.0, FOLLOWUP_DELAY, FOLLOWUP_ACCEL);
                            increase_sep = s;
                            strengthen = Some(if cross {
                                if up {
                                    &CLIMB_CROSS
                                } else {
                                    &DESCEND_CROSS
                                }
                            } else if up {
                                &CLIMB
                            } else {
                                &DESCEND
                            });
                        }
                        CALLOUT_ID_CLIMB
                        | CALLOUT_ID_CLIMB_CROSS
                        | CALLOUT_ID_CLIMB_NOW
                        | CALLOUT_ID_DESCEND
                        | CALLOUT_ID_DESCEND_NOW
                        | CALLOUT_ID_MAINTAIN_VS
                        | CALLOUT_ID_MAINTAIN_VS_CROSS => {
                            if (prev_info.sense == RaSense::Up && vs >= 1500.0)
                                || (prev_info.sense == RaSense::Down && vs <= -1500.0)
                            {
                                let (s, _) = self.get_vertical_sep(sense, mul * 2500.0, FOLLOWUP_DELAY, FOLLOWUP_ACCEL);
                                increase_sep = s;
                                if self.inhibitions != Inhibit::AllIncrDescRa {
                                    strengthen = Some(if up { &CLIMB_INCREASE } else { &DESCEND_INCREASE });
                                }
                            }
                        }
                        _ => {}
                    }

                    if previous.is_reversal || previous.seconds_since_start < 10.0 || min_time_to_cpa < 4.0 {
                        match strengthen {
                            None => {
                                self.new_ra.info = previous.info;
                                self.new_ra.has_been_announced = true;
                            }
                            Some(s) => self.new_ra.info = Some(s),
                        }
                    } else if self.inhibitions != Inhibit::AllClimbRa && self.inhibitions != Inhibit::AllDescRa {
                        let reversed = if up { RaSense::Down } else { RaSense::Up };
                        let rev_mul = if reversed == RaSense::Up { 1.0 } else { -1.0 };
                        let (reverse_sep, _) =
                            self.get_vertical_sep(reversed, rev_mul * 1500.0, FOLLOWUP_DELAY, FOLLOWUP_ACCEL);
                        let reverse_ra: &'static RaParams = if reversed == RaSense::Up { &CLIMB_NOW } else { &DESCEND_NOW };

                        if strengthen.is_none() {
                            if predicted_sep >= reverse_sep {
                                self.new_ra.info = previous.info;
                                self.new_ra.has_been_announced = true;
                                return;
                            }
                            self.new_ra.info = Some(reverse_ra);
                            self.new_ra.is_reversal = true;
                        }

                        if increase_sep >= alim && reverse_sep >= alim {
                            self.new_ra.info = strengthen;
                        }

                        if increase_sep < alim && reverse_sep < alim {
                            if increase_sep >= reverse_sep {
                                self.new_ra.info = strengthen;
                            } else {
                                self.new_ra.info = Some(reverse_ra);
                                self.new_ra.is_reversal = true;
                            }
                        }

                        if increase_sep >= alim && reverse_sep < alim {
                            self.new_ra.info = strengthen;
                        } else {
                            self.new_ra.info = Some(reverse_ra);
                            self.new_ra.is_reversal = true;
                        }
                    } else if strengthen.is_some() {
                        self.new_ra.info = strengthen;
                    } else {
                        self.new_ra.info = previous.info;
                        self.new_ra.has_been_announced = true;
                    }
                } else {
                    self.new_ra.info = previous.info;
                    self.new_ra.has_been_announced = true;
                }
            }
        }
    }

    fn update_advisory_state(&mut self, dt_ms: f64) {
        let ta_threat_count =
            self.air_traffic.iter().filter(|t| t.alive && t.is_displayed && t.intrusion == INTR_TA).count();
        let ra_threat_count = self.ra_traffic.len();
        let ra_inhibited = self.inhibitions == Inhibit::AllRa || self.inhibitions == Inhibit::AllRaAuralTa;

        match self.advisory_state {
            TcasState::Ta => {
                if ra_threat_count > 0 && !ra_inhibited {
                    self.advisory_state = TcasState::Ra;
                    self.vars.state = TcasState::Ra as u8;
                } else if ta_threat_count == 0 {
                    self.advisory_state = TcasState::None;
                    self.vars.state = TcasState::None as u8;
                }
            }
            TcasState::Ra => {
                if ra_threat_count == 0 {
                    if ta_threat_count > 0 {
                        self.advisory_state = TcasState::Ta;
                        self.vars.state = TcasState::Ta as u8;
                    } else {
                        self.advisory_state = TcasState::None;
                        self.vars.state = TcasState::None as u8;
                    }
                    self.play(SOUND_CLEAR_OF_CONFLICT, None);
                    self.active_ra.info = None;
                }
            }
            TcasState::None => {
                if ra_threat_count > 0 && !ra_inhibited {
                    self.advisory_state = TcasState::Ra;
                    self.vars.state = TcasState::Ra as u8;
                } else if ta_threat_count > 0 {
                    self.advisory_state = TcasState::Ta;
                    self.vars.state = TcasState::Ta as u8;
                    if self.inhibitions != Inhibit::AllRaAuralTa {
                        self.play(SOUND_TRAFFIC_TRAFFIC, None);
                    }
                }
            }
        }

        if let (Some(info), TcasState::Ra) = (self.new_ra.info, self.advisory_state) {
            self.active_ra.info = Some(info);
            self.active_ra.is_reversal = self.new_ra.is_reversal;
            self.active_ra.seconds_since_start = self.new_ra.seconds_since_start;
            self.active_ra.has_been_announced = self.new_ra.has_been_announced;
            self.active_ra.seconds_since_start += dt_ms / 1000.0;
            if !self.active_ra.has_been_announced {
                self.play(info.callout.sound, Some(info.callout.repeat));
                let corrective = info.kind == RaType::Correct;
                self.vars.ra_corrective = corrective;
                self.vars.ra_type = info.type2 as u8;
                self.vars.rate_to_maintain = info.rate_to_maintain;
                self.vars.up_advisory = info.up as u8;
                self.vars.down_advisory = info.down as u8;
                self.vars.vspeed_red = info.red;
                if corrective {
                    self.vars.vspeed_green = info.green;
                }
                self.active_ra.has_been_announced = true;
            }
        }
    }

    fn nd_traffic(t: &Traffic) -> NdTraffic {
        NdTraffic {
            id: t.id.clone(),
            lat: t.lat,
            lon: t.lon,
            relative_alt: js_round(t.relative_alt / 100.0),
            intrusion_level: t.intrusion,
            vert_speed: t.vert_speed,
        }
    }

    fn emit_display(&mut self) {
        let mut sent: Vec<usize> =
            (0..self.air_traffic.len()).filter(|&i| self.air_traffic[i].alive && self.air_traffic[i].is_displayed).collect();
        {
            let tr = &self.air_traffic;
            js_sort(&mut sent, |&ia, &ib| {
                let (a, b) = (&tr[ia], &tr[ib]);
                let keys = [
                    b.intrusion as f64 - a.intrusion as f64,
                    a.ra_tau - b.ra_tau,
                    a.ta_tau - b.ta_tau,
                    a.slant_distance - b.slant_distance,
                ];
                for (n, k) in keys.iter().enumerate() {
                    if n == keys.len() - 1 || !(*k == 0.0 || k.is_nan()) {
                        return *k;
                    }
                }
                0.0
            });
        }
        self.send_air_traffic.clear();
        for (n, &i) in sent.iter().enumerate() {
            if n >= DISPLAY_MAX {
                break;
            }
            self.send_air_traffic.push(Self::nd_traffic(&self.air_traffic[i]));
        }
        let all = self.send_air_traffic.clone();
        let only_ta: Vec<NdTraffic> = all.iter().filter(|t| t.intrusion_level >= INTR_TA).cloned().collect();
        self.traf_l = Some(if self.inputs.traffic_left_efis { all.clone() } else { only_ta.clone() });
        self.traf_r = Some(if self.inputs.traffic_right_efis { all } else { only_ta });
    }

    fn reset_display(&mut self) {
        self.send_air_traffic.clear();
        self.traf_l = Some(Vec::new());
        self.traf_r = Some(Vec::new());
    }
}

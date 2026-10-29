use std::collections::BTreeSet;
use std::sync::Arc;

use fms::plan::{FlightPlan, SegmentKind};
use fms_lnav::geo::{bearing_to, diff_angle, distance_to};
use fms_lnav::Coord;
use fms_perf::FlightPhase;

use crate::facilities::{vhf_type, Facilities, IlsNavaid, NdbNavaid, NearbyKind, UpdateThrottler, VhfNavaid};
use crate::outputs::Sink;

const EARTH_RADIUS_NM: f64 = 3440.065;
const DISPLAY_VOR_STICKINESS_THRESHOLD: f64 = 2.;
const DESTINATION_TUNING_DISTANCE: f64 = 300.;
const DELAY_AFTER_RMP_TUNING_MS: f64 = 1000.;

pub mod approach_type {
    pub const LOC_BACKCOURSE: i32 = 1;
    pub const VOR_DME: i32 = 2;
    pub const IGS: i32 = 4;
    pub const ILS: i32 = 5;
    pub const LOC: i32 = 7;
    pub const NDB: i32 = 9;
    pub const NDB_DME: i32 = 11;
    pub const RNAV: i32 = 12;
    pub const VORTAC: i32 = 13;
    pub const SDF: i32 = 15;
    pub const VOR: i32 = 16;
    pub const LDA: i32 = 18;
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum VorSelectionReason {
    #[default]
    None,
    Display,
    Route,
    Navigation,
    Procedure,
}

pub struct NavInputs<'a> {
    pub plan: Option<&'a FlightPlan>,
    pub phase: FlightPhase,
    pub ppos: Coord,
    pub epe: f64,
    pub altitude_ft: Option<f64>,
    pub height_above_ground_ft: f64,
    pub radio_height_ft: Option<f64>,
    pub rmp_nav_selected: bool,
}

#[derive(Clone, Debug)]
struct Candidate {
    navaid: VhfNavaid,
    distance: f64,
}

fn is_vor_type(t: u32) -> bool {
    t == vhf_type::VOR || t == vhf_type::VOR_DME || t == vhf_type::VORTAC
}

fn is_vor_dme(n: &VhfNavaid) -> bool {
    n.vhf_type == vhf_type::VOR_DME || n.vhf_type == vhf_type::VORTAC
}

fn first_approach_leg_index(plan: &FlightPlan) -> usize {
    plan.segments.iter().take_while(|s| s.kind != SegmentKind::Approach).map(|s| s.legs.len()).sum()
}

pub fn is_approach_active(plan: &FlightPlan) -> bool {
    let active = plan.active_leg_index.max(0) as usize;
    plan.approach.is_some() && active >= first_approach_leg_index(plan) && active < plan.first_missed_approach_leg_index()
}

fn approach_recommended_navaid(plan: &FlightPlan) -> Option<&fms::navdb::Fix> {
    plan.seg(SegmentKind::Approach).legs.iter().rev().find_map(|e| e.leg()).and_then(|l| l.definition.recommended_navaid.as_ref())
}

/// FBW `NavaidSelectionManager`.
pub struct NavaidSelectionManager {
    candidate_throttler: UpdateThrottler,
    dme_pair_throttler: UpdateThrottler,
    autotune_throttler: UpdateThrottler,
    ppos: Coord,
    epe: f64,
    altitude: f64,
    height_above_ground: f64,
    filtered_height: Option<f64>,
    candidates: Vec<Candidate>,
    vor_candidates: Vec<Candidate>,
    display_vor: Option<VhfNavaid>,
    display_vor_reason: VorSelectionReason,
    dme_pair: Option<(VhfNavaid, VhfNavaid)>,
    display_ndb: Option<NdbNavaid>,
    pub deselected: BTreeSet<String>,
    pub specified_vor_deselected: bool,
    pub specified_ndb_deselected: bool,
}

impl Default for NavaidSelectionManager {
    fn default() -> Self {
        NavaidSelectionManager {
            candidate_throttler: UpdateThrottler::new(180.),
            dme_pair_throttler: UpdateThrottler::new(10.),
            autotune_throttler: UpdateThrottler::new(1.),
            ppos: Coord::new(0., 0.),
            epe: f64::INFINITY,
            altitude: 0.,
            height_above_ground: 0.,
            filtered_height: None,
            candidates: Vec::new(),
            vor_candidates: Vec::new(),
            display_vor: None,
            display_vor_reason: VorSelectionReason::None,
            dme_pair: None,
            display_ndb: None,
            deselected: BTreeSet::new(),
            specified_vor_deselected: false,
            specified_ndb_deselected: false,
        }
    }
}

impl NavaidSelectionManager {
    pub fn display_vor(&self) -> Option<&VhfNavaid> {
        self.display_vor.as_ref()
    }

    pub fn display_vor_reason(&self) -> VorSelectionReason {
        self.display_vor_reason
    }

    pub fn display_ndb(&self) -> Option<&NdbNavaid> {
        self.display_ndb.as_ref()
    }

    pub fn dme_pair(&self) -> Option<&(VhfNavaid, VhfNavaid)> {
        self.dme_pair.as_ref()
    }

    pub fn update(&mut self, dt_ms: f64, force: bool, i: &NavInputs, fac: &dyn Facilities) {
        self.ppos = i.ppos;
        self.epe = i.epe;
        if let Some(a) = i.altitude_ft {
            self.altitude = a;
        }
        self.height_above_ground = i.height_above_ground_ft;
        self.filtered_height = Some(match self.filtered_height {
            None => self.height_above_ground,
            Some(f) => 0.01 * self.height_above_ground + 0.99 * f,
        });
        if self.candidate_throttler.can_update(dt_ms, force).is_some() {
            self.update_candidates(fac);
        }
        if self.dme_pair_throttler.can_update(dt_ms, force).is_some() {
            self.select_dme_pair();
        }
        if self.autotune_throttler.can_update(dt_ms, force).is_some() {
            if let Some(plan) = i.plan {
                self.select_display_vor(plan, fac);
                self.select_display_ndb(plan, fac);
            }
        }
    }

    fn update_candidates(&mut self, fac: &dyn Facilities) {
        self.candidates.clear();
        self.vor_candidates.clear();
        let mut freqs: Vec<f64> = Vec::new();
        let mut dupes: Vec<f64> = Vec::new();
        for f in fac.nearby(NearbyKind::VhfNavaid, self.ppos, 381., 100) {
            if !matches!(f.vhf_type, vhf_type::DME | vhf_type::ILS_DME | vhf_type::ILS_TACAN | vhf_type::VOR_DME | vhf_type::VORTAC) {
                continue;
            }
            let Some(navaid) = fac.vhf_navaid(&f.database_id) else { continue };
            let distance = distance_to(self.ppos, navaid.dme_or_location());
            if self.in_line_of_sight(&navaid, distance) {
                if freqs.contains(&navaid.frequency) {
                    dupes.push(navaid.frequency);
                }
                freqs.push(navaid.frequency);
            }
            let eligible = !self.deselected.contains(&navaid.database_id)
                && self.in_line_of_sight(&navaid, distance)
                && self.within_fom(&navaid, distance)
                && !self.within_cone_of_confusion(&navaid, distance)
                && distance >= 9f64.min(1f64.max(self.epe));
            let display = is_vor_type(navaid.vhf_type) && self.within_fom(&navaid, distance) && !self.deselected.contains(&navaid.database_id);
            if eligible {
                self.candidates.push(Candidate { navaid: navaid.clone(), distance });
            }
            if display {
                self.vor_candidates.push(Candidate { navaid, distance });
            }
        }
        self.candidates.retain(|c| !dupes.contains(&c.navaid.frequency));
        self.candidates.sort_by(|a, b| a.distance.total_cmp(&b.distance));
        self.vor_candidates.sort_by(|a, b| a.distance.total_cmp(&b.distance));
        self.vor_candidates.truncate(7);
    }

    fn height_above_navaid_nm(&self, n: &VhfNavaid) -> f64 {
        match n.dme_location.and_then(|l| l.alt) {
            Some(alt) => (self.altitude - alt).max(0.) / 6076.12,
            None => self.filtered_height.unwrap_or(0.),
        }
    }

    fn in_line_of_sight(&self, n: &VhfNavaid, distance: f64) -> bool {
        let h = self.height_above_navaid_nm(n) / 6076.12;
        let los = (h * (2. * EARTH_RADIUS_NM + h)).sqrt();
        distance <= 10f64.max(7. / 6. * los)
    }

    fn within_fom(&self, n: &VhfNavaid, distance: f64) -> bool {
        match n.figure_of_merit {
            3 => distance <= 250.,
            2 => distance <= 130.,
            1 => distance <= 70. && self.altitude < 18000.,
            0 => distance <= 40. && self.altitude < 12000.,
            _ => false,
        }
    }

    fn within_cone_of_confusion(&self, n: &VhfNavaid, distance: f64) -> bool {
        let cone = match n.dme_location.and_then(|l| l.alt) {
            Some(alt) => (self.altitude - alt).max(0.) / 6076.12,
            None => self.altitude - self.height_above_ground,
        };
        distance < cone / 6076.12 * (std::f64::consts::PI / 6.).tan()
    }

    fn pair_angle(&self, a: &VhfNavaid, b: &VhfNavaid) -> f64 {
        diff_angle(bearing_to(self.ppos, a.dme_or_location()), bearing_to(self.ppos, b.dme_or_location())).abs()
    }

    fn select_dme_pair(&mut self) {
        let mut find_new = self.dme_pair.is_none();
        let mut geometry_check = false;
        if let Some((a, b)) = &self.dme_pair {
            let angle = self.pair_angle(a, b);
            geometry_check = !(70. ..=110.).contains(&angle);
            find_new = find_new || !self.candidates.iter().any(|c| c.navaid.database_id == a.database_id) || !self.candidates.iter().any(|c| c.navaid.database_id == b.database_id);
        }
        if !(find_new || geometry_check) {
            return;
        }
        let mut best_diff = f64::INFINITY;
        let mut best = None;
        for (i, a) in self.candidates.iter().enumerate() {
            for b in &self.candidates[i + 1..] {
                let angle = self.pair_angle(&a.navaid, &b.navaid);
                if angle > 30. && angle < 150. {
                    if self.altitude < 12000. {
                        self.dme_pair = Some((a.navaid.clone(), b.navaid.clone()));
                        return;
                    }
                    let d = diff_angle(90., angle).abs();
                    if d < best_diff {
                        best = Some((a.navaid.clone(), b.navaid.clone()));
                        best_diff = d;
                    }
                }
            }
        }
        if geometry_check {
            let current = self.dme_pair.as_ref().map_or(f64::INFINITY, |(a, b)| diff_angle(90., self.pair_angle(a, b)).abs());
            if current - best_diff >= 10. {
                self.dme_pair = best;
            }
        } else {
            self.dme_pair = best;
        }
    }

    fn specified_vor(&self, plan: &FlightPlan) -> Option<VhfNavaid> {
        let t = plan.approach.as_ref()?.approach_type;
        if ![approach_type::RNAV, approach_type::VOR, approach_type::VOR_DME, approach_type::VORTAC].contains(&t) {
            return None;
        }
        let f = approach_recommended_navaid(plan)?;
        (f.section_code == 3 && f.sub_section_code == 0).then(|| VhfNavaid {
            database_id: f.database_id.clone(),
            ident: f.ident.clone(),
            location: f.location,
            vhf_type: vhf_type::VOR,
            figure_of_merit: 3,
            ..VhfNavaid::default()
        })
    }

    fn select_display_vor(&mut self, plan: &FlightPlan, fac: &dyn Facilities) {
        self.specified_vor_deselected = false;
        if is_approach_active(plan) {
            if let Some(s) = self.specified_vor(plan) {
                if self.deselected.contains(&s.database_id) {
                    self.specified_vor_deselected = true;
                } else {
                    self.set_display_vor(Some(s), VorSelectionReason::Procedure);
                    return;
                }
            }
        }
        if self.dme_pair.is_none() {
            if let Some(r) = self.reference_navaid(plan) {
                self.set_display_vor(Some(r), VorSelectionReason::Navigation);
                return;
            }
            if let Some(c) = self.candidates.iter().find(|c| is_vor_dme(&c.navaid)) {
                let n = c.navaid.clone();
                self.set_display_vor(Some(n), VorSelectionReason::Navigation);
                return;
            }
        }
        let active = plan.active_leg_index;
        if active >= 0 {
            let all = plan.all_legs();
            for i in active as usize..active as usize + 5 {
                let Some(e) = all.get(i) else { break };
                let Some(leg) = e.leg() else { break };
                if let Some(w) = leg.definition.waypoint.as_ref() {
                    if w.section_code == 3 && w.sub_section_code == 0 {
                        if let Some(n) = fac.vhf_navaid(&w.database_id) {
                            if is_vor_type(n.vhf_type) && self.within_fom(&n, distance_to(self.ppos, n.dme_or_location())) {
                                self.set_display_vor(Some(n), VorSelectionReason::Route);
                                return;
                            }
                        }
                    }
                }
            }
        }
        if let (Some(first), Some(current_id)) = (self.vor_candidates.first(), self.display_vor.as_ref().map(|v| v.database_id.clone())) {
            let current = self.vor_candidates.iter().find(|c| c.navaid.database_id == current_id);
            let replace = match current {
                None => true,
                Some(c) => self.display_vor_reason != VorSelectionReason::Display || c.distance - first.distance > DISPLAY_VOR_STICKINESS_THRESHOLD,
            };
            if replace {
                let n = first.navaid.clone();
                self.set_display_vor(Some(n), VorSelectionReason::Display);
            }
        } else if let Some(first) = self.vor_candidates.first() {
            let n = first.navaid.clone();
            self.set_display_vor(Some(n), VorSelectionReason::Display);
        } else {
            self.set_display_vor(None, VorSelectionReason::None);
        }
    }

    fn reference_navaid(&self, plan: &FlightPlan) -> Option<VhfNavaid> {
        let rwy = plan.destination_runway.as_ref()?;
        let thr = Coord::new(rwy.threshold_location.lat, rwy.threshold_location.long);
        let mut c: Vec<&Candidate> = self.candidates.iter().filter(|c| is_vor_dme(&c.navaid) && c.distance < 51.).collect();
        c.sort_by(|a, b| distance_to(thr, a.navaid.dme_or_location()).total_cmp(&distance_to(thr, b.navaid.dme_or_location())));
        c.first().filter(|c| distance_to(thr, c.navaid.dme_or_location()) < 5.).map(|c| c.navaid.clone())
    }

    fn select_display_ndb(&mut self, plan: &FlightPlan, fac: &dyn Facilities) {
        self.specified_ndb_deselected = false;
        if is_approach_active(plan) && plan.approach.as_ref().is_some_and(|a| [approach_type::NDB, approach_type::NDB_DME].contains(&a.approach_type)) {
            if let Some(f) = approach_recommended_navaid(plan).filter(|f| f.section_code == 3 && f.sub_section_code == 1) {
                if self.deselected.contains(&f.database_id) {
                    self.specified_ndb_deselected = true;
                } else {
                    self.display_ndb = Some(fac.ndb_navaid(&f.database_id).unwrap_or(NdbNavaid { database_id: f.database_id.clone(), ident: f.ident.clone(), location: f.location, frequency: 0. }));
                    return;
                }
            }
        }
        self.display_ndb = None;
    }

    fn set_display_vor(&mut self, vor: Option<VhfNavaid>, reason: VorSelectionReason) {
        self.display_vor = vor;
        self.display_vor_reason = reason;
    }
}

/// FBW `LandingSystemSelectionManager`.
pub struct LandingSystemSelectionManager {
    throttler: UpdateThrottler,
    plan_version: Option<u32>,
    pub selected_ils: Option<IlsNavaid>,
    selected_loc_course: Option<f64>,
    pub selected_backcourse: bool,
    pub selected_gs_slope: Option<f64>,
}

impl Default for LandingSystemSelectionManager {
    fn default() -> Self {
        LandingSystemSelectionManager { throttler: UpdateThrottler::new(30_000.), plan_version: None, selected_ils: None, selected_loc_course: None, selected_backcourse: false, selected_gs_slope: None }
    }
}

impl LandingSystemSelectionManager {
    pub fn selected_loc_course(&self) -> Option<f64> {
        self.selected_loc_course.map(|c| (c % 360.).round())
    }

    pub fn update(&mut self, dt_ms: f64, i: &NavInputs, fac: &dyn Facilities) {
        let Some(plan) = i.plan else { return };
        let force = self.plan_version != Some(plan.version);
        self.plan_version = Some(plan.version);
        if self.throttler.can_update(dt_ms, force).is_none() {
            return;
        }
        if i.phase <= FlightPhase::Takeoff {
            self.select_departure(plan, fac);
        } else if i.phase >= FlightPhase::Descent {
            self.select_approach(plan, fac);
        } else if i.phase >= FlightPhase::Cruise {
            match plan.destination_airport.as_ref() {
                Some(d) if distance_to(i.ppos, Coord::new(d.location.lat, d.location.long)) <= DESTINATION_TUNING_DISTANCE => self.select_approach(plan, fac),
                _ => self.reset(),
            }
        } else {
            self.reset();
        }
    }

    fn select_departure(&mut self, plan: &FlightPlan, fac: &dyn Facilities) {
        let found = plan.origin_runway.as_ref().and_then(|r| {
            let airport = r.fix.airport_ident.clone().or_else(|| plan.origin_airport.as_ref().map(|a| a.ident.clone()))?;
            let ls = fac.runway_details(&airport, &r.fix.ident)?.ls_ident?;
            fac.ils_at_airport(&airport, &ls)
        });
        match found {
            Some(ils) => {
                self.selected_loc_course = (ils.loc_bearing != -1.).then_some(ils.loc_bearing);
                self.selected_gs_slope = ils.gs_slope;
                self.selected_backcourse = false;
                self.selected_ils = Some(ils);
            }
            None => self.reset(),
        }
    }

    fn select_approach(&mut self, plan: &FlightPlan, fac: &dyn Facilities) {
        let Some(appr) = plan.approach.as_ref().filter(|a| matches!(a.approach_type, approach_type::IGS | approach_type::ILS | approach_type::LDA | approach_type::LOC | approach_type::LOC_BACKCOURSE | approach_type::SDF)) else {
            self.reset();
            return;
        };
        let Some(rec) = appr.common_legs.last().and_then(|l| l.recommended_navaid.as_ref()).filter(|n| !n.database_id.trim().is_empty()) else { return };
        let backcourse = appr.approach_type == approach_type::LOC_BACKCOURSE;
        if self.selected_ils.as_ref().is_some_and(|s| s.database_id == rec.database_id) && backcourse == self.selected_backcourse {
            return;
        }
        let Some(ils) = fac.ils(&rec.database_id).filter(|i| i.is_localizer_glide_slope) else { return };
        self.selected_backcourse = backcourse;
        self.selected_loc_course = Some((if backcourse { ils.loc_bearing + 180. } else { ils.loc_bearing }).rem_euclid(360.));
        self.selected_gs_slope = ils.gs_slope;
        self.selected_ils = Some(ils);
    }

    fn reset(&mut self) {
        self.selected_ils = None;
        self.selected_loc_course = None;
        self.selected_backcourse = false;
        self.selected_gs_slope = None;
    }
}

#[derive(Clone, Debug, Default)]
pub struct VorStatus {
    pub frequency: Option<f64>,
    pub ident: Option<String>,
    pub manual: bool,
    pub facility: Option<VhfNavaid>,
    pub course: Option<f64>,
    pub dme_only: bool,
}

#[derive(Clone, Debug, Default)]
pub struct MmrStatus {
    pub frequency: Option<f64>,
    pub ident: Option<String>,
    pub manual: bool,
    pub facility: Option<IlsNavaid>,
    pub database_course: Option<f64>,
    pub database_backcourse: bool,
    pub course: Option<f64>,
    pub course_manual: bool,
    pub backcourse: bool,
    pub slope: Option<f64>,
}

#[derive(Clone, Debug, Default)]
pub struct AdfStatus {
    pub frequency: Option<f64>,
    pub ident: Option<String>,
    pub manual: bool,
    pub facility: Option<NdbNavaid>,
}

fn freq_eq(a: Option<f64>, b: Option<f64>) -> bool {
    (a.unwrap_or(0.) - b.unwrap_or(0.)).abs() < 0.01
}

fn course_eq(a: Option<f64>, b: Option<f64>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => a.round() == b.round(),
        (None, None) => true,
        (Some(a), None) | (None, Some(a)) => a.round() == 0.,
    }
}

/// `Avionics.Utils.make_adf_bcd32`.
pub fn make_adf_bcd32(hz: f64) -> f64 {
    let mut v = hz.round() as u64;
    let mut bcd: u64 = 0;
    let mut shift = 0;
    while v > 0 {
        bcd |= (v % 10) << shift;
        v /= 10;
        shift += 4;
    }
    bcd as f64
}

/// FBW `NavaidTuner` together with the two selection managers it reads.
pub struct NavaidTuner {
    pub selection: NavaidSelectionManager,
    pub landing: LandingSystemSelectionManager,
    initialised: bool,
    pub vor: [VorStatus; 2],
    pub mmr: [MmrStatus; 2],
    pub adf: [AdfStatus; 2],
    last_vor_freq: [Option<f64>; 2],
    last_vor_course: [Option<f64>; 2],
    last_mmr_freq: [Option<f64>; 2],
    last_mmr_course: [Option<f64>; 2],
    last_adf_freq: [Option<f64>; 2],
    pub navaid_version: u32,
    rmp_tuning_active: bool,
    tuning_lockout_ms: f64,
    tuning_active: bool,
    nav_discrete_bits: u32,
    last_discrete: Option<(u32, bool)>,
    pub tune_navaid_message: Option<(f64, String)>,
    pub rwy_ls_mismatch: bool,
    facilities: Arc<dyn Facilities>,
}

impl NavaidTuner {
    pub fn new(facilities: Arc<dyn Facilities>) -> Self {
        NavaidTuner {
            selection: NavaidSelectionManager::default(),
            landing: LandingSystemSelectionManager::default(),
            initialised: false,
            vor: Default::default(),
            mmr: Default::default(),
            adf: Default::default(),
            last_vor_freq: [None; 2],
            last_vor_course: [None; 2],
            last_mmr_freq: [None; 2],
            last_mmr_course: [None; 2],
            last_adf_freq: [None; 2],
            navaid_version: 0,
            rmp_tuning_active: false,
            tuning_lockout_ms: -1.,
            tuning_active: false,
            nav_discrete_bits: 0,
            last_discrete: None,
            tune_navaid_message: None,
            rwy_ls_mismatch: false,
            facilities,
        }
    }

    pub fn tuned_vors(&self) -> impl Iterator<Item = &VhfNavaid> {
        self.vor.iter().filter_map(|v| v.facility.as_ref())
    }

    pub fn tuned_ndbs(&self) -> impl Iterator<Item = &NdbNavaid> {
        self.adf.iter().filter_map(|a| a.facility.as_ref())
    }

    pub fn is_fm_tuning_active(&self) -> bool {
        self.tuning_active
    }

    fn mmr_locked(&self, i: &NavInputs) -> bool {
        i.phase == FlightPhase::Approach && i.radio_height_ft.unwrap_or(f64::INFINITY) < 700.
    }

    pub fn update(&mut self, dt_ms: f64, i: &NavInputs, sink: &mut Sink) {
        let fac = self.facilities.clone();
        self.selection.update(dt_ms, false, i, fac.as_ref());
        self.landing.update(dt_ms, i, fac.as_ref());
        if i.phase == FlightPhase::Done {
            self.reset_state(false);
        }
        if !self.initialised {
            self.initialised = true;
            sink.set_bool("A32NX_FM1_BACKBEAM_SELECTED", false);
            sink.set_bool("A32NX_FM2_BACKBEAM_SELECTED", false);
            self.reset_all_receivers(i, sink);
        }
        let rmp_deactivated = !i.rmp_nav_selected && self.rmp_tuning_active;
        self.rmp_tuning_active = i.rmp_nav_selected;
        if rmp_deactivated {
            self.reset_all_receivers(i, sink);
            self.tuning_lockout_ms = DELAY_AFTER_RMP_TUNING_MS;
        }
        let active = !self.rmp_tuning_active && self.tuning_lockout_ms <= 0.;
        let deactivated = !active && self.tuning_active;
        self.tuning_active = active;
        if deactivated {
            self.reset_all_receivers(i, sink);
        }
        if self.tuning_active {
            self.update_selection(i, sink);
            self.rwy_ls_mismatch = self.runway_ls_mismatch(0) || self.runway_ls_mismatch(1);
        } else if self.tuning_lockout_ms > 0. {
            self.tuning_lockout_ms -= dt_ms;
        }
        self.update_arinc_bus(sink);
    }

    fn reset_all_receivers(&mut self, i: &NavInputs, sink: &mut Sink) {
        for k in 0..2 {
            self.tune_vor_frequency(k, None, sink);
            self.tune_vor_course(k, None, sink);
            self.tune_mmr_frequency(k, None, i, sink);
            self.tune_mmr_course(k, None, false, i, sink);
            self.tune_adf(k, None, sink);
        }
    }

    fn update_arinc_bus(&mut self, sink: &mut Sink) {
        let mut bits = 0u32;
        for k in 0..2 {
            if self.tuning_active && self.vor[k].manual {
                bits |= 1 << (10 + k);
            }
            if self.tuning_active && self.adf[k].manual {
                bits |= 1 << (12 + k);
            }
            if self.tuning_active && self.mmr[k].manual {
                bits |= 1 << (14 + k);
            }
        }
        self.nav_discrete_bits = bits;
        let state = (bits, self.tuning_active);
        if self.last_discrete != Some(state) {
            self.last_discrete = Some(state);
            let ssm: f64 = if self.tuning_active { 3. } else { 1. };
            sink.set("A32NX_FM1_NAV_DISCRETE", ssm * 4_294_967_296. + f32::to_bits(bits as f32) as f64);
        }
    }

    fn update_selection(&mut self, i: &NavInputs, sink: &mut Sink) {
        let mut message = None;
        for k in 0..2 {
            let auto = self.selection.display_vor().cloned();
            let v = &mut self.vor[k];
            if v.manual {
                let reason = self.selection.display_vor_reason();
                if matches!(reason, VorSelectionReason::Navigation | VorSelectionReason::Procedure) && !freq_eq(auto.as_ref().map(|a| a.frequency), v.frequency) {
                    message = auto.as_ref().map(|a| (a.frequency, a.ident.clone()));
                }
            } else if v.facility.as_ref().map(|f| &f.database_id) != auto.as_ref().map(|f| &f.database_id) {
                v.course = None;
                v.frequency = auto.as_ref().map(|a| a.frequency);
                v.ident = auto.as_ref().map(|a| a.ident.clone());
                v.dme_only = auto.as_ref().is_some_and(|a| a.vhf_type == vhf_type::DME || a.vhf_type == vhf_type::TACAN);
                v.facility = auto;
            }
            let (f, c) = (self.vor[k].frequency, self.vor[k].course);
            self.tune_vor_frequency(k, f, sink);
            self.tune_vor_course(k, c, sink);
        }
        for k in 0..2 {
            let auto = self.landing.selected_ils.clone();
            let auto_course = self.landing.selected_loc_course();
            let auto_back = self.landing.selected_backcourse;
            let m = &mut self.mmr[k];
            if !m.manual && (m.facility.as_ref().map(|f| &f.database_id) != auto.as_ref().map(|f| &f.database_id) || m.course != auto_course || m.backcourse != auto_back) && (auto_course.is_some() || auto.is_none()) {
                m.database_course = auto_course;
                m.database_backcourse = auto_back;
                m.course = auto_course;
                m.course_manual = false;
                m.frequency = auto.as_ref().map(|a| a.frequency);
                m.ident = auto.as_ref().map(|a| a.ident.clone());
                m.backcourse = auto_back;
                m.slope = self.landing.selected_gs_slope;
                m.facility = auto;
            }
            let (f, c, b) = (self.mmr[k].frequency, self.mmr[k].course, self.mmr[k].backcourse);
            self.tune_mmr_frequency(k, f, i, sink);
            self.tune_mmr_course(k, c, b, i, sink);
        }
        for k in 0..2 {
            let auto = self.selection.display_ndb().cloned();
            let a = &mut self.adf[k];
            if !a.manual && a.facility.as_ref().map(|f| &f.database_id) != auto.as_ref().map(|f| &f.database_id) {
                a.frequency = auto.as_ref().map(|n| n.frequency);
                a.ident = auto.as_ref().map(|n| n.ident.clone());
                a.facility = auto;
            }
            let f = self.adf[k].frequency;
            self.tune_adf(k, f, sink);
        }
        self.tune_navaid_message = message;
    }

    fn tune_vor_frequency(&mut self, k: usize, f: Option<f64>, sink: &mut Sink) {
        if !freq_eq(self.last_vor_freq[k], f) {
            self.last_vor_freq[k] = f;
            self.navaid_version += 1;
            sink.key(&format!("NAV{}_RADIO_SET_HZ", k + 1), f.unwrap_or(0.) * 1_000_000.);
        }
    }

    fn tune_vor_course(&mut self, k: usize, c: Option<f64>, sink: &mut Sink) {
        if !course_eq(self.last_vor_course[k], c) {
            self.last_vor_course[k] = c;
            sink.key(&format!("VOR{}_SET", k + 1), c.unwrap_or(0.));
        }
    }

    fn tune_mmr_frequency(&mut self, k: usize, f: Option<f64>, i: &NavInputs, sink: &mut Sink) {
        if self.mmr_locked(i) {
            return;
        }
        if !freq_eq(self.last_mmr_freq[k], f) {
            self.last_mmr_freq[k] = f;
            self.navaid_version += 1;
            sink.key(&format!("NAV{}_RADIO_SET_HZ", k + 3), f.unwrap_or(0.) * 1_000_000.);
        }
    }

    fn tune_mmr_course(&mut self, k: usize, c: Option<f64>, backcourse: bool, i: &NavInputs, sink: &mut Sink) {
        if self.mmr_locked(i) {
            return;
        }
        if !course_eq(self.last_mmr_course[k], c) {
            self.last_mmr_course[k] = c;
            sink.key(&format!("VOR{}_SET", k + 3), c.unwrap_or(0.));
            return;
        }
        sink.set_bool(&format!("A32NX_FM{}_BACKBEAM_SELECTED", k + 1), backcourse);
    }

    fn tune_adf(&mut self, k: usize, f: Option<f64>, sink: &mut Sink) {
        if !freq_eq(self.last_adf_freq[k], f) {
            self.last_adf_freq[k] = f;
            self.navaid_version += 1;
            let name = if k == 0 { "ADF_COMPLETE_SET".to_string() } else { format!("ADF{}_COMPLETE_SET", k + 1) };
            sink.key(&name, make_adf_bcd32(f.unwrap_or(0.) * 1000.));
        }
    }

    fn runway_ls_mismatch(&self, k: usize) -> bool {
        let m = &self.mmr[k];
        let db_freq = self.landing.selected_ils.as_ref().map(|i| i.frequency);
        if let (Some(f), Some(d)) = (m.frequency, db_freq) {
            if !freq_eq(Some(d), Some(f)) {
                return true;
            }
        }
        matches!((m.course, self.landing.selected_loc_course()), (Some(c), Some(d)) if (c - d).abs() > 3.)
    }

    pub fn set_manual_vor(&mut self, index: usize, vor: Option<Result<VhfNavaid, f64>>) {
        let v = &mut self.vor[index.min(1)];
        *v = match vor {
            None => VorStatus::default(),
            Some(Err(freq)) => VorStatus { manual: true, frequency: Some(freq), ..VorStatus::default() },
            Some(Ok(n)) => VorStatus { manual: true, frequency: Some(n.frequency), ident: Some(n.ident.clone()), dme_only: n.vhf_type == vhf_type::DME || n.vhf_type == vhf_type::TACAN, facility: Some(n), course: None },
        };
    }

    pub fn set_vor_course(&mut self, index: usize, course: Option<f64>) {
        self.vor[index.min(1)].course = course;
    }

    pub fn set_manual_ils(&mut self, ils: Option<Result<IlsNavaid, f64>>) {
        for m in &mut self.mmr {
            *m = match &ils {
                None => MmrStatus::default(),
                Some(Err(freq)) => MmrStatus { manual: true, frequency: Some(*freq), ..MmrStatus::default() },
                Some(Ok(n)) => {
                    let course = (n.loc_bearing != -1.).then_some(n.loc_bearing);
                    MmrStatus { manual: true, database_course: course, course, frequency: Some(n.frequency), ident: Some(n.ident.clone()), slope: n.gs_slope, facility: Some(n.clone()), ..MmrStatus::default() }
                }
            };
        }
    }

    pub fn set_ils_course(&mut self, course: Option<f64>, backcourse: bool) {
        for m in &mut self.mmr {
            m.course = match course {
                None => m.database_course,
                Some(c) => Some(c % 360.),
            };
            m.backcourse = if course.is_none() { m.database_backcourse } else { backcourse };
            m.course_manual = course.is_some();
        }
    }

    pub fn set_manual_adf(&mut self, index: usize, ndb: Option<Result<NdbNavaid, f64>>) {
        let a = &mut self.adf[index.min(1)];
        *a = match ndb {
            None => AdfStatus::default(),
            Some(Err(freq)) => AdfStatus { manual: true, frequency: Some(freq), ..AdfStatus::default() },
            Some(Ok(n)) => AdfStatus { manual: true, frequency: Some(n.frequency), ident: Some(n.ident.clone()), facility: Some(n) },
        };
    }

    pub fn deselect_navaid(&mut self, database_id: &str) {
        self.selection.deselected.insert(database_id.to_string());
    }

    pub fn reselect_navaid(&mut self, database_id: &str) {
        self.selection.deselected.remove(database_id);
    }

    pub fn reset_state(&mut self, with_lockout: bool) {
        self.vor = Default::default();
        self.adf = Default::default();
        self.mmr = Default::default();
        if with_lockout {
            self.tuning_lockout_ms = DELAY_AFTER_RMP_TUNING_MS;
        }
    }
}

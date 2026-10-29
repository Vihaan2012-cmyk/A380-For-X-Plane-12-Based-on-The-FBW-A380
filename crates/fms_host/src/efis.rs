use std::collections::BTreeMap;

use fms::plan::{FlightPlan, LegType};
use fms_lnav::{Geometry, PathVector};
use fms_perf::FlightPhase;
use serde_json::Value;

use crate::modes;
use crate::outputs::Sink;

pub const SIDES: [&str; 2] = ["L", "R"];
pub const NUM_SECONDARY: usize = 3;
pub const FIRST_SECONDARY: usize = 3;
pub const A380_NUM_LEGS_ON_FPLN_PAGE: usize = 9;
pub const ND_MODE_PLAN: i32 = 4;
const UPDATE_TIMER_MS: f64 = 2_500.;
const MAX_VECTOR_LENGTH_NM: f64 = 5_000.;

/// FBW `EfisInterface`, one per side.
#[derive(Clone, Debug)]
pub struct EfisInterface {
    pub version: u32,
    pub plan_centre: (usize, usize, bool),
    pub sec_related_page_open: Option<usize>,
    pub shown_missed: bool,
    pub shown_alternate: bool,
    pub shown_alternate_missed: bool,
    pub num_legs_on_fpln_page: usize,
}

impl Default for EfisInterface {
    fn default() -> Self {
        EfisInterface {
            version: 0,
            plan_centre: (0, 0, false),
            sec_related_page_open: None,
            shown_missed: false,
            shown_alternate: false,
            shown_alternate_missed: false,
            num_legs_on_fpln_page: A380_NUM_LEGS_ON_FPLN_PAGE,
        }
    }
}

impl EfisInterface {
    pub fn set_plan_centre(&mut self, fp_index: usize, index: usize, in_alternate: bool) {
        if self.plan_centre != (fp_index, index, in_alternate) {
            self.plan_centre = (fp_index, index, in_alternate);
            self.version += 1;
        }
    }

    pub fn set_sec_related_page_open(&mut self, sec: Option<usize>) {
        if self.sec_related_page_open != sec {
            self.sec_related_page_open = sec;
            self.version += 1;
        }
    }

    pub fn should_transmit_secondary(&self, sec: usize) -> bool {
        self.sec_related_page_open == Some(sec)
    }

    pub fn should_transmit_any_secondary(&self) -> bool {
        self.sec_related_page_open.is_some()
    }

    fn page_window(&self, plan: &FlightPlan) -> (i64, i64) {
        let first = plan.active_leg_index - 1;
        (first, first + self.num_legs_on_fpln_page as i64 - 1)
    }

    pub fn should_transmit_missed(&self, plan_index: usize, plan: &FlightPlan, plan_mode: bool) -> bool {
        let first_missed = plan.first_missed_approach_leg_index() as i64;
        if !plan_mode {
            let (first, last) = self.page_window(plan);
            let last_missed = plan.leg_count() as i64 - 1;
            return self.shown_missed || (first <= last_missed && last >= first_missed);
        }
        plan_index == self.plan_centre.0 && self.plan_centre.1 as i64 <= plan.leg_count() as i64 && self.plan_centre.1 as i64 >= first_missed
    }

    pub fn should_transmit_alternate(&self, plan_index: usize, plan: &FlightPlan, plan_mode: bool) -> bool {
        if !plan_mode {
            let (first, last) = self.page_window(plan);
            let altn_count = plan.alternate.as_ref().map_or(0, |a| a.leg_count()) as i64;
            let first_altn = plan.leg_count() as i64 + 1;
            let last_altn = plan.leg_count() as i64 + altn_count;
            return self.shown_alternate || (first <= last_altn && last >= first_altn);
        }
        plan_index == self.plan_centre.0 && self.plan_centre.2
    }

    pub fn should_transmit_alternate_missed(&self, plan_index: usize, plan: &FlightPlan, plan_mode: bool) -> bool {
        let (altn_count, altn_first_missed) = plan.alternate.as_ref().map_or((0, 0), |a| (a.leg_count(), a.first_missed_approach_leg_index()));
        if !plan_mode {
            let (first, last) = self.page_window(plan);
            let first_altn_missed = (plan.leg_count() + 1 + altn_first_missed) as i64;
            let last_altn = (plan.leg_count() + altn_count) as i64;
            return self.shown_alternate_missed || (first <= last_altn && last >= first_altn_missed);
        }
        plan_index == self.plan_centre.0 && self.plan_centre.2 && self.plan_centre.1 >= altn_first_missed
    }

    /// `eosid_in_plans`: the active or temporary plan already carries the EOSID (FBW `FlightPlanFlags.EngineOutSid`).
    pub fn should_transmit_eosid(&self, plan_index: usize, plan_mode: bool, engine_out_active: bool, departure_active: bool, eosid_in_plans: bool) -> bool {
        if plan_index != 0 || !departure_active {
            return false;
        }
        if engine_out_active {
            return !eosid_in_plans;
        }
        plan_mode
    }
}

pub const PLAN_FLAG_ENGINE_OUT_SID: u32 = 1 << 6;

pub fn eosid_in_plans(plans: &[Option<FlightPlan>]) -> bool {
    plans.iter().take(2).flatten().any(|p| p.flags & PLAN_FLAG_ENGINE_OUT_SID != 0)
}

/// FBW `FlightPlanUtils.getEngineOutVectorsInFlightPlan`.
pub fn engine_out_vectors(plan: &FlightPlan, eosid: &Geometry) -> Vec<PathVector> {
    let Some(branch) = plan.all_legs().iter().position(|e| e.leg().is_some_and(|l| l.definition.is_engine_out_branch)) else { return Vec::new() };
    let legs = plan.engine_out_legs();
    let mut out = Vec::new();
    for i in branch + 1..legs.len().min(eosid.leg_count()) {
        let Some(leg) = legs[i].leg() else { continue };
        if is_course_reversal(leg.leg_type) {
            continue;
        }
        out.extend(eosid.leg_path(i).iter().copied());
    }
    out
}

pub fn vector_reasonable(v: &PathVector) -> bool {
    v.is_valid() && v.length() <= MAX_VECTOR_LENGTH_NM
}

fn is_course_reversal(t: LegType) -> bool {
    matches!(t, LegType::HA | LegType::HF | LegType::HM | LegType::PI)
}

/// FBW `FlightPlanUtils.getAllPathVectorsInFlightPlan`: each leg's path with its inbound transition.
pub fn plan_path_vectors(plan: &FlightPlan, geometry: &Geometry, missed: bool) -> Vec<PathVector> {
    geometry.efis_path_vectors(plan.active_leg_index.max(0) as usize, missed)
}

pub fn departure_procedure_active(plan: &FlightPlan) -> bool {
    if plan.departure.get().is_none() {
        return false;
    }
    let active = plan.active_leg_index.max(0) as usize;
    plan.segment_position(active).is_some_and(|(s, _)| plan.segments[s].kind.class() == fms::plan::SegmentClass::Departure)
}

fn vectors_json(vectors: &[PathVector]) -> Value {
    Value::Array(vectors.iter().filter(|v| vector_reasonable(v)).map(PathVector::to_json).collect())
}

pub struct VectorsContext<'a> {
    pub plans: &'a [Option<FlightPlan>; 6],
    pub geometries: &'a BTreeMap<(usize, bool), Geometry>,
    pub phase: FlightPhase,
    pub lateral_mode: i32,
    pub lateral_armed: u32,
    pub nd_mode: [i32; 2],
    pub engine_out_active: bool,
    pub eosid: Option<&'a Geometry>,
}

/// FBW `EfisVectors`. `last_fp_versions` is shared by both sides as in FBW, so the side
/// processed second only transmits on a forced update.
#[derive(Clone, Debug, Default)]
pub struct EfisVectors {
    last_fp_versions: BTreeMap<usize, u32>,
    last_interface_versions: [Option<u32>; 2],
    timer_ms: f64,
    pub legs_culled: [bool; 2],
}

impl EfisVectors {
    pub fn force_update(&mut self) {
        self.timer_ms = UPDATE_TIMER_MS + 1.;
    }

    pub fn update(&mut self, dt_ms: f64, ctx: &VectorsContext, ifaces: &[EfisInterface; 2], sink: &mut Sink) {
        self.timer_ms += dt_ms;
        let force = self.timer_ms >= UPDATE_TIMER_MS;
        for side in 0..2 {
            self.update_side(side, force, ctx, ifaces, sink);
        }
        if force {
            self.timer_ms = 0.;
        }
    }

    fn update_side(&mut self, side: usize, force: bool, ctx: &VectorsContext, ifaces: &[EfisInterface; 2], sink: &mut Sink) {
        let iface = &ifaces[side];
        if force || self.last_interface_versions[side] != Some(iface.version) {
            self.last_interface_versions[side] = Some(iface.version);
            self.try_process(0, side, true, ctx, iface, sink);
            self.try_process(1, side, true, ctx, iface, sink);
            for i in 0..NUM_SECONDARY {
                self.try_process(FIRST_SECONDARY + i, side, true, ctx, iface, sink);
            }
            let culled = match (&ctx.plans[0], ctx.geometries.get(&(0, false))) {
                (Some(p), Some(g)) => {
                    let all = g.all_path_vectors(Some(p.active_leg_index.max(0) as usize), false);
                    all.iter().any(|v| !vector_reasonable(v))
                }
                _ => false,
            };
            self.legs_culled[side] = culled;
        } else {
            self.try_process(0, side, false, ctx, iface, sink);
            self.try_process(1, side, false, ctx, iface, sink);
            for i in 0..NUM_SECONDARY {
                self.try_process(FIRST_SECONDARY + i, side, false, ctx, iface, sink);
            }
        }
    }

    fn transmit(sink: &mut Sink, side: usize, group: &str, vectors: Option<Value>) {
        sink.gd(&format!("A32NX_EFIS_VECTORS_{}_{}", SIDES[side], group), vectors.unwrap_or(Value::Null));
    }

    fn try_process(&mut self, index: usize, side: usize, force: bool, ctx: &VectorsContext, iface: &EfisInterface, sink: &mut Sink) {
        let Some(plan) = ctx.plans.get(index).and_then(|p| p.as_ref()) else {
            self.last_fp_versions.remove(&index);
            match index {
                0 => {
                    for g in ["ACTIVE", "DASHED", "MISSED", "ALTERNATE", "ACTIVE_EOSID"] {
                        Self::transmit(sink, side, g, None);
                    }
                }
                1 => Self::transmit(sink, side, "TEMPORARY", None),
                _ => {
                    if !iface.should_transmit_any_secondary() {
                        Self::transmit(sink, side, "SECONDARY", None);
                    }
                }
            }
            return;
        };
        if !force && self.last_fp_versions.get(&index) == Some(&plan.version) {
            return;
        }
        self.last_fp_versions.insert(index, plan.version);
        match index {
            0 => {
                let transmit_active = match ctx.phase {
                    FlightPhase::Preflight => true,
                    FlightPhase::Takeoff => ctx.lateral_mode == modes::LAT_NAV || modes::is_armed(ctx.lateral_armed, modes::ARMED_LAT_NAV),
                    _ => !matches!(ctx.lateral_mode, modes::LAT_NONE | modes::LAT_HDG | modes::LAT_TRACK | modes::LAT_GA_TRACK | modes::LAT_RWY_TRACK),
                };
                if transmit_active {
                    self.transmit_plan(index, plan, side, ctx, iface, sink, "ACTIVE", "MISSED", "ALTERNATE", Some("ACTIVE_EOSID"));
                    Self::transmit(sink, side, "DASHED", None);
                } else {
                    Self::transmit(sink, side, "ACTIVE", None);
                    self.transmit_plan(index, plan, side, ctx, iface, sink, "DASHED", "MISSED", "ALTERNATE", Some("ACTIVE_EOSID"));
                }
            }
            1 => self.transmit_plan(index, plan, side, ctx, iface, sink, "TEMPORARY", "TEMPORARY", "TEMPORARY", None),
            _ => {
                if iface.should_transmit_secondary(index - FIRST_SECONDARY + 1) {
                    self.transmit_plan(index, plan, side, ctx, iface, sink, "SECONDARY", "SECONDARY", "SECONDARY", None);
                } else if !iface.should_transmit_any_secondary() {
                    Self::transmit(sink, side, "SECONDARY", None);
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn transmit_plan(&mut self, index: usize, plan: &FlightPlan, side: usize, ctx: &VectorsContext, iface: &EfisInterface, sink: &mut Sink, main: &str, missed_group: &str, altn_group: &str, eosid_group: Option<&str>) {
        let plan_mode = ctx.nd_mode[side] == ND_MODE_PLAN;
        let Some(geometry) = ctx.geometries.get(&(index, false)) else {
            Self::transmit(sink, side, main, None);
            if missed_group != main {
                Self::transmit(sink, side, missed_group, None);
            }
            if altn_group != main {
                Self::transmit(sink, side, altn_group, None);
            }
            if let Some(g) = eosid_group {
                Self::transmit(sink, side, g, None);
            }
            return;
        };
        let mut vectors: Vec<PathVector> = plan_path_vectors(plan, geometry, false).into_iter().filter(vector_reasonable).collect();
        if iface.should_transmit_missed(index, plan, plan_mode) {
            let missed: Vec<PathVector> = plan_path_vectors(plan, geometry, true).into_iter().filter(vector_reasonable).collect();
            if missed_group == main {
                vectors.extend(missed);
            } else {
                Self::transmit(sink, side, missed_group, Some(vectors_json(&missed)));
            }
        } else if missed_group != main {
            Self::transmit(sink, side, missed_group, None);
        }
        if let Some(g) = eosid_group {
            if iface.should_transmit_eosid(index, plan_mode, ctx.engine_out_active, departure_procedure_active(plan), eosid_in_plans(ctx.plans)) {
                let v = ctx.eosid.map(|e| engine_out_vectors(plan, e)).unwrap_or_default();
                Self::transmit(sink, side, g, Some(vectors_json(&v)));
            } else {
                Self::transmit(sink, side, g, None);
            }
        }
        Self::transmit(sink, side, main, Some(vectors_json(&vectors)));
        if iface.should_transmit_alternate(index, plan, plan_mode) {
            if let Some(altn_geometry) = ctx.geometries.get(&(index, true)) {
                let mut altn: Vec<PathVector> = altn_geometry.all_path_vectors(Some(0), false).into_iter().filter(vector_reasonable).collect();
                if iface.should_transmit_alternate_missed(index, plan, plan_mode) {
                    altn.extend(altn_geometry.all_path_vectors(Some(0), true).into_iter().filter(vector_reasonable));
                }
                if altn_group != main {
                    Self::transmit(sink, side, altn_group, Some(vectors_json(&altn)));
                }
            } else if altn_group != main {
                Self::transmit(sink, side, altn_group, None);
            }
        } else if altn_group != main {
            Self::transmit(sink, side, altn_group, None);
        }
    }
}

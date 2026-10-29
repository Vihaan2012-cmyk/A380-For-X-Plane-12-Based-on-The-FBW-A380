//! FwsCore mapped subjects the generator leaves to hand translation (spread arguments, `Date.now()` time stamps,
//! `fwsNumber`, `Array.of`).

use crate::fwc::Core;
use crate::js::js_round;

pub fn mapped_fire_eng1_agent1_discharge10s_time_stamp(c: &Core) -> Option<f64> {
    c.fire_timestamp(0)
}

pub fn mapped_fire_eng1_agent2_discharge30s_time_stamp(c: &Core) -> Option<f64> {
    c.fire_timestamp(1)
}

pub fn mapped_fire_eng2_agent1_discharge10s_time_stamp(c: &Core) -> Option<f64> {
    c.fire_timestamp(2)
}

pub fn mapped_fire_eng2_agent2_discharge30s_time_stamp(c: &Core) -> Option<f64> {
    c.fire_timestamp(3)
}

pub fn mapped_fire_eng3_agent1_discharge10s_time_stamp(c: &Core) -> Option<f64> {
    c.fire_timestamp(4)
}

pub fn mapped_fire_eng3_agent2_discharge30s_time_stamp(c: &Core) -> Option<f64> {
    c.fire_timestamp(5)
}

pub fn mapped_fire_eng4_agent1_discharge10s_time_stamp(c: &Core) -> Option<f64> {
    c.fire_timestamp(6)
}

pub fn mapped_fire_eng4_agent2_discharge30s_time_stamp(c: &Core) -> Option<f64> {
    c.fire_timestamp(7)
}

pub fn mapped_apu_fire_agent1_discharge10_seconds_timestamp(c: &Core) -> Option<f64> {
    c.fire_timestamp(8)
}

pub fn mapped_all_sec_fault_condition(c: &Core) -> bool {
    c.s.sec1_fault_condition && c.s.sec2_fault_condition && c.s.sec3_fault_condition
}

pub fn mapped_all_feed_tank_pumps_off(c: &Core) -> bool {
    !c.feed_tank_pumps.iter().any(|p| *p)
}

pub fn mapped_all_feed_tank_pumps_on(c: &Core) -> bool {
    c.feed_tank_pumps.iter().all(|p| *p)
}

pub fn mapped_cross_feed_open_memo(c: &Core) -> bool {
    [c.s.cross_feed1_valve_open, c.s.cross_feed2_valve_open, c.s.cross_feed3_valve_open, c.s.cross_feed4_valve_open].iter().filter(|v| **v).count() >= 2
}

pub fn mapped_computed_air_speed_to_nearest2(c: &Core) -> f64 {
    let w = if c.air_data_capt_on3() { c.s.adr3_cas } else { c.s.adr1_cas };
    js_round(w.value() / 2.) * 2.
}

pub fn mapped_mach_selected_from_adr(c: &Core) -> f64 {
    if c.air_data_capt_on3() {
        c.s.adr3_mach.value()
    } else {
        c.s.adr1_mach.value()
    }
}

pub fn mapped_two_ir_fault(c: &Core) -> bool {
    [c.s.ir1_fault, c.s.ir2_fault, c.s.ir3_fault].iter().filter(|v| **v).count() >= 2
}

pub fn mapped_attention_getter_normal_collection(_c: &Core) -> bool {
    false
}

pub fn mapped_attention_getter_abnormal_primary_params(_c: &Core) -> bool {
    false
}

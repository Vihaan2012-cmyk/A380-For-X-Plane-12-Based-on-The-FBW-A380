use std::ffi::{c_int, c_void, CString};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::OnceLock;

use systems::simulation::{SimulatorReaderWriter, VariableIdentifier, VariableRegistry};

use crate::Vars;

const POTS: [usize; 9] = [7, 8, 9, 10, 11, 76, 83, 96, 97];
const PARAMS: c_int = 9;
const ALPHA: c_int = 3;

static LEVELS: [AtomicU32; POTS.len()] = [const { AtomicU32::new(0) }; POTS.len()];
static NAMES: OnceLock<Vec<CString>> = OnceLock::new();

pub fn processor_name(pot: usize) -> String {
    format!("fbw/spill/LIGHT_POTENTIOMETER_{pot}")
}

pub unsafe fn scale_alpha(level: f32, values: *mut f32, offset: c_int, max: c_int) -> c_int {
    if values.is_null() {
        return PARAMS;
    }
    let n = max.min(PARAMS - offset).max(0);
    if offset <= ALPHA && ALPHA < offset + n {
        let a = values.add((ALPHA - offset) as usize);
        *a *= level.clamp(0.0, 1.0);
    }
    n
}

unsafe extern "C" fn get_vf(refcon: *mut c_void, values: *mut f32, offset: c_int, max: c_int) -> c_int {
    let level = LEVELS.get(refcon as usize).map_or(0.0, |v| f32::from_bits(v.load(Ordering::Relaxed)));
    scale_alpha(level, values, offset, max)
}

#[derive(Default)]
pub struct SpillLights {
    ids: Option<([VariableIdentifier; POTS.len()], VariableIdentifier)>,
}

impl SpillLights {
    pub fn update(&mut self, vars: &mut Vars) {
        if self.ids.is_none() {
            let names = NAMES.get_or_init(|| POTS.iter().map(|&p| CString::new(processor_name(p)).unwrap_or_default()).collect());
            let accessors = crate::xp::Accessors { get_vf: Some(get_vf), ..Default::default() };
            for (i, name) in names.iter().enumerate() {
                let _ = crate::xp::register_owned_accessor(name, crate::xp::DATA_TYPE_FLOAT_ARRAY, false, &accessors, i as *mut c_void);
            }
        }
        let (pots, panel) = &*self.ids.get_or_insert_with(|| (POTS.map(|p| vars.get(format!("LIGHT POTENTIOMETER:{p}"))), vars.get("LIGHT PANEL".to_string())));
        let powered = if vars.read(panel) != 0. { 1.0 } else { 0.0 };
        for (i, id) in pots.iter().enumerate() {
            let level = (vars.read(id) * powered).clamp(0.0, 1.0) as f32;
            LEVELS[i].store(level.to_bits(), Ordering::Relaxed);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scales_only_the_alpha_of_the_lights_parameters() {
        let mut p = [1.0, 0.9, 0.75, 0.7, 2.0, 0.0, -1.0, 0.0, 0.4];
        let n = unsafe { scale_alpha(0.5, p.as_mut_ptr(), 0, 9) };
        assert_eq!(n, 9);
        assert_eq!(p, [1.0, 0.9, 0.75, 0.35, 2.0, 0.0, -1.0, 0.0, 0.4]);
    }

    #[test]
    fn a_dark_knob_zeroes_the_light_and_a_null_read_reports_the_size() {
        let mut p = [1.0; 9];
        unsafe { scale_alpha(0.0, p.as_mut_ptr(), 0, 9) };
        assert_eq!(p[3], 0.0);
        assert_eq!(unsafe { scale_alpha(1.0, std::ptr::null_mut(), 0, 0) }, PARAMS);
    }

    #[test]
    fn a_partial_read_past_alpha_leaves_values_alone() {
        let mut p = [1.0; 5];
        let n = unsafe { scale_alpha(0.0, p.as_mut_ptr(), 4, 5) };
        assert_eq!(n, 5);
        assert_eq!(p, [1.0; 5]);
    }
}

//! How far each cockpit screen is minified on the monitor. X-Plane samples an
//! avionics device's texture without mips, so a 768x1024 screen seen about
//! 450 pixels tall aliases; `gl.rs` prefilters the page to the size it is seen
//! at, and the device texture then holds nothing finer than the monitor shows.

use crate::xp::{DataRef, Xplm};

/// The chain is built from 1.5x pages, so level 3 is about 5x below a device.
const MAX_LOD: f32 = 3.0;

/// Device texels per metre of glass and the glass centre in the OBJ frame
/// (`a380_cockpit_003.obj`, the same frame as `pilots_head_*`). The FCU, rudder
/// trim and battery strips stretch their pages unevenly and are left out.
const GLASS: &[(&str, f32, [f32; 3])] = &[
    ("SCREEN_DU_PFDL", 4897., [-0.559, 2.690, -32.932]),
    ("SCREEN_DU_NDL", 4897., [-0.369, 2.690, -32.932]),
    ("SCREEN_DU_PFDR", 4897., [0.556, 2.690, -32.932]),
    ("SCREEN_DU_NDR", 4897., [0.366, 2.690, -32.932]),
    ("SCREEN_DU_EWD", 4897., [-0.005, 2.689, -32.936]),
    ("SCREEN_DU_SD", 4850., [0.001, 2.485, -32.784]),
    ("SCREEN_DU_MFD", 4850., [-0.192, 2.485, -32.784]),
    ("SCREEN_EFB", 5900., [0.957, 2.841, -32.428]),
    ("SCREEN_OIT_LEFT", 4500., [-0.928, 2.613, -32.768]),
    ("SCREEN_OIT_RIGHT", 4500., [0.929, 2.613, -32.768]),
    ("SCREEN_ISIS_1", 7900., [-0.159, 2.761, -32.953]),
    ("SCREEN_ISIS_2", 7900., [-0.159, 2.674, -32.922]),
    ("Clock", 4547., [0.211, 2.634, -32.902]),
    ("SCREEN_DU_RMP_1", 17700., [-0.181, 2.398, -32.383]),
    ("SCREEN_DU_RMP_2", 17700., [0.182, 2.398, -32.383]),
    ("SCREEN_DU_RMP_3", 17700., [0.238, 3.576, -31.987]),
];

/// Device texels behind one monitor pixel.
pub fn minification(texels_per_m: f32, distance_m: f32, focal_px: f32) -> f32 {
    texels_per_m * distance_m / focal_px
}

/// The mip bias that removes what one monitor pixel cannot show: none when
/// the screen is magnified or zoomed to one texel per pixel.
pub fn prefilter_lod(minified: f32) -> f32 {
    minified.max(1.).log2().min(MAX_LOD)
}

pub struct ViewScale {
    head: [Option<DataRef>; 3],
    projection: Option<DataRef>,
    window_height: Option<DataRef>,
    head_m: [f32; 3],
    focal_px: f32,
}

impl ViewScale {
    pub fn new(xplm: Option<&Xplm>) -> Self {
        let find = |name: &str| xplm.and_then(|x| x.find(name));
        Self {
            head: [find("sim/graphics/view/pilots_head_x"), find("sim/graphics/view/pilots_head_y"), find("sim/graphics/view/pilots_head_z")],
            projection: find("sim/graphics/view/projection_matrix_3d"),
            window_height: find("sim/graphics/view/window_height"),
            head_m: [0.; 3],
            focal_px: 0.,
        }
    }

    /// Once per frame: the projection dataref keeps its value outside 3-D drawing.
    pub fn refresh(&mut self, xplm: &Xplm) {
        for (value, dataref) in self.head_m.iter_mut().zip(self.head) {
            *value = dataref.map_or(0., |d| xplm.get_f(d));
        }
        let mut matrix = [0f32; 16];
        let filled = self.projection.map_or(0, |d| xplm.get_vf(d, &mut matrix));
        let height = self.window_height.map_or(0, |d| xplm.get_i(d));
        self.focal_px = if filled == 16 { matrix[5].abs() * height as f32 * 0.5 } else { 0. };
    }

    pub fn lod(&self, screen_id: &str) -> f32 {
        let Some((_, texels_per_m, centre)) = GLASS.iter().find(|g| g.0 == screen_id) else { return 0. };
        if self.focal_px <= 0. {
            return 0.;
        }
        let distance = self.head_m.iter().zip(centre).map(|(a, b)| (a - b) * (a - b)).sum::<f32>().sqrt();
        prefilter_lod(minification(*texels_per_m, distance, self.focal_px))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_captain_pfd_at_the_default_eye_and_60_degrees_needs_about_one_level() {
        let eye = [-0.524f32, 3.158, -32.07];
        let glass = GLASS[0].2;
        let distance = eye.iter().zip(glass).map(|(a, b)| (a - b) * (a - b)).sum::<f32>().sqrt();
        let focal = 3.08 * 1440. * 0.5;
        let lod = prefilter_lod(minification(GLASS[0].1, distance, focal));
        assert!((lod - 1.1).abs() < 0.15, "{lod}");
    }

    #[test]
    fn zooming_in_stops_filtering_and_a_far_view_stays_bounded() {
        assert_eq!(prefilter_lod(0.5), 0.);
        assert_eq!(prefilter_lod(1.0), 0.);
        assert_eq!(prefilter_lod(100.), MAX_LOD);
    }

    #[test]
    fn a_screen_with_no_glass_entry_is_not_filtered() {
        let v = ViewScale { head: [None; 3], projection: None, window_height: None, head_m: [0.; 3], focal_px: 2000. };
        assert_eq!(v.lod("FCU"), 0.);
        assert!(v.lod("SCREEN_DU_SD") > 0.);
    }
}

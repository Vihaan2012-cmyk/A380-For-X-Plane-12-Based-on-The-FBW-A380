//! Polar-to-image: a set of geo-referenced returns, rasterised into the
//! straight RGBA buffer opcode 60 NATIVE_IMAGE draws (docs/display-stream.md).
//! Each level is a field of summed Gaussian bumps, one per return at or
//! above that level, drawn where the field crosses one half with a soft
//! edge, so neighbouring returns merge into rounded cells as on the real
//! radar picture; worse levels draw last, over milder ones.

use super::geometry::screen_offset;
use super::levels::Level;

/// One classified, geo-referenced return.
#[derive(Clone, Copy, Debug)]
pub struct Return {
    pub lat: f64,
    pub lon: f64,
    pub level: Level,
}

/// What the image is drawn for: the aircraft's current position and
/// heading, the ND's selected range, and the canvas the image fills.
pub struct View {
    pub own_lat: f64,
    pub own_lon: f64,
    pub heading_true_deg: f64,
    pub range_nm: f64,
    pub half_sector_deg: f64,
    pub width: u32,
    pub height: u32,
    /// Ownship's pixel, within the canvas (ARC mode has it low and centred;
    /// ROSE modes have it in the middle -- `wxr/mod.rs`'s `ARC_CENTER_PX`
    /// / `ROSE_CENTER_PX`).
    pub center_px: (f64, f64),
    /// Range at the edge of the visible fan, in pixels.
    pub max_radius_px: f64,
    /// Half the spacing of the sampled grid on screen, in pixels: each
    /// return's bump is this wide, so adjoining cells merge.
    pub cell_half_px: f64,
}

impl View {
    fn px_per_nm(&self) -> f64 {
        if self.range_nm > 0. {
            self.max_radius_px / self.range_nm
        } else {
            0.
        }
    }
}

/// Straight RGBA, row 0 at the top, `width * height * 4` bytes, transparent
/// where nothing was painted.
const LEVELS: [Level; 4] = [Level::Green, Level::Amber, Level::Red, Level::Magenta];
const EDGE: f32 = 0.5;
const EDGE_SOFTNESS: f32 = 0.12;

pub fn rasterize(returns: &[Return], view: &View) -> Vec<u8> {
    let (width, height) = (view.width as usize, view.height as usize);
    let mut rgba = vec![0u8; width * height * 4];
    let px_per_nm = view.px_per_nm();
    let sigma = view.cell_half_px.max(1.);
    let step = (sigma / 2.).floor().clamp(1., 4.) as usize;
    let (gw, gh) = (width / step + 2, height / step + 2);
    let mut fields = vec![vec![0f32; gw * gh]; LEVELS.len()];
    let reach = 3. * sigma;
    let inv = 1. / (2. * sigma * sigma);
    for r in returns.iter().filter(|r| !r.level.is_none()) {
        let Some((dx, dy)) = screen_offset(r.lat, r.lon, view.own_lat, view.own_lon, view.heading_true_deg, view.range_nm, px_per_nm, view.half_sector_deg) else {
            continue;
        };
        let (cx, cy) = (view.center_px.0 + dx, view.center_px.1 + dy);
        let top = LEVELS.iter().position(|&l| l == r.level).unwrap_or(0);
        let gx0 = ((cx - reach) / step as f64).floor().max(0.) as usize;
        let gx1 = (((cx + reach) / step as f64).ceil() as usize).min(gw - 1);
        let gy0 = ((cy - reach) / step as f64).floor().max(0.) as usize;
        let gy1 = (((cy + reach) / step as f64).ceil() as usize).min(gh - 1);
        for gy in gy0..=gy1 {
            let ddy = (gy * step) as f64 - cy;
            for gx in gx0..=gx1 {
                let ddx = (gx * step) as f64 - cx;
                let w = (-(ddx * ddx + ddy * ddy) * inv).exp() as f32;
                if w < 1e-3 {
                    continue;
                }
                for field in fields.iter_mut().take(top + 1) {
                    field[gy * gw + gx] += w;
                }
            }
        }
    }
    let sample = |field: &[f32], x: usize, y: usize| -> f32 {
        let fx = x as f32 / step as f32;
        let fy = y as f32 / step as f32;
        let (x0, y0) = (fx.floor() as usize, fy.floor() as usize);
        let (tx, ty) = (fx - x0 as f32, fy - y0 as f32);
        let at = |gx: usize, gy: usize| field[gy.min(gh - 1) * gw + gx.min(gw - 1)];
        let top = at(x0, y0) * (1. - tx) + at(x0 + 1, y0) * tx;
        let bottom = at(x0, y0 + 1) * (1. - tx) + at(x0 + 1, y0 + 1) * tx;
        top * (1. - ty) + bottom * ty
    };
    let half_sector = view.half_sector_deg.to_radians();
    for y in 0..height {
        for x in 0..width {
            let (ox, oy) = (x as f64 + 0.5 - view.center_px.0, y as f64 + 0.5 - view.center_px.1);
            if ox.hypot(oy) > view.max_radius_px || ox.atan2(-oy).abs() > half_sector {
                continue;
            }
            let i = (y * width + x) * 4;
            for (level, field) in LEVELS.iter().zip(&fields) {
                let a = ((sample(field, x, y) - EDGE) / EDGE_SOFTNESS + 0.5).clamp(0., 1.);
                if a <= 0. {
                    break;
                }
                let colour = level.rgba();
                for c in 0..3 {
                    rgba[i + c] = (colour[c] as f32 * a + rgba[i + c] as f32 * (1. - a)).round() as u8;
                }
                rgba[i + 3] = (255. * a + rgba[i + 3] as f32 * (1. - a)).round() as u8;
            }
        }
    }
    rgba
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view() -> View {
        View {
            own_lat: 0.,
            own_lon: 0.,
            heading_true_deg: 0.,
            range_nm: 40.,
            half_sector_deg: 60.,
            width: 100,
            height: 100,
            center_px: (50., 80.),
            max_radius_px: 70.,
            cell_half_px: 4.,
        }
    }

    fn pixel(rgba: &[u8], width: u32, x: i32, y: i32) -> [u8; 4] {
        let i = ((y as u32 * width + x as u32) * 4) as usize;
        [rgba[i], rgba[i + 1], rgba[i + 2], rgba[i + 3]]
    }

    #[test]
    fn an_empty_grid_is_fully_transparent() {
        let v = view();
        let rgba = rasterize(&[], &v);
        assert!(rgba.iter().all(|&b| b == 0));
    }

    #[test]
    fn a_return_dead_ahead_paints_above_ownship() {
        let v = view();
        // 10 nm north of an aircraft at the origin heading north: dead ahead.
        let returns = [Return { lat: 10. / 60., lon: 0., level: Level::Green }];
        let rgba = rasterize(&returns, &v);
        // Ownship is at (50, 80); 10 nm at 70 px / 40 nm is 17.5 px up.
        let p = pixel(&rgba, v.width, 50, 80 - 18);
        assert_eq!(p, Level::Green.rgba());
    }

    #[test]
    fn a_worse_level_paints_over_a_milder_one_at_the_same_spot() {
        let mut v = view();
        v.cell_half_px = 20.; // force the two squares to overlap fully
        let returns = [
            Return { lat: 10. / 60., lon: 0., level: Level::Green },
            Return { lat: 10. / 60., lon: 0.01, level: Level::Magenta },
        ];
        let rgba = rasterize(&returns, &v);
        let p = pixel(&rgba, v.width, 50, 80 - 18);
        assert_eq!(p, Level::Magenta.rgba());
    }

    #[test]
    fn neighbouring_returns_merge_and_the_edge_is_soft() {
        let mut v = view();
        v.cell_half_px = 6.;
        let returns = [
            Return { lat: 10. / 60., lon: -1.5 / 60., level: Level::Green },
            Return { lat: 10. / 60., lon: 1.5 / 60., level: Level::Green },
        ];
        let rgba = rasterize(&returns, &v);
        assert_eq!(pixel(&rgba, v.width, 50, 80 - 18), Level::Green.rgba());
        let alphas: Vec<u8> = (0..40).map(|dy| pixel(&rgba, v.width, 50, 80 - 18 - dy)[3]).collect();
        assert!(alphas.iter().any(|&a| a > 0 && a < 255), "{alphas:?}");
    }

    #[test]
    fn behind_the_aircraft_is_never_painted() {
        let v = view();
        let returns = [Return { lat: -10. / 60., lon: 0., level: Level::Red }];
        let rgba = rasterize(&returns, &v);
        assert!(rgba.iter().all(|&b| b == 0));
    }
}

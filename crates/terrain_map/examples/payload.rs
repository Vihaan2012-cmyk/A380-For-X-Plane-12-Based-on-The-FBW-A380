use terrain_map::alert::{AlertArea, AlertLevel};
use terrain_map::bands::{classify, ClassifyInputs};
use terrain_map::grid::{GeoGrid, Probe};
use terrain_map::profile::{cut_width_nm, profile_along};
use terrain_map::publish::{nd_json, vd_json};
use terrain_map::TerrainMap;

fn synthetic(north: f64, east: f64) -> Probe {
    if east < -12. {
        return Probe::Hit { elevation_ft: 0., water: true };
    }
    let ridge = (-((north - 18.) / 4.).powi(2)).exp() * 9000.;
    let hills = 1500. * ((north / 3.).sin() * (east / 4.).cos()).abs();
    Probe::Hit { elevation_ft: 800. + ridge + hills, water: false }
}

fn main() {
    let a: Vec<f64> = std::env::args().skip(1).map(|s| s.parse().expect("number")).collect();
    let [lat, lon, alt, track, range, vs, alert]: [f64; 7] = a.try_into().expect("lat lon alt_ft track_deg range_nm vs_fpm alert(0/1/2)");
    let mut map = TerrainMap::default();
    for _ in 0..400 {
        let reqs = map.plan(lat, lon, track, range, 20000);
        if reqs.is_empty() {
            break;
        }
        for r in reqs {
            let (n, e) = GeoGrid::ne_nm(lat, lon, r.lat, r.lon);
            map.store(&r, synthetic(n, e));
        }
    }
    let level = map.level_for(range);
    let alert = AlertArea {
        level: [AlertLevel::None, AlertLevel::Caution, AlertLevel::Warning][alert as usize],
        lat,
        lon,
        alt_ft: alt,
        vs_fpm: vs,
        gs_kt: 300.,
        track_deg: track,
        caution_time_s: 60.,
        warning_time_s: 30.,
        half_angle_deg: 4.,
        start_width_nm: 0.2,
        clearance_ft: 500.,
        max_lookahead_nm: 12.,
    };
    let inp = ClassifyInputs { lat, lon, alt_ft: alt, vs_fpm: vs, gear_down: false, range_nm: range, arc_up_deg: Some(track), cut_off_ft: -500. };
    let img = classify(&map.levels[level], &inp, &alert);
    let end = GeoGrid::offset(lat, lon, range * track.to_radians().cos(), range * track.to_radians().sin());
    let step = range / 270.;
    let prof = profile_along(&map, &[(lat, lon), end], range, step, cut_width_nm(2.));
    println!("{}", nd_json(&img, 1));
    println!("{}", vd_json(&prof, step, None, 1));
}

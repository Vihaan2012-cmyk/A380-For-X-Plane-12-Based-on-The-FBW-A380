//! Runway data for the EFB performance calculators, read from X-Plane's own
//! airport database: Custom Scenery airports first, then Global Airports.
//! The first request indexes every airport's byte offset (about 380 MB of
//! apt.dat, a few seconds); later requests seek straight to the airport.

use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

struct Index {
    files: Vec<PathBuf>,
    airports: HashMap<String, (usize, u64)>,
    places: HashMap<String, (f64, f64)>,
}

static INDEX: OnceLock<Mutex<Option<Index>>> = OnceLock::new();

fn xplane_root(start: &[&Path]) -> Option<PathBuf> {
    start.iter().flat_map(|p| p.ancestors()).find(|d| d.join("Global Scenery").join("Global Airports").is_dir()).map(Path::to_path_buf)
}

fn apt_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(root.join("Custom Scenery")) {
        let mut dirs: Vec<_> = rd.flatten().map(|e| e.path()).collect();
        dirs.sort();
        for d in dirs {
            let f = d.join("Earth nav data").join("apt.dat");
            if f.is_file() {
                out.push(f);
            }
        }
    }
    out.push(root.join("Global Scenery").join("Global Airports").join("Earth nav data").join("apt.dat"));
    out
}

fn build(root: &Path) -> Index {
    let files = apt_files(root);
    let mut airports = HashMap::new();
    let mut places: HashMap<String, (f64, f64)> = HashMap::new();
    for (n, f) in files.iter().enumerate() {
        let mut current: Option<String> = None;
        let Ok(file) = File::open(f) else { continue };
        let mut r = BufReader::with_capacity(1 << 20, file);
        let mut line = Vec::new();
        let mut pos = 0u64;
        loop {
            line.clear();
            let Ok(len) = r.read_until(b'\n', &mut line) else { break };
            if len == 0 {
                break;
            }
            if line.starts_with(b"1 ") || line.starts_with(b"16 ") || line.starts_with(b"17 ") {
                let text = String::from_utf8_lossy(&line);
                current = text.split_whitespace().nth(4).map(str::to_ascii_uppercase);
                if let Some(icao) = &current {
                    airports.entry(icao.clone()).or_insert((n, pos));
                }
            } else if line.starts_with(b"100 ") {
                if let Some(icao) = current.take() {
                    let text = String::from_utf8_lossy(&line);
                    let t: Vec<&str> = text.split_whitespace().collect();
                    if let (Some(Ok(lat)), Some(Ok(lon))) = (t.get(9).map(|v| v.parse::<f64>()), t.get(10).map(|v| v.parse::<f64>())) {
                        places.entry(icao).or_insert((lat, lon));
                    }
                }
            }
            pos += len as u64;
        }
    }
    Index { files, airports, places }
}

fn heading(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let (p1, p2, dl) = (lat1.to_radians(), lat2.to_radians(), (lon2 - lon1).to_radians());
    let y = dl.sin() * p2.cos();
    let x = p1.cos() * p2.sin() - p1.sin() * p2.cos() * dl.cos();
    (y.atan2(x).to_degrees() + 360.0) % 360.0
}

fn distance_m(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let (p1, p2) = (lat1.to_radians(), lat2.to_radians());
    let a = ((p2 - p1) / 2.0).sin().powi(2) + p1.cos() * p2.cos() * ((lon2 - lon1).to_radians() / 2.0).sin().powi(2);
    2.0 * 6_371_000.0 * a.sqrt().asin()
}

fn surface(code: &str) -> &'static str {
    match code {
        "1" => "ASPHALT",
        "2" => "CONCRETE",
        "3" => "GRASS",
        "4" => "DIRT",
        "5" => "GRAVEL",
        "12" => "DRY LAKEBED",
        "13" => "WATER",
        "14" => "SNOW",
        _ if code.starts_with('2') && code.len() == 2 => "ASPHALT",
        _ if code.starts_with('5') && code.len() == 2 => "CONCRETE",
        _ => "HARD",
    }
}

fn read_airport(path: &Path, pos: u64) -> Option<Value> {
    let mut r = BufReader::new(File::open(path).ok()?);
    r.seek(SeekFrom::Start(pos)).ok()?;
    let mut line = String::new();
    r.read_line(&mut line).ok()?;
    let head: Vec<&str> = line.split_whitespace().collect();
    let elev: f64 = head.get(1)?.parse().ok()?;
    let icao = head.get(4)?.to_string();
    let name = head.get(5..).map(|v| v.join(" ")).unwrap_or_default();
    let mut runways = Vec::new();
    loop {
        line.clear();
        if r.read_line(&mut line).ok()? == 0 {
            break;
        }
        let t: Vec<&str> = line.split_whitespace().collect();
        match t.first().copied() {
            Some("1" | "16" | "17" | "99") => break,
            Some("100") if t.len() >= 26 => {
                let width: f64 = t[1].parse().unwrap_or(45.0);
                let surf = surface(t[2]);
                let end = |i: usize| -> Option<(String, f64, f64, f64, f64)> {
                    Some((t[i].to_string(), t[i + 1].parse().ok()?, t[i + 2].parse().ok()?, t[i + 3].parse().unwrap_or(0.0), t[i + 4].parse().unwrap_or(0.0)))
                };
                let (Some(a), Some(b)) = (end(8), end(17)) else { continue };
                let len = distance_m(a.1, a.2, b.1, b.2);
                for (from, to) in [(&a, &b), (&b, &a)] {
                    runways.push(json!({
                        "id": from.0,
                        "heading": (heading(from.1, from.2, to.1, to.2) * 10.0).round() / 10.0,
                        "length_m": len.round(),
                        "tora_m": len.round(),
                        "toda_m": (len + to.4.max(0.0)).round(),
                        "asda_m": (len + to.4.max(0.0)).round(),
                        "lda_m": (len - from.3).max(0.0).round(),
                        "displaced_m": from.3.round(),
                        "width_m": width,
                        "surface": surf,
                        "lat": from.1,
                        "lon": from.2,
                    }));
                }
            }
            _ => {}
        }
    }
    Some(json!({ "icao": icao, "name": name, "elevation_ft": elev, "runways": runways }))
}

fn with_index<T>(starts: &[&Path], f: impl FnOnce(&Index) -> T) -> Result<T, Value> {
    let cell = INDEX.get_or_init(|| Mutex::new(None));
    let mut guard = cell.lock().unwrap_or_else(|e| e.into_inner());
    if guard.is_none() {
        let Some(root) = xplane_root(starts) else { return Err(json!({ "error": "X-Plane folder not found" })) };
        *guard = Some(build(&root));
    }
    Ok(f(guard.as_ref().expect("built above")))
}

pub fn nearest(starts: &[&Path], lat: f64, lon: f64) -> Value {
    if !(lat.is_finite() && lon.is_finite()) || (lat == 0.0 && lon == 0.0) {
        return json!({ "error": "no position" });
    }
    let found = with_index(starts, |index| {
        index.places.iter().map(|(icao, &(a, b))| (distance_m(lat, lon, a, b), icao.clone())).min_by(|x, y| x.0.total_cmp(&y.0))
    });
    match found {
        Ok(Some((d, icao))) => json!({ "icao": icao, "distance_m": d.round() }),
        Ok(None) => json!({ "error": "no airports" }),
        Err(e) => e,
    }
}

pub fn lookup(starts: &[&Path], icao: &str) -> Value {
    let icao = icao.trim().to_ascii_uppercase();
    if icao.len() < 3 || icao.len() > 7 || !icao.chars().all(|c| c.is_ascii_alphanumeric()) {
        return json!({ "error": "bad airport code" });
    }
    let found = with_index(starts, |index| index.airports.get(&icao).map(|&(n, pos)| (index.files[n].clone(), pos)));
    match found {
        Ok(Some((path, pos))) => read_airport(&path, pos).unwrap_or_else(|| json!({ "error": "could not read the airport" })),
        Ok(None) => json!({ "error": format!("{icao} is not in X-Plane's airport database") }),
        Err(e) => e,
    }
}

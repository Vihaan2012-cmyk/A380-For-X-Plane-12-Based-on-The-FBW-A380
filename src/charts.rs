//! Chart sources for the EFB besides the AMDB Bridge: the user's own files
//! under Documents/Charts/<ICAO>, and the FAA's free d-TPP (US airports),
//! whose per-cycle index is fetched once with curl.

use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::Command;

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn civil_year(days: i64) -> i64 {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    yoe + era * 400 + i64::from(m <= 2)
}

pub fn airac_cycle(today_days: i64) -> String {
    let epoch = days_from_civil(2020, 1, 2);
    let start = epoch + (today_days - epoch).div_euclid(28) * 28;
    let year = civil_year(start);
    let jan1 = days_from_civil(year, 1, 1);
    let first = epoch + (jan1 - epoch + 27).div_euclid(28) * 28;
    format!("{:02}{:02}", year % 100, (start - first) / 28 + 1)
}

fn today_days() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| (d.as_secs() / 86_400) as i64).unwrap_or(0)
}

pub fn local(documents: &Path, icao: &str, file_url: &str) -> Value {
    let dir = documents.join("Charts").join(icao);
    let mut charts = Vec::new();
    fn walk(dir: &Path, rel: &str, cat: &str, file_url: &str, out: &mut Vec<Value>) {
        let mut entries: Vec<_> = std::fs::read_dir(dir).map(|r| r.flatten().map(|e| e.path()).collect()).unwrap_or_default();
        entries.sort();
        for p in entries {
            let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            let child = format!("{rel}/{name}");
            if p.is_dir() {
                walk(&p, &child, &name, file_url, out);
                continue;
            }
            let ext = p.extension().map(|x| x.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
            if matches!(ext.as_str(), "pdf" | "png" | "jpg" | "jpeg" | "webp") {
                let stem = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or(name);
                out.push(json!({ "id": child, "name": stem, "category": cat, "kind": if ext == "pdf" { "pdf" } else { "image" }, "url": format!("{file_url}{}", child.replace(' ', "%20")) }));
            }
        }
    }
    walk(&dir, &format!("Charts/{icao}"), "Charts", file_url, &mut charts);
    if charts.is_empty() {
        let _ = std::fs::create_dir_all(&dir);
        return json!({ "error": format!("No charts in Documents/Charts/{icao}. Put PDF or image files there (subfolders become sections).") });
    }
    json!({ "charts": charts })
}

fn cache_dir() -> PathBuf {
    std::env::var("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(|_| std::env::temp_dir()).join("XPHFBW").join("faa")
}

fn tag<'a>(s: &'a str, name: &str) -> Option<&'a str> {
    let open = format!("<{name}>");
    let a = s.find(&open)? + open.len();
    let b = s[a..].find(&format!("</{name}>"))? + a;
    Some(s[a..b].trim())
}

fn category(code: &str) -> &'static str {
    match code {
        "APD" => "Airport Diagram",
        "IAP" => "Approach",
        "DP" | "ODP" => "Departure",
        "STAR" => "Arrival",
        "MIN" => "Minimums",
        "HOT" => "Hot Spots",
        "LAH" => "LAHSO",
        _ => "Other",
    }
}

pub fn faa(icao: &str) -> Value {
    let cycle = airac_cycle(today_days());
    let dir = cache_dir().join(&cycle);
    let meta = dir.join("d-tpp_Metafile.xml");
    if !meta.is_file() {
        let _ = std::fs::create_dir_all(&dir);
        let url = format!("https://aeronav.faa.gov/d-tpp/{cycle}/xml_data/d-tpp_Metafile.xml");
        let tmp = dir.join("d-tpp_Metafile.part");
        let ok = Command::new(if cfg!(windows) { "curl.exe" } else { "curl" }).args(["-s", "-f", "-L", "--max-time", "180", "-o"]).arg(&tmp).arg(&url).status().map(|s| s.success()).unwrap_or(false);
        if !ok || std::fs::rename(&tmp, &meta).is_err() {
            let _ = std::fs::remove_file(&tmp);
            return json!({ "error": format!("Could not download the FAA chart index for cycle {cycle}.") });
        }
    }
    let Ok(text) = std::fs::read_to_string(&meta) else { return json!({ "error": "Could not read the FAA chart index." }) };
    let key = format!("icao_ident=\"{icao}\"");
    let Some(at) = text.find(&key) else { return json!({ "error": format!("{icao} has no FAA charts (US airports only).") }) };
    let end = text[at..].find("</airport_name>").map(|e| at + e).unwrap_or(text.len());
    let block = &text[at..end];
    let mut charts = Vec::new();
    for rec in block.split("<record>").skip(1) {
        let (Some(code), Some(name), Some(pdf)) = (tag(rec, "chart_code"), tag(rec, "chart_name"), tag(rec, "pdf_name")) else { continue };
        if pdf.is_empty() || pdf.eq_ignore_ascii_case("DELETED_JOB.PDF") {
            continue;
        }
        charts.push(json!({ "id": pdf, "name": name, "category": category(code), "kind": "pdf", "url": format!("https://aeronav.faa.gov/d-tpp/{cycle}/{pdf}") }));
    }
    json!({ "charts": charts, "cycle": cycle })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn airac_cycles() {
        assert_eq!(airac_cycle(days_from_civil(2020, 1, 2)), "2001");
        assert_eq!(airac_cycle(days_from_civil(2024, 1, 25)), "2401");
        assert_eq!(airac_cycle(days_from_civil(2024, 2, 21)), "2401");
        assert_eq!(airac_cycle(days_from_civil(2024, 2, 22)), "2402");
        assert_eq!(airac_cycle(days_from_civil(2025, 1, 23)), "2501");
        assert_eq!(airac_cycle(days_from_civil(2025, 1, 22)), "2413");
    }
}

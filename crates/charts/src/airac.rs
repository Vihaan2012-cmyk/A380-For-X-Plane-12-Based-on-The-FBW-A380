pub fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

pub fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

const EPOCH: (i64, i64, i64) = (2020, 1, 2);

pub fn cycle_start(today_days: i64) -> i64 {
    let epoch = days_from_civil(EPOCH.0, EPOCH.1, EPOCH.2);
    epoch + (today_days - epoch).div_euclid(28) * 28
}

pub fn cycle(today_days: i64) -> String {
    let epoch = days_from_civil(EPOCH.0, EPOCH.1, EPOCH.2);
    let start = cycle_start(today_days);
    let year = civil_from_days(start).0;
    let jan1 = days_from_civil(year, 1, 1);
    let first = epoch + (jan1 - epoch + 27).div_euclid(28) * 28;
    format!("{:02}{:02}", year % 100, (start - first) / 28 + 1)
}

pub fn today_days() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| (d.as_secs() / 86_400) as i64).unwrap_or(0)
}

const MONTHS: [&str; 12] = ["JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC"];

/// Fills `{cycle}`, `{airac_iso}` (2026-10-01), `{airac_compact}` (20261001),
/// `{airac_dmy}` (01 OCT 2026), `{airac_us}` (2026_10_01) and `{icao}`.
pub fn fill(template: &str, icao: &str, today_days: i64) -> String {
    let start = cycle_start(today_days);
    let (y, m, d) = civil_from_days(start);
    template
        .replace("{icao}", icao)
        .replace("{cycle}", &cycle(today_days))
        .replace("{airac_iso}", &format!("{y:04}-{m:02}-{d:02}"))
        .replace("{airac_compact}", &format!("{y:04}{m:02}{d:02}"))
        .replace("{airac_dmy}", &format!("{d:02} {} {y:04}", MONTHS[(m - 1) as usize]))
        .replace("{airac_us}", &format!("{y:04}_{m:02}_{d:02}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cycles() {
        assert_eq!(cycle(days_from_civil(2020, 1, 2)), "2001");
        assert_eq!(cycle(days_from_civil(2024, 1, 25)), "2401");
        assert_eq!(cycle(days_from_civil(2024, 2, 21)), "2401");
        assert_eq!(cycle(days_from_civil(2024, 2, 22)), "2402");
        assert_eq!(cycle(days_from_civil(2025, 1, 23)), "2501");
        assert_eq!(cycle(days_from_civil(2025, 1, 22)), "2413");
    }

    #[test]
    fn civil_round_trip() {
        for (y, m, d) in [(2026, 10, 22), (2000, 2, 29), (1999, 12, 31)] {
            assert_eq!(civil_from_days(days_from_civil(y, m, d)), (y, m, d));
        }
    }

    #[test]
    fn templates() {
        let today = days_from_civil(2026, 10, 22);
        let (y, m, d) = civil_from_days(cycle_start(today));
        assert_eq!((y, m, d), (2026, 10, 1));
        assert_eq!(fill("{airac_iso}/{icao}/{cycle}", "EGLL", today), "2026-10-01/EGLL/2610");
        assert_eq!(fill("{airac_dmy}_{airac_us}", "EFHK", today), "01 OCT 2026_2026_10_01");
    }
}

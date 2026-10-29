const IN_HG_REGIONS: [&str; 8] = ["K", "C", "M", "P", "RJ", "RO", "TI", "TJ"];

pub enum BaroSelection {
    Fixed { is_hpa: bool },
    NeedsNearestAirport,
}

pub fn perform_selection(config_init_baro_unit: &str) -> BaroSelection {
    if config_init_baro_unit != "AUTO" {
        BaroSelection::Fixed { is_hpa: config_init_baro_unit == "HPA" }
    } else {
        BaroSelection::NeedsNearestAirport
    }
}

fn substring(s: &str, from: usize, to: usize) -> String {
    s.chars().skip(from).take(to.saturating_sub(from)).collect()
}

pub fn is_hpa_from_nearest(added: &[&str]) -> bool {
    let use_in_hg = match added.first() {
        Some(first) => {
            let one = substring(first, 7, 8);
            let two = substring(first, 7, 9);
            IN_HG_REGIONS.contains(&one.as_str()) || IN_HG_REGIONS.contains(&two.as_str())
        }
        None => false,
    };
    !use_in_hg
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_overrides_the_search() {
        assert!(matches!(perform_selection("HPA"), BaroSelection::Fixed { is_hpa: true }));
        assert!(matches!(perform_selection("INHG"), BaroSelection::Fixed { is_hpa: false }));
        assert!(matches!(perform_selection("AUTO"), BaroSelection::NeedsNearestAirport));
    }

    #[test]
    fn us_and_canadian_airports_use_inhg() {
        assert!(!is_hpa_from_nearest(&["AK1    KJFK"]));
        assert!(!is_hpa_from_nearest(&["ACY    CYYZ"]));
        assert!(!is_hpa_from_nearest(&["ACY    MMMX"]));
        assert!(!is_hpa_from_nearest(&["APH    PHNL"]));
    }

    #[test]
    fn two_letter_regions_use_inhg_and_others_hpa() {
        assert!(!is_hpa_from_nearest(&["AJ     RJTT"]));
        assert!(!is_hpa_from_nearest(&["AJ     ROAH"]));
        assert!(!is_hpa_from_nearest(&["AJ     TIST"]));
        assert!(!is_hpa_from_nearest(&["AJ     TJSJ"]));
        assert!(is_hpa_from_nearest(&["AEG    EGLL"]));
        assert!(is_hpa_from_nearest(&["AJ     RKSI"]));
    }

    #[test]
    fn nothing_found_or_short_ident_is_hpa() {
        assert!(is_hpa_from_nearest(&[]));
        assert!(is_hpa_from_nearest(&["AK1"]));
    }
}

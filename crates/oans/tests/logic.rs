use std::path::Path;

use oans::arinc::Arinc;
use oans::geo::{distance_to, place_bearing_distance, Coordinates};
use oans::labels::{feature_type as ft, filter_label, label_style, FmsRunways, Label, LabelStyle, OancLabelFilter};
use oans::oanc::{
    aircraft_on_ground, compute_position, generate_all_labels, label_filter_for, point_in_polygon, using_ppos_as_reference, Feature, Geometry,
    LabelGenInput, NdMode, Props,
};
use oans::projection::{airport_to_global_coordinates, global_to_airport_coordinates};

fn label(text: &str, style: LabelStyle, feattype: Option<u8>) -> Label {
    Label { text: text.into(), style, position: [0.0, 0.0], rotation: None, feattype, axis_ends: None, termref: None }
}

#[test]
fn projection_round_trips_and_is_in_metres() {
    let arp = Coordinates { lat: -33.9461, long: 151.1772 };
    let p = place_bearing_distance(arp, 90.0, 1.0);
    let local = global_to_airport_coordinates(arp, p);
    assert!((local[0] - 1851.85).abs() < 1.0, "{local:?}");
    assert!(local[1].abs() < 1.0);
    let back = airport_to_global_coordinates(arp, local);
    assert!(distance_to(p, back) * 1852.0 < 1.0);
}

#[test]
fn arinc_words_match_the_recorded_golden_state() {
    let word = Arinc::from_raw(14050411610.0);
    assert!(word.is_normal());
    assert!((word.value - 3972.2720598335964).abs() < 1e-3);
    assert_eq!(Arinc::normal(3972.2720598335964).raw(), 14050411610.0);
    assert_eq!(Arinc { value: 0.0, ssm: oans::arinc::Ssm::NoComputedData }.raw(), 4294967296.0);
    let state = Path::new("E:/golden-tour/yssy-omdb-v15/captures/0193_nd_L_OANS-control-1-16R/nd-1.state.json");
    if let Ok(text) = std::fs::read_to_string(state) {
        let doc: serde_json::Value = serde_json::from_str(&text).unwrap();
        let raw = doc["vars"]["L:A32NX_OANS_RWY_LENGTH|number"].as_f64().unwrap();
        let lda = doc["bus"]["ropsDetectedRunwayLda"]["data"].as_f64().unwrap();
        assert_eq!(Arinc::normal(lda).raw(), raw);
        let bearing = Arinc::from_raw(doc["vars"]["L:A32NX_OANS_RWY_BEARING|number"].as_f64().unwrap());
        assert!(bearing.is_normal() && (0.0..360.0).contains(&bearing.value));
    }
}

#[test]
fn label_filters_follow_the_oanc_rules() {
    let fms = FmsRunways { departure: Some("YSSY16R"), landing: None };
    let taxiway = label("A", LabelStyle::Taxiway, Some(ft::TAXIWAY_ELEMENT));
    let long_taxiway = label("A12", LabelStyle::Taxiway, Some(ft::TAXIWAY_ELEMENT));
    let exit = label("A4", LabelStyle::ExitLine, Some(ft::RUNWAY_EXIT_LINE));
    let none = OancLabelFilter::None;
    assert!(!filter_label(&taxiway, &none, None, None, None, None));
    assert!(filter_label(&taxiway, &OancLabelFilter::Major, None, None, None, None));
    assert!(!filter_label(&long_taxiway, &OancLabelFilter::Major, None, None, None, None));
    assert!(!filter_label(&exit, &OancLabelFilter::Null, None, None, None, None));
    assert!(filter_label(&exit, &label_filter_for(NdMode::RoseNav, 3), None, None, None, None));
    assert!(filter_label(&long_taxiway, &label_filter_for(NdMode::RoseNav, 0), None, None, None, None));
    assert!(!filter_label(&long_taxiway, &label_filter_for(NdMode::RoseNav, 2), None, None, None, None));
    let fms_end = label("16R", LabelStyle::FmsSelectedRunwayEnd, Some(ft::PAINTED_CENTERLINE));
    assert!(filter_label(&fms_end, &none, fms.departure, fms.landing, None, None));
    let arrow = label("34L", LabelStyle::BtvSelectedRunwayArrow, Some(ft::PAINTED_CENTERLINE));
    assert!(!filter_label(&arrow, &none, None, None, Some("YSSY16R"), None));
    assert!(filter_label(&arrow, &none, None, None, Some("YSSY34L"), None));
    assert_eq!(label_style(&label("34L", LabelStyle::RunwayEnd, None), &fms, false, false, Some("YSSY34L"), None), LabelStyle::BtvSelectedRunwayEnd);
    assert_eq!(label_style(&exit, &fms, false, false, None, Some("A4")), LabelStyle::BtvSelectedExit);
    assert_eq!(label_style(&label("16R", LabelStyle::RunwayAxis, None), &fms, true, false, None, None), LabelStyle::FmsSelectedRunwayAxis);
}

#[test]
fn mode_and_flag_rules() {
    assert!(aircraft_on_ground(5.0) && !aircraft_on_ground(7.0));
    assert!(using_ppos_as_reference(NdMode::Arc, false, false));
    assert!(using_ppos_as_reference(NdMode::RoseNav, true, true));
    assert!(!using_ppos_as_reference(NdMode::RoseNav, true, false));
    assert_eq!(label_filter_for(NdMode::Arc, 4), OancLabelFilter::None);
    assert_eq!(label_filter_for(NdMode::Arc, 2), OancLabelFilter::Major);
    assert_eq!(label_filter_for(NdMode::Arc, 0), OancLabelFilter::Null);
}

fn square(x: f64, y: f64, s: f64) -> Vec<[f64; 2]> {
    vec![[x, y], [x + s, y], [x + s, y + s], [x, y + s], [x, y]]
}

#[test]
fn position_computer_names_the_surface_under_the_aircraft() {
    let rwy = Feature {
        feattype: ft::RUNWAY_ELEMENT,
        props: Props { idrwy: Some("16R.34L".into()), ..Default::default() },
        geometry: Geometry::Polygon(vec![square(0.0, 0.0, 100.0)]),
    };
    let twy = Feature {
        feattype: ft::TAXIWAY_ELEMENT,
        props: Props { idlin: Some("A".into()), ..Default::default() },
        geometry: Geometry::Polygon(vec![square(200.0, 0.0, 50.0), square(210.0, 10.0, 10.0)]),
    };
    let all = [rwy, twy];
    assert_eq!(compute_position(&all, [50.0, 50.0]).as_deref(), Some("16R-34L"));
    assert_eq!(compute_position(&all, [0.0, 50.0]).as_deref(), Some("16R-34L"));
    assert_eq!(compute_position(&all, [225.0, 40.0]).as_deref(), Some("A"));
    assert_eq!(compute_position(&all, [215.0, 15.0]), None);
    assert_eq!(compute_position(&all, [205.0, 5.0]).as_deref(), Some("A"));
    assert_eq!(compute_position(&all, [500.0, 500.0]), None);
    assert!(point_in_polygon([5.0, 5.0], &[square(0.0, 0.0, 10.0)]));
}

#[test]
fn runway_labels_swap_designators_to_match_the_bearing() {
    let line = Feature {
        feattype: ft::PAINTED_CENTERLINE,
        props: Props { idrwy: Some("16R.34L".into()), ..Default::default() },
        geometry: Geometry::LineString(vec![[0.0, 0.0], [0.0, 3000.0]]),
    };
    let input = LabelGenInput { airport_icao: "YSSY", fms_origin: Some("YSSY"), fms_departure_runway: Some("YSSY34L"), fms_landing_runway: None, btv_runway: Some("34L") };
    let labels = generate_all_labels(&[line], &input);
    assert_eq!(labels.len(), 7);
    assert_eq!(labels[0].text, "34L");
    assert_eq!(labels[0].style, LabelStyle::BtvSelectedRunwayEnd);
    assert_eq!(labels[1].text, "16R");
    assert_eq!(labels[2].text, "34L-16R");
    assert_eq!(labels[2].style, LabelStyle::FmsSelectedRunwayAxis);
}

#[test]
fn polygon_label_position_averages_every_ring_like_turf() {
    let building = Feature {
        feattype: ft::VERTICAL_POLYGONAL_STRUCTURE,
        props: Props { ident: Some("T1".into()), plysttyp: Some(1), ..Default::default() },
        geometry: Geometry::Polygon(vec![square(0.0, 0.0, 100.0), square(10.0, 10.0, 20.0)]),
    };
    let input = LabelGenInput { airport_icao: "YSSY", fms_origin: None, fms_departure_runway: None, fms_landing_runway: None, btv_runway: None };
    let labels = generate_all_labels(&[building], &input);
    assert_eq!(labels.len(), 1);
    assert_eq!(labels[0].position, [(0.0 + 100.0 + 100.0 + 0.0 + 10.0 + 30.0 + 30.0 + 10.0) / 8.0, (0.0 + 0.0 + 100.0 + 100.0 + 10.0 + 10.0 + 30.0 + 30.0) / 8.0]);
}

#[test]
fn empty_designators_are_not_padded() {
    let mut props = Props { idthr: Some(String::new()), idrwy: Some(String::new()), ..Default::default() };
    oans::oanc::pad_designators(&mut props);
    assert_eq!(props.idthr.as_deref(), Some(""));
    assert_eq!(props.idrwy.as_deref(), Some(""));
    let mut props = Props { idthr: Some("9L".into()), idrwy: Some("9L.27R".into()), ..Default::default() };
    oans::oanc::pad_designators(&mut props);
    assert_eq!(props.idthr.as_deref(), Some("09L"));
    assert_eq!(props.idrwy.as_deref(), Some("09L.27R"));
}

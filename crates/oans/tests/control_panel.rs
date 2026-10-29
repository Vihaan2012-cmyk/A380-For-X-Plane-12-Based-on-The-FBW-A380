use oans::arinc::Arinc;
use oans::control_panel::*;
use oans::geo::Coordinates;
use serde_json::json;

struct Db {
    ok: bool,
}

fn apt(icao: &str, iata: Option<&str>, name: &str, lat: f64, lon: f64) -> AmdbAirport {
    AmdbAirport { idarpt: icao.into(), iata: iata.map(str::to_string), name: Some(name.into()), coordinates: Coordinates { lat, long: lon } }
}

impl AmdbSource for Db {
    fn search_airports(&self) -> Result<Vec<AmdbAirport>, String> {
        if !self.ok {
            return Err("no navigraph".into());
        }
        Ok(vec![
            apt("YSSY", Some("SYD"), "Sydney Kingsford Smith", -33.9461, 151.1772),
            apt("YSBK", Some("BWU"), "Bankstown", -33.9244, 150.9883),
            apt("YMML", Some("MEL"), "Melbourne", -37.6733, 144.8433),
            apt("YXXX", None, "No Iata", -30.0, 140.0),
        ])
    }

    fn map_data(&self, _icao: &str, feature_type: &str) -> Option<Vec<MapDataFeature>> {
        if feature_type != "runwaythreshold" {
            return Some(vec![]);
        }
        Some(vec![
            MapDataFeature { id: Some(7), feattype: Some(2), idthr: Some("34L".into()), lda: Some(3962.0), tora: Some(3962.0), point: Some([1.0, 2.0]), ..Default::default() },
            MapDataFeature { id: Some(8), feattype: Some(2), idthr: Some("16R".into()), lda: Some(3900.0), tora: Some(3962.0), point: Some([3.0, 4.0]), ..Default::default() },
            MapDataFeature { id: Some(9), feattype: Some(2), idthr: Some("16R".into()), point: Some([5.0, 6.0]), ..Default::default() },
        ])
    }
}

fn normal(v: f64) -> Arinc {
    Arinc::normal(v)
}

fn topics(cp: &ControlPanel, t: &str) -> Vec<serde_json::Value> {
    cp.out_bus.iter().filter(|(k, _)| k == t).map(|(_, v)| v.clone()).collect()
}

#[test]
fn availability_follows_navigraph_and_reset_panel() {
    let db = Db { ok: true };
    let mut cp = ControlPanel::new("L");
    assert_eq!(cp.vars["L:A32NX_OANS_AVAILABLE"], 0.0);
    assert_eq!(topics(&cp, "oans_not_avail"), vec![json!(true)]);
    cp.load_oans_db(&db);
    assert_eq!(cp.vars["L:A32NX_OANS_AVAILABLE"], 1.0);
    assert_eq!(cp.airport_database(), AIRPORT_DATABASE_IDENT);
    cp.set_ppos(normal(-33.95), normal(151.18));
    cp.on_real_time(&db, 0.0, 1.0, 100.0, false);
    assert_eq!(cp.loaded_airport.as_ref().unwrap().idarpt, "YSSY");
    cp.set_reset_pulled(true);
    assert_eq!(cp.vars["L:A32NX_OANS_FAILED"], 1.0);
    assert_eq!(cp.vars["L:A32NX_OANS_AVAILABLE"], 0.0);
    assert_eq!(topics(&cp, "oans_display_airport").last(), Some(&json!("")));
    assert!(cp.loaded_airport.is_none());
    let mut cp = ControlPanel::new("L");
    cp.load_oans_db(&Db { ok: false });
    assert_eq!(cp.airport_database(), "N/A");
    assert!(cp.fms_landing_runway_not_selected_in_fallback());
}

#[test]
fn auto_load_nearest_on_ground_and_destination_in_flight() {
    let db = Db { ok: true };
    let mut cp = ControlPanel::new("L");
    cp.load_oans_db(&db);
    cp.on_real_time(&db, 0.0, 1.0, 0.0, false);
    assert!(cp.loaded_airport.is_none(), "no ppos");
    cp.set_ppos(normal(-33.93), normal(151.0));
    cp.on_real_time(&db, 2000.0, 1.0, 0.0, false);
    assert_eq!(cp.loaded_airport.as_ref().unwrap().idarpt, "YSBK");
    assert_eq!(cp.selected_airport.as_ref().unwrap().idarpt, "YSBK");
    assert_eq!(cp.available_entity_list, vec!["16R".to_string(), "34L".to_string()]);
    cp.set_ppos(normal(-36.6), normal(145.0));
    cp.fms.destination = Some("YMML".into());
    cp.on_real_time(&db, 4000.0, 7.0, 0.0, false);
    assert_eq!(cp.loaded_airport.as_ref().unwrap().idarpt, "YSBK", "70 NM from destination");
    cp.set_ppos(normal(-37.5), normal(144.9));
    cp.on_real_time(&db, 6000.0, 7.0, 0.0, false);
    assert_eq!(cp.loaded_airport.as_ref().unwrap().idarpt, "YMML");
    assert!(!cp.is_airport_selection_pending);
}

#[test]
fn manual_selection_blocks_auto_load_for_ten_minutes() {
    let db = Db { ok: true };
    let mut cp = ControlPanel::new("R");
    cp.load_oans_db(&db);
    cp.set_ppos(normal(-33.95), normal(151.18));
    cp.fms.origin = Some("YMML".into());
    cp.select_fms_airport(&db, FmsAirport::Origin).unwrap();
    assert!(cp.is_airport_selection_pending);
    cp.on_real_time(&db, 0.0, 1.0, 10.0, false);
    assert!(cp.loaded_airport.is_none(), "selected differs from loaded");
    cp.handle_display_airport(&db, 10.0).unwrap();
    assert_eq!(cp.loaded_airport.as_ref().unwrap().idarpt, "YMML");
    cp.on_real_time(&db, 2000.0, 1.0, 600.0, false);
    assert_eq!(cp.loaded_airport.as_ref().unwrap().idarpt, "YMML");
    cp.on_real_time(&db, 4000.0, 1.0, 610.5, false);
    assert_eq!(cp.loaded_airport.as_ref().unwrap().idarpt, "YSSY");
}

#[test]
fn search_modes_sort_and_drop_missing_props() {
    let db = Db { ok: true };
    let mut cp = ControlPanel::new("L");
    cp.load_oans_db(&db);
    assert_eq!(cp.airport_search_data, vec!["YMML", "YSBK", "YSSY", "YXXX"]);
    cp.handle_select_airport(&db, "YSSY", None).unwrap();
    assert_eq!(cp.selected_letter_index, Some(24));
    cp.handle_select_airport_search_mode(AirportSearchMode::Iata);
    assert_eq!(cp.airport_search_data, vec!["BWU", "MEL", "SYD"]);
    assert_eq!(cp.selected_airport_index, Some(2));
    assert_eq!(cp.selected_letter_index, Some(18));
    assert!(cp.handle_select_airport(&db, "YXXX", None).is_err());
    cp.handle_select_airport_search_mode(AirportSearchMode::City);
    assert_eq!(cp.airport_search_data[0], "BANKSTOWN");
    assert_eq!(cp.selected_airport_name(), "SYDNEY KINGSFORD S");
}

#[test]
fn map_data_entity_selection_and_symbol_buttons() {
    let db = Db { ok: true };
    let mut cp = ControlPanel::new("L");
    cp.load_oans_db(&db);
    cp.handle_select_airport(&db, "YSSY", None).unwrap();
    assert!(cp.entity_is_not_selected());
    cp.set_selected_entity_index(Some(0));
    assert_eq!(cp.selected_entity_string.as_deref(), Some("16R"));
    assert_eq!(cp.selected_feature_id, Some(8));
    assert_eq!(cp.selected_entity_position, [3.0, 4.0]);
    assert_eq!(cp.runway_lda, 3900.0);
    cp.handle_cross_button();
    assert_eq!(topics(&cp, "oans_add_cross_at_feature"), vec![json!({"id": 8, "feattype": 2})]);
    cp.set_symbols_for_feature_ids(vec![8], vec![]);
    assert!(cp.cross_exists_for_entity());
    cp.handle_cross_button();
    assert_eq!(topics(&cp, "oans_remove_cross_at_feature").len(), 1);
    cp.handle_flag_button();
    assert_eq!(topics(&cp, "oans_add_flag_at_feature").len(), 1);
    cp.center_map_on_selected();
    assert_eq!(topics(&cp, "oans_center_map_on"), vec![json!([3.0, 4.0])]);
    cp.set_runway_info_inputs(None, None, None, None);
    assert!(!cp.entity_is_not_selected());
    cp.set_runway_info_inputs(None, None, None, Some("YSSY16R".into()));
    cp.set_runway_info_inputs(None, None, None, None);
    assert!(cp.entity_is_not_selected());
    assert!(cp.runway_lda.is_nan());
    cp.set_runway_info_inputs(Some("YSSY".into()), Some("YSSY34L".into()), Some(3962.0), None);
    assert_eq!(cp.runway_tora, 3962.0);
    assert_eq!(ControlPanel::runway_length_text(3962.0, false), "12999");
    cp.set_requested_stopping_distance(normal(1700.4));
    assert_eq!(cp.req_stopping_distance(), Some(2100.0));
}

#[test]
fn performance_mode_hides_after_sixty_seconds() {
    let db = Db { ok: true };
    let mut cp = ControlPanel::new("L");
    cp.performance_mode = true;
    let mut t = 0.0;
    let mut hidden_at = None;
    for _ in 0..40 {
        cp.on_real_time(&db, t, 1.0, 0.0, false);
        if hidden_at.is_none() && topics(&cp, "oans_performance_mode_hide").last().unwrap()["hide"] == json!(true) {
            hidden_at = Some(t);
        }
        t += 2000.0;
    }
    assert_eq!(hidden_at, Some(62_000.0));
    cp.set_show_oans("L", true);
    cp.on_real_time(&db, t, 1.0, 0.0, false);
    assert_eq!(topics(&cp, "oans_performance_mode_hide").last().unwrap(), &json!({"side": "L", "hide": false}));
}

use mfd_pages_c::data_airport::*;
use mfd_pages_c::data_navaid::*;
use mfd_pages_c::data_status::*;
use mfd_pages_c::data_waypoint::*;
use std::fs;

fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| "E:/fbw-build/mfd-pages-c".to_string());
    let w = |name: &str, n: mfd_tree::Node| {
        fs::write(format!("{}/{}.json", out, name), n.to_json()).unwrap();
    };
    let status = DataStatusModel {
        fuel_penalty_pct: Some(0.0),
        database: Some(DatabaseIdent {
            provider: "MS".into(),
            airac_cycle: "2406".into(),
            effective_from: "2024-06-13".into(),
            effective_to: "2024-07-11".into(),
        }),
        stored: Some(StoredElements { waypoints: 0, routes: 0, navaids: 0, runways: 0 }),
        ..Default::default()
    };
    w("data_status", data_status(&status));
    w("data_waypoint", data_waypoint(&DataWaypointModel::default()));
    w("data_airport", data_airport(&DataAirportModel { use_metric: true, custom_rwy_button_disabled: true, ..Default::default() }));
    w("data_navaid", data_navaid(&DataNavaidModel::default()));
    w("data_waypoint_tab1", data_waypoint(&DataWaypointModel { selected_tab: 1, ..Default::default() }));
    w("data_airport_tab1", data_airport(&DataAirportModel { selected_tab: 1, use_metric: true, custom_rwy_button_disabled: true, ..Default::default() }));
    w("data_navaid_tab1", data_navaid(&DataNavaidModel { selected_tab: 1, ..Default::default() }));
}

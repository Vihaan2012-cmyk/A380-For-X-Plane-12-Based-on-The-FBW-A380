use crate::formats::*;
use crate::init::{side_prefix, FP_ACTIVE, FP_TEMPORARY, PHASE_DESCENT};
use crate::widgets::*;
use mfd_tree::Node;
use serde::Deserialize;

const PHASE_TAKEOFF: u8 = 1;

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct FuelModel {
    pub captain: bool,
    pub loaded_fp_index: i32,
    pub flight_phase: u8,
    pub fp_is_active_or_copy_of_active: bool,
    pub sec_active: bool,
    pub eo_active: bool,
    pub metric: bool,
    pub engines_were_started: bool,
    pub gross_weight_kg: Option<f64>,
    pub center_of_gravity: Option<f64>,
    pub fuel_on_board_kg: Option<f64>,
    pub zero_fuel_weight_kg: Option<f64>,
    pub zero_fuel_weight_cg: Option<f64>,
    pub block_fuel_kg: Option<f64>,
    pub taxi_fuel_kg: Option<f64>,
    pub taxi_fuel_pilot_entered: bool,
    pub trip_fuel_kg: Option<f64>,
    pub trip_fuel_time: String,
    pub route_reserve_kg: Option<f64>,
    pub route_reserve_pilot_entered: bool,
    pub route_reserve_pct: Option<f64>,
    pub route_reserve_pct_pilot_entered: bool,
    pub alternate_fuel_kg: Option<f64>,
    pub alternate_fuel_pilot_entered: bool,
    pub alternate_exists: bool,
    pub final_fuel_kg: Option<f64>,
    pub final_fuel_pilot_entered: bool,
    pub final_time_min: Option<f64>,
    pub final_time_pilot_entered: bool,
    pub pax: Option<f64>,
    pub cost_index_mode: Option<usize>,
    pub cost_index: Option<f64>,
    pub jettison_gw: Option<f64>,
    pub takeoff_weight_kg: Option<f64>,
    pub landing_weight_kg: Option<f64>,
    pub dest_icao: Option<String>,
    pub dest_eta: String,
    pub dest_efob_kg: Option<f64>,
    pub dest_efob_below_min: bool,
    pub altn_icao: String,
    pub altn_eta: String,
    pub altn_efob_kg: Option<f64>,
    pub min_dest_fuel_kg: Option<f64>,
    pub min_dest_pilot_entered: bool,
    pub extra_fuel_kg: Option<f64>,
    pub extra_fuel_time_text: String,
    pub open_dropdown: Option<String>,
}

impl Default for FuelModel {
    fn default() -> Self {
        FuelModel {
            captain: true,
            loaded_fp_index: FP_ACTIVE,
            flight_phase: 0,
            fp_is_active_or_copy_of_active: true,
            sec_active: false,
            eo_active: false,
            metric: true,
            engines_were_started: false,
            gross_weight_kg: None,
            center_of_gravity: None,
            fuel_on_board_kg: None,
            zero_fuel_weight_kg: None,
            zero_fuel_weight_cg: None,
            block_fuel_kg: None,
            taxi_fuel_kg: None,
            taxi_fuel_pilot_entered: false,
            trip_fuel_kg: None,
            trip_fuel_time: "--:--".to_string(),
            route_reserve_kg: None,
            route_reserve_pilot_entered: false,
            route_reserve_pct: None,
            route_reserve_pct_pilot_entered: false,
            alternate_fuel_kg: None,
            alternate_fuel_pilot_entered: false,
            alternate_exists: true,
            final_fuel_kg: None,
            final_fuel_pilot_entered: false,
            final_time_min: None,
            final_time_pilot_entered: false,
            pax: None,
            cost_index_mode: None,
            cost_index: None,
            jettison_gw: None,
            takeoff_weight_kg: None,
            landing_weight_kg: None,
            dest_icao: None,
            dest_eta: "--:--".to_string(),
            dest_efob_kg: None,
            dest_efob_below_min: false,
            altn_icao: "----".to_string(),
            altn_eta: "--:--".to_string(),
            altn_efob_kg: None,
            min_dest_fuel_kg: None,
            min_dest_pilot_entered: false,
            extra_fuel_kg: None,
            extra_fuel_time_text: "--:--".to_string(),
            open_dropdown: None,
        }
    }
}

fn value_span(sec: bool, text: &str) -> Node {
    Node::el("span", "mfd-value").class_if("sec", sec).text(text)
}

fn weight_value(sec: bool, kg: Option<f64>, metric: bool) -> Vec<Node> {
    vec![value_span(sec, &weight_text(kg, metric)), span_c("mfd-label-unit mfd-unit-trailing", weight_unit_text(metric))]
}

pub fn fuel_load_page(m: &FuelModel) -> Node {
    let mandatory_and_active = m.loaded_fp_index == FP_ACTIVE || m.loaded_fp_index == FP_TEMPORARY;
    let metric = m.metric;
    let sec = m.sec_active;
    let side = side_prefix(m.captain);
    let taxi_rsv_disabled = m.flight_phase >= PHASE_TAKEOFF && m.fp_is_active_or_copy_of_active;
    let dest_icao = m.dest_icao.as_deref().filter(|s| !s.is_empty());
    let ci_mode_disabled = (m.fp_is_active_or_copy_of_active && m.flight_phase >= PHASE_DESCENT) || dest_icao.is_none();
    let ci_disabled = ci_mode_disabled || m.cost_index_mode == Some(0);
    let ci_labels: [&str; 2] = if m.eo_active { ["EO-LRC", "EO-ECON"] } else { ["LRC", "ECON"] };
    let time_header = if m.flight_phase == 0 { "TIME" } else { "UTC" };
    let jtsn_vis = if mandatory_and_active { "visible" } else { "hidden" };
    let dest_amber = m.dest_efob_below_min && m.loaded_fp_index == FP_ACTIVE;
    let cg_text = match m.center_of_gravity {
        Some(v) if v != 0.0 => fixed1(v),
        _ => "--.-".to_string(),
    };
    let (gw, cg, fob) = if m.engines_were_started {
        (m.gross_weight_kg, cg_text.clone(), m.fuel_on_board_kg)
    } else {
        (None, "--.-".to_string(), None)
    };
    let lv = |label: &str, text: &str, unit: &str| {
        label_value("")
            .child(span_c("mfd-label mfd-spacing-right", label))
            .child(value_span(sec, text))
            .child(span_c("mfd-label-unit mfd-unit-trailing", unit))
    };

    let top = div("display: flex; flex-direction: row; justify-content: space-between; margin: 10px 25px 10px 25px;")
        .child(lv("GW", &weight_text(gw, metric), weight_unit_text(metric)))
        .child(lv("CG", &cg, "%"))
        .child(lv("FOB", &weight_text(fob, metric), weight_unit_text(metric)));

    let zfw_row = div("display: flex; flex-direction: row; margin-bottom: 15px; align-items: center;")
        .child(div_c("mfd-label mfd-spacing-right fuelLoad", "").text("ZFW"))
        .child(
            field(weight(m.zero_fuel_weight_kg, metric))
                .mandatory(mandatory_and_active)
                .align("flex-end")
                .cstyle("width: 150px;")
                .node(),
        )
        .child(div_c("mfd-label mfd-spacing-right fuelLoad", "").text("ZFWCG"))
        .child(
            field(percentage(m.zero_fuel_weight_cg))
                .mandatory(mandatory_and_active)
                .align("center")
                .cstyle("width: 125px;")
                .node(),
        );

    let block_line = div_c(
        "mfd-fms-fuel-load-block-line",
        &format!("visibility: {};", if m.engines_were_started { "hidden" } else { "visible" }),
    )
    .child(div_c("mfd-label mfd-spacing-right fuelLoad", "").text("BLOCK"))
    .child(
        field(weight(m.block_fuel_kg, metric))
            .mandatory(mandatory_and_active)
            .align("flex-end")
            .cstyle("width: 150px;")
            .node(),
    )
    .child(
        div("display: flex; flex: 1; justify-content: center;").child(
            button_node(two_line_star_label("FUEL", "PLANNING"))
                .disabled(true)
                .style("padding-right: 2px;")
                .node(),
        ),
    );

    let tri = |kg: Option<f64>, pilot: bool, max_dis: bool, min_unused: bool| {
        let _ = min_unused;
        field(weight(kg, metric)).pilot(pilot).disabled(max_dis).align("flex-end").cstyle("width: 150px;").node()
    };

    let left = div_c("mfd-fms-fuel-load-middle-flex-left", "")
        .child(div_c("mfd-label mfd-spacing-right middleGrid", "").text("TAXI"))
        .child(div("margin-bottom: 20px;").child(tri(m.taxi_fuel_kg, m.taxi_fuel_pilot_entered, taxi_rsv_disabled, false)))
        .child(div(""))
        .child(div_c("mfd-label mfd-spacing-right middleGrid", "").text("TRIP"))
        .child(
            label_value("justify-content: flex-end; margin-bottom: 20px;").children(weight_value(sec, m.trip_fuel_kg, metric)),
        )
        .child(div("display: flex; justify-content: center; margin-bottom: 20px;").child(value_span(sec, &m.trip_fuel_time)))
        .child(div_c("mfd-label mfd-spacing-right middleGrid", "").text("RTE RSV"))
        .child(
            div("margin-bottom: 20px;")
                .child(tri(m.route_reserve_kg, m.route_reserve_pilot_entered, taxi_rsv_disabled, false)),
        )
        .child(
            div("margin-bottom: 20px; margin-left: 5px;").child(
                field(percentage(m.route_reserve_pct))
                    .pilot(m.route_reserve_pct_pilot_entered)
                    .disabled(taxi_rsv_disabled)
                    .align("center")
                    .cstyle("width: 120px;")
                    .node(),
            ),
        )
        .child(div_c("mfd-label mfd-spacing-right middleGrid", "").text("ALTN"))
        .child(
            div("margin-bottom: 20px;")
                .child(tri(m.alternate_fuel_kg, m.alternate_fuel_pilot_entered, !m.alternate_exists, false)),
        )
        .child(div("display: flex; justify-content: center; margin-bottom: 20px;").child(value_span(sec, "--:--")))
        .child(div_c("mfd-label mfd-spacing-right middleGrid", "").text("FINAL"))
        .child(div("margin-bottom: 20px;").child(tri(m.final_fuel_kg, m.final_fuel_pilot_entered, false, false)))
        .child(
            div("margin-bottom: 20px; margin-left: 5px;").child(
                field(time_hhmm(m.final_time_min))
                    .pilot(m.final_time_pilot_entered)
                    .align("center")
                    .cstyle("width: 120px;")
                    .node(),
            ),
        );

    let mode_open = m.open_dropdown.as_deref() == Some("costIndexMode");
    let right = div_c("mfd-fms-fuel-load-middle-flex-right", "")
        .child(div_c("mfd-label mfd-spacing-right middleGridSmall", "").text("PAX NBR"))
        .child(
            div("margin-bottom: 10px;").child(
                field(pax_nbr(m.pax)).mandatory(mandatory_and_active).align("center").cstyle("width: 75px;").node(),
            ),
        )
        .child(div_c("mfd-label mfd-spacing-right middleGridSmall", "").text("MODE"))
        .child(
            div("margin-bottom: 10px;").child(
                Dropdown::new(&format!("{}_MFD_initCostIndexModeDropdown", side), &ci_labels, m.cost_index_mode)
                    .disabled(ci_mode_disabled)
                    .cstyle("width: 175px; margin-right: 65px;")
                    .digits(7)
                    .open(mode_open)
                    .node(),
            ),
        )
        .child(div_c("mfd-label mfd-spacing-right middleGridSmall", "").text("CI"))
        .child(
            div("margin-bottom: 10px;").child(
                field(cost_index(m.cost_index))
                    .mandatory(mandatory_and_active)
                    .disabled(ci_disabled)
                    .align("center")
                    .cstyle("width: 75px;")
                    .node(),
            ),
        )
        .child(div_c("mfd-label mfd-spacing-right middleGridSmall", &format!("visibility: {};", jtsn_vis)).text("JTSN GW"))
        .child(
            div(&format!("margin-bottom: 10px; visibility: {};", jtsn_vis)).child(
                field(weight(m.jettison_gw, metric)).disabled(true).align("flex-end").cstyle("width: 150px;").node(),
            ),
        )
        .child(div_c("mfd-label mfd-spacing-right middleGridSmall", "").text("TOW"))
        .child(
            label_value("justify-content: center; margin-bottom: 10px;").children(weight_value(sec, m.takeoff_weight_kg, metric)),
        )
        .child(div_c("mfd-label mfd-spacing-right middleGridSmall", "").text("LW"))
        .child(
            label_value("justify-content: center; margin-bottom: 10px;").children(weight_value(sec, m.landing_weight_kg, metric)),
        );

    let middle = div_c("mfd-fms-fuel-load-middle-flex", "").child(left).child(right);

    let big_label = |text: &str, with_sec: bool| {
        Node::el("div", "mfd-label bigger")
            .class_if("green", mandatory_and_active)
            .class_if("sec", with_sec && sec)
            .class_if("mfd-fms-fuel-load-dest-grid-middle-cell", with_sec)
            .text(text)
    };
    let dest_efob_cell = label_value("")
        .class_if("mfd-fms-fuel-load-dest-grid-efob-cell", true)
        .child(
            Node::el("span", "mfd-value")
                .class_if("amber", dest_amber)
                .class_if("sec", sec)
                .text(&weight_text(m.dest_efob_kg, metric)),
        )
        .child(span_c("mfd-label-unit mfd-unit-trailing", weight_unit_text(metric)));
    let altn_efob_cell = label_value("")
        .class_if("mfd-fms-fuel-load-dest-grid-efob-cell", true)
        .children(weight_value(sec, m.altn_efob_kg, metric));
    let centered = "text-align: center; align-self: center;";
    let grid = div("display: grid; grid-template-columns: auto auto auto auto;")
        .child(div_c("mfd-fms-fuel-load-dest-grid-top-cell", ""))
        .child(div_c("mfd-fms-fuel-load-dest-grid-top-cell", ""))
        .child(div_c("mfd-label mfd-fms-fuel-load-dest-grid-top-cell", "").text(time_header))
        .child(div_c("mfd-label mfd-fms-fuel-load-dest-grid-top-cell", "").text("EFOB"))
        .child(div_c("mfd-label mfd-fms-fuel-load-dest-grid-middle-cell", "").text("DEST"))
        .child(big_label(dest_icao.unwrap_or("NONE"), true))
        .child(big_label(&m.dest_eta, true))
        .child(dest_efob_cell)
        .child(div_c("mfd-label", centered).text("ALTN"))
        .child(big_label(&m.altn_icao, false).style(centered))
        .child(big_label(&m.altn_eta, false).style(centered))
        .child(altn_efob_cell);

    let right_col = div("flex: 1; flex-direction: column; justify-content: center; align-items: center;")
        .child(div_c("mfd-label", "margin-bottom: 20px; text-align: center;").text("MIN FUEL AT DEST"))
        .child(
            div("margin-bottom: 30px; display: flex; justify-content: center;").child(
                field(weight(m.min_dest_fuel_kg, metric))
                    .pilot(m.min_dest_pilot_entered)
                    .align("flex-end")
                    .cstyle("width: 150px;")
                    .node(),
            ),
        )
        .child(div_c("mfd-label", "margin-bottom: 5px; text-align: center;").text("EXTRA"))
        .child(
            div("display: flex; flex-direction: row; justify-content: center; align-items: center;")
                .child(label_value("margin-right: 20px;").children(weight_value(sec, m.extra_fuel_kg, metric)))
                .child(value_span(sec, &m.extra_fuel_time_text)),
        );

    let lower = div("flex: 1; display: flex; flex-direction: row; margin-top: 25px;")
        .child(div("width: 62.5%").child(grid))
        .child(right_col);

    div_c("mfd-page-container", "")
        .child(top)
        .child(zfw_row)
        .child(block_line)
        .child(middle)
        .child(lower)
        .child(div("flex-grow: 1;"))
        .child(div("width: 150px;").child(button("RETURN").style("margin-right: 5px;").node()))
}

use crate::formats::*;
use crate::widgets::*;
use mfd_tree::Node;
use serde::Deserialize;

pub const FP_ACTIVE: i32 = 0;
pub const FP_TEMPORARY: i32 = 1;
pub const FP_FIRST_SECONDARY: i32 = 3;

pub const PHASE_PREFLIGHT: u8 = 0;
pub const PHASE_CRUISE: u8 = 3;
pub const PHASE_DESCENT: u8 = 4;

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct InitModel {
    pub captain: bool,
    pub loaded_fp_index: i32,
    pub flight_phase: u8,
    pub tmpy_active: bool,
    pub eo_active: bool,
    pub has_flight_plan: bool,
    pub fp_is_active_or_copy_of_active: bool,
    pub flight_number: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub altn: Option<String>,
    pub cpny_rte: Option<String>,
    pub altn_rte: Option<String>,
    pub crz_fl: Option<f64>,
    pub crz_temp: Option<f64>,
    pub crz_temp_pilot_entered: bool,
    pub cost_index_mode: usize,
    pub cost_index: Option<f64>,
    pub tropo: Option<f64>,
    pub tropo_pilot_entered: bool,
    pub trip_wind: Option<f64>,
    pub trip_wind_pilot_entered: bool,
    pub fp_has_wind_entries: bool,
    pub cpny_fpln_avail: bool,
    pub cpny_fpln_uplink_in_progress: bool,
    pub uplink_in_progress: bool,
    pub cpny_fpln_requested_for_plan: Option<i32>,
    pub engines_were_started: bool,
    pub has_active_fp_with_city_pair: bool,
    pub wind_uplink_available_for_plan: bool,
    pub wind_uplink_available_any_plan: bool,
    pub cpny_wind_uplink_in_progress: bool,
    pub open_dropdown: Option<String>,
}

impl Default for InitModel {
    fn default() -> Self {
        InitModel {
            captain: true,
            loaded_fp_index: FP_ACTIVE,
            flight_phase: PHASE_PREFLIGHT,
            tmpy_active: false,
            eo_active: false,
            has_flight_plan: true,
            fp_is_active_or_copy_of_active: true,
            flight_number: None,
            from: None,
            to: None,
            altn: None,
            cpny_rte: None,
            altn_rte: None,
            crz_fl: None,
            crz_temp: None,
            crz_temp_pilot_entered: false,
            cost_index_mode: 1,
            cost_index: None,
            tropo: None,
            tropo_pilot_entered: false,
            trip_wind: None,
            trip_wind_pilot_entered: false,
            fp_has_wind_entries: false,
            cpny_fpln_avail: false,
            cpny_fpln_uplink_in_progress: false,
            uplink_in_progress: false,
            cpny_fpln_requested_for_plan: None,
            engines_were_started: false,
            has_active_fp_with_city_pair: false,
            wind_uplink_available_for_plan: false,
            wind_uplink_available_any_plan: false,
            cpny_wind_uplink_in_progress: false,
            open_dropdown: None,
        }
    }
}

pub fn side_prefix(captain: bool) -> &'static str {
    if captain {
        "CAPT"
    } else {
        "FO"
    }
}

pub fn cpny_fpln_button_label(avail: bool, in_progress: bool) -> &'static str {
    if in_progress {
        "REQUEST\nPENDING..."
    } else if !avail {
        "CPNY F-PLN\nREQUEST"
    } else {
        "RECEIVED\nCPNY F-PLN"
    }
}

pub fn cpny_fpln_button_disabled(m: &InitModel) -> bool {
    m.uplink_in_progress
        || (m.loaded_fp_index == FP_ACTIVE
            && (m.engines_were_started
                || m.has_active_fp_with_city_pair
                || m.cpny_fpln_requested_for_plan.map(|p| p >= FP_FIRST_SECONDARY).unwrap_or(false)))
}

pub fn cpny_wind_button(m: &InitModel) -> Node {
    let avail = m.wind_uplink_available_for_plan;
    let in_progress = m.cpny_wind_uplink_in_progress;
    let disabled_phase = m.fp_is_active_or_copy_of_active && m.flight_phase > PHASE_CRUISE;
    let disabled = disabled_phase
        || m.tmpy_active
        || m.uplink_in_progress
        || (!avail && m.wind_uplink_available_any_plan);
    let top = if in_progress { "REQUEST" } else if avail { "RECEIVED" } else { "CPNY WIND" };
    let bottom = if in_progress { "PENDING..." } else if avail { "CPNY WIND" } else { "REQUEST" };
    let hide_asterisk = avail || in_progress;
    let label = Node::new("div")
        .style("display: flex; flex-direction: row; justify-content: space-between;")
        .child(
            Node::new("span")
                .style("text-align: center; vertical-align: center; margin-right: 10px;")
                .text(top)
                .child(Node::new("br"))
                .text(bottom),
        )
        .child(
            Node::new("span")
                .style("display: flex; align-items: center; justify-content: center;")
                .class_if("hidden", hide_asterisk)
                .text("*"),
        );
    let items: Vec<(String, bool)> = if avail && !m.tmpy_active {
        vec![("INSERT*".to_string(), false), ("CLEAR*".to_string(), false)]
    } else {
        vec![]
    };
    button_node(label)
        .disabled(disabled)
        .menu(items, "mfd-fms-wind-cpny-wind-request")
        .show_arrow(false)
        .style("margin-right: 10px; justify-self: flex-end; width: 178px; height: 58px;")
        .node()
}

pub fn init_page(m: &InitModel) -> Node {
    let mandatory_and_active = m.loaded_fp_index == FP_ACTIVE || m.loaded_fp_index == FP_TEMPORARY;
    let visibility_only_active = mandatory_and_active;
    let visible_in_sec = !mandatory_and_active;
    let from = m.from.as_deref().filter(|s| !s.is_empty());
    let to = m.to.as_deref().filter(|s| !s.is_empty());
    let no_fp = !m.has_flight_plan;
    let city_pair_disabled =
        (m.flight_phase > PHASE_PREFLIGHT && m.fp_is_active_or_copy_of_active) || m.tmpy_active;
    let altn_disabled = m.tmpy_active || to.is_none() || from.is_none();
    let crz_fl_mandatory = m.flight_phase < PHASE_DESCENT && m.fp_is_active_or_copy_of_active;
    let ci_mode_disabled = to.is_none()
        || from.is_none()
        || (m.flight_phase >= PHASE_DESCENT && m.fp_is_active_or_copy_of_active);
    let ci_disabled = ci_mode_disabled || m.cost_index_mode == 0;
    let trip_wind_disabled = to.is_none() || from.is_none() || m.fp_has_wind_entries;
    let cpny_rte_mandatory = (to.is_none() || from.is_none()) && mandatory_and_active;
    let departure_disabled = to.is_none() || from.is_none() || m.flight_phase != PHASE_PREFLIGHT;
    let crz_temp_disabled =
        m.crz_fl.is_none() || (m.fp_is_active_or_copy_of_active && m.flight_phase >= PHASE_CRUISE);
    let side = side_prefix(m.captain);
    let ci_labels: [&str; 2] = if m.eo_active { ["EO-LRC", "EO-ECON"] } else { ["LRC", "ECON"] };

    let cpny_fpln = button(cpny_fpln_button_label(m.cpny_fpln_avail, m.cpny_fpln_uplink_in_progress))
        .disabled(cpny_fpln_button_disabled(m))
        .style("width: 175px;")
        .menu(
            if m.cpny_fpln_avail { vec![("INSERT*".to_string(), false), ("CLEAR*".to_string(), false)] } else { vec![] },
            &format!("{}_MFD_fplnreq", side),
        )
        .show_arrow(false)
        .node();

    let line1 = div_c("mfd-fms-init-line", "")
        .child(div_c("mfd-label init-input-field", "").text("FLT NBR"))
        .child(
            field(long_alphanumeric(m.flight_number.as_deref()))
                .disabled(no_fp)
                .mandatory(mandatory_and_active)
                .pilot(true)
                .cstyle("width: 200px; margin-right: 5px;")
                .align("center")
                .node(),
        )
        .child(button("ACFT STATUS").style("margin-right: 10px; width: 200px;").node())
        .child(div("flex-grow: 1"))
        .child(cpny_fpln);

    let line2 = div_c("mfd-fms-init-line second-line", "")
        .child(div_c("mfd-label init-input-field", "").text("FROM"))
        .child(field(airport(from)).mandatory(mandatory_and_active).disabled(city_pair_disabled).align("center").node())
        .child(div_c("mfd-label init-space-lr", "").text("TO"))
        .child(field(airport(to)).mandatory(mandatory_and_active).disabled(city_pair_disabled).align("center").node())
        .child(div_c("mfd-label init-space-lr", "").text("ALTN"))
        .child(
            field(airport(m.altn.as_deref()))
                .mandatory(mandatory_and_active)
                .disabled(altn_disabled)
                .align("center")
                .node(),
        );

    let line3 = div_c("mfd-fms-init-line", "")
        .child(div_c("mfd-label init-input-field", "").text("CPNY RTE"))
        .child(
            field(long_alphanumeric(m.cpny_rte.as_deref()))
                .mandatory(cpny_rte_mandatory)
                .disabled(true)
                .cstyle("width: 200px; margin-right: 5px;")
                .align("center")
                .node(),
        )
        .child(button("RTE SEL").disabled(true).style("margin-right: 10px; width: 200px;").node());

    let line4 = div_c("mfd-fms-init-line altn-rte", "")
        .child(div_c("mfd-label init-input-field", "").text("ALTN RTE"))
        .child(
            field(long_alphanumeric(m.altn_rte.as_deref()))
                .disabled(true)
                .cstyle("width: 200px; margin-right: 5px;")
                .align("center")
                .node(),
        )
        .child(button("ALTN RTE SEL").disabled(true).style("margin-right: 10px; width: 200px;").node());

    let line5 = div_c("mfd-fms-init-line", "")
        .child(div_c("mfd-label init-input-field", "").text("CRZ FL"))
        .child(
            field(flight_level(m.crz_fl))
                .mandatory(crz_fl_mandatory)
                .disabled(altn_disabled)
                .class("mfd-init-crz-fl")
                .align("center")
                .node(),
        )
        .child(div_c("mfd-label init-input-field", "width: auto;").text("CRZ TEMP"))
        .child(
            field(crz_temp(m.crz_temp))
                .pilot(m.crz_temp_pilot_entered)
                .disabled(crz_temp_disabled)
                .cstyle("width: 110px; justify-content: flex-end;")
                .align("center")
                .node(),
        );

    let mode_open = m.open_dropdown.as_deref() == Some("costIndexMode");
    let line6 = div_c("mfd-fms-init-line", "margin-top: 10px;")
        .child(div_c("mfd-label init-input-field", "").text("MODE"))
        .child(
            Dropdown::new(&format!("{}_MFD_initCostIndexModeDropdown", side), &ci_labels, Some(m.cost_index_mode))
                .disabled(ci_mode_disabled)
                .cstyle("width: 175px; margin-right: 65px; ")
                .digits(7)
                .open(mode_open)
                .node(),
        )
        .child(div_c("mfd-label init-input-field", "width: auto;").text("TROPO"))
        .child(
            field(tropo(m.tropo))
                .pilot(m.tropo_pilot_entered)
                .disabled(no_fp)
                .align("flex-end")
                .node(),
        );

    let line7 = div_c("mfd-fms-init-line trip-wind", "")
        .child(
            div_c("fc", "align-self: flex-start; margin-top: 15px;")
                .child(div_c("mfd-label init-input-field", "").text("CI"))
                .child(div_c("mfd-label init-input-field", "margin-top: 27px;").text("TRIP WIND")),
        )
        .child(
            div_c("fc", "align-self: flex-start; margin-top: 5px;")
                .child(
                    field(cost_index(m.cost_index))
                        .mandatory(mandatory_and_active)
                        .disabled(ci_disabled)
                        .cstyle("width: 70px; margin-right: 90px; justify-content: center;")
                        .align("center")
                        .key("CI")
                        .node(),
                )
                .child(
                    field(trip_wind(m.trip_wind))
                        .pilot(m.trip_wind_pilot_entered)
                        .disabled(trip_wind_disabled)
                        .cstyle("width: 125px; margin-right: 80px; margin-top: 10px;")
                        .align("flex-start")
                        .node(),
                ),
        )
        .child(button("WIND").style("margin-right: 10px; margin-top: 52px;").node())
        .child(div("flex-grow: 1"))
        .child(cpny_wind_button(m));

    let vis = |v: bool| if v { "visibility: inherit;" } else { "visibility: hidden;" };
    let active_block = div(vis(visibility_only_active))
        .child(button("IRS").style("width: 160px; margin-left: 150px; margin-bottom: 10px;").node())
        .child(
            div_c("fr", "")
                .child(
                    button("DEPARTURE")
                        .disabled(departure_disabled)
                        .style("width: 160px; margin-left: 150px; margin-bottom: 10px;")
                        .node(),
                )
                .child(
                    button("RTE SUMMARY").disabled(true).style("margin-left: 50px; margin-bottom: 10px;").node(),
                ),
        )
        .child(button("NAVAIDS").style("width: 160px; margin-left: 150px; margin-bottom: 10px;").node())
        .child(button("FUEL&LOAD").style("width: 160px; margin-left: 150px; margin-bottom: 10px;").node())
        .child(
            div("display: flex; flex-direction: row;")
                .child(
                    button("T.O. PERF")
                        .style("width: 160px; margin-left: 150px; margin-bottom: 10px; height: 40px;")
                        .node(),
                )
                .child(div("flex-grow: 1"))
                .child(
                    button("CPNY T.O.<br />REQUEST")
                        .disabled(true)
                        .style("margin-right: 10px; justify-self: flex-end; width: 175px;")
                        .node(),
                ),
        );
    let sec_block = div(vis(visible_in_sec)).child(button("RETURN").style("margin-right: 5px; width:150px;").node());

    div_c("mfd-page-container", "")
        .child(line1)
        .child(line2)
        .child(line3)
        .child(line4)
        .child(line5)
        .child(line6)
        .child(line7)
        .child(active_block)
        .child(sec_block)
}

use crate::fmt::*;
use crate::w::*;
use mfd_tree::Node;

#[derive(Clone, Debug)]
pub struct DatabaseIdent {
    pub provider: String,
    pub airac_cycle: String,
    pub effective_from: String,
    pub effective_to: String,
}

#[derive(Clone, Debug)]
pub struct StoredElements {
    pub waypoints: u32,
    pub routes: u32,
    pub navaids: u32,
    pub runways: u32,
}

#[derive(Clone, Debug)]
pub struct DataStatusModel {
    pub selected_tab: usize,
    pub idle_factor: f64,
    pub perf_factor: f64,
    pub fuel_penalty_pct: Option<f64>,
    pub fuel_penalty_entered_by_pilot: bool,
    pub database: Option<DatabaseIdent>,
    pub stored: Option<StoredElements>,
    pub swap_confirm_visible: bool,
}

impl Default for DataStatusModel {
    fn default() -> Self {
        DataStatusModel {
            selected_tab: 0,
            idle_factor: 0.0,
            perf_factor: 0.0,
            fuel_penalty_pct: None,
            fuel_penalty_entered_by_pilot: false,
            database: None,
            stored: None,
            swap_confirm_visible: false,
        }
    }
}

pub fn calculate_active_date(d: &DatabaseIdent) -> String {
    let sub = |s: &str, a: usize, b: usize| s.get(a..b).unwrap_or("").to_string();
    let eff_day = d.effective_from.get(8..).unwrap_or("").to_string();
    let exp_day = d.effective_to.get(8..).unwrap_or("").to_string();
    format!(
        "{}{}-{}{}",
        eff_day,
        month_abbr(&sub(&d.effective_from, 5, 7)),
        exp_day,
        month_abbr(&sub(&d.effective_to, 5, 7))
    )
}

pub fn data_status(m: &DataStatusModel) -> Node {
    let dates = m.database.as_ref().map(calculate_active_date).unwrap_or_default();
    let serial = m
        .database
        .as_ref()
        .map(|d| format!("{}{}0001", d.provider.chars().take(2).collect::<String>().to_uppercase(), d.airac_cycle))
        .unwrap_or_default();
    let cnt = |v: Option<u32>| pad_start(&v.unwrap_or(0).to_string(), 2, '0');
    let st = m.stored.as_ref();
    let total = st.map(|s| s.waypoints + s.routes + s.navaids + s.runways).unwrap_or(0);
    let fuel_text = match m.fuel_penalty_pct {
        Some(v) => format!("+{}", pad_start(&to_fixed(v, 1), 5, '0')),
        None => "+000.0".to_string(),
    };

    let swap_label = Node::new("div")
        .style("display: flex; flex-direction: row; justify-content: space-between; width: 100px; height: 40px;")
        .child(Node::new("span").style("display: flex; align-items: center; justify-content: center; margin-left: 10px;").text("SWAP"))
        .child(Node::new("span").style("display: flex; align-items: center; justify-content: center;").text(&format!("{}{}*", NBSP, NBSP)));
    let delete_label = Node::new("div")
        .style("display: flex; flex-direction: row; justify-content: space-between; width: 175px; height: 40px;")
        .child(Node::new("span").style("display: flex; align-items: center; justify-content: center;").text("DELETE ALL"))
        .child(Node::new("span").style("display: flex; align-items: center; justify-content: center;").text("*"));

    let fuel_classes: &[&'static str] = if m.fuel_penalty_entered_by_pilot { &[] } else { &["computedByFms"] };
    let acft = vec![
        Node::el("div", "mfd-data-status-airframe-label mfd-value bigger").text(&format!("A380-800{}/{}TRENT 972", NBSP, NBSP)),
        Node::el("div", "mfd-data-status-performance-row")
            .style("width:655px; margin-bottom: 10px;")
            .child(label_big("IDLE").style("margin-right: 25px;"))
            .child(value_big(&signed_factor(m.idle_factor)).style("margin-right: 60px;"))
            .child(label_big("PERF").style("margin-right: 25px;"))
            .child(value_big(&signed_factor(m.perf_factor)).style("margin-right: 95px;"))
            .child(
                Node::new("div")
                    .style("display: flex; justify-content: center;")
                    .child(button("MODIFY", &Btn::new().disabled(true).style("width: 125px;"))),
            ),
        Node::el("div", "mfd-data-status-performance-row")
            .style("width:380px; margin-bottom: 10px;")
            .child(label_big(&format!("FUEL PENALTY{}", NBSP)))
            .child(input_field(&Field::new(&fuel_text).units("", "%").cls(fuel_classes).style("width: 155px;").align("center").min_width(108))),
        Node::el("div", "mfd-data-status-first-section-divider"),
        Node::el("div", "mfd-data-status-second-section")
            .child(
                Node::new("div")
                    .style("position: relative; display: flex; justify-content: center; width: 100%;")
                    .child(confirmation_dialog(
                        m.swap_confirm_visible,
                        &format!("SWAP{}?", NBSP),
                        "width: 325px; height: 165px; transform: translateX(-50%);",
                    )),
            )
            .child(
                Node::new("div")
                    .style("margin-bottom: 15px; margin-left: 10px;")
                    .child(label_big("NAV DATABASE").style("margin-right: 25px; "))
                    .child(value_big(&serial)),
            )
            .child(
                Node::new("div")
                    .style("width: 100%; display: flex; flex-direction: row; align-items: center; margin-bottom:10px;")
                    .child(
                        Node::new("div")
                            .style("border: 1px solid lightgrey; padding:20px 15px 15px 15px; display: flex; flex-direction: column; height:117px; margin-right:70px;")
                            .child(Node::el("div", "mfd-label bigger").style("display: flex; justify-content: center; margin-bottom: 25px;").text("ACTIVE"))
                            .child(Node::new("div").child(value_big(&dates))),
                    )
                    .child(
                        Node::new("div")
                            .style(" margin-right:55px;")
                            .child(button_label_node(swap_label, &Btn::new().style("width: 140px; height: 62px;"))),
                    )
                    .child(
                        Node::new("div")
                            .style("display: flex; flex-direction: column; margin-top:8px;")
                            .child(Node::el("div", "mfd-label bigger").style("display: flex; justify-content: center;").text("SECOND"))
                            .child(value("").style("margin-top:27px;").text(&format!("{}{}{}", NBSP, NBSP, dates))),
                    ),
            ),
        Node::el("div", "mfd-data-status-third-section")
            .child(Node::new("div").style("margin-bottom: 17px;").child(label_big("PILOT STORED ELEMENTS").style("margin-left: 19px;")))
            .child(
                Node::new("div")
                    .style("width: 100%; display: flex; flex-direction: row; justify-content: space-between; align-items: center;")
                    .child(
                        Node::new("div")
                            .style("flex: 3; display: grid; grid-template-columns: 52% 22% 40% 10%; margin-right: 200px;")
                            .child(Node::el("div", "mfd-label bigger mfd-data-status-pse-label").text("WAYPOINTS"))
                            .child(Node::el("div", "mfd-value bigger mfd-data-status-pse-value").text(&cnt(st.map(|s| s.waypoints))))
                            .child(Node::el("div", "mfd-label bigger mfd-data-status-pse-label").text("ROUTES"))
                            .child(Node::el("div", "mfd-value bigger mfd-data-status-pse-value").text(&cnt(st.map(|s| s.routes))))
                            .child(Node::el("div", "mfd-label bigger mfd-data-status-pse-label no-margin").text("NAVAIDS"))
                            .child(Node::el("div", "mfd-value bigger mfd-data-status-pse-value no-margin").text(&cnt(st.map(|s| s.navaids))))
                            .child(Node::el("div", "mfd-label bigger mfd-data-status-pse-label no-margin").text("RUNWAYS"))
                            .child(Node::el("div", "mfd-value bigger mfd-data-status-pse-value no-margin").text(&cnt(st.map(|s| s.runways)))),
                    )
                    .child(Node::new("div").style("flex: 1;").child(button_label_node(delete_label, &Btn::new().disabled(total == 0)))),
            ),
    ];
    let t = Tabs::new(&["ACFT STATUS", "FMS P/N"], m.selected_tab).angle(25.0);
    page_container(vec![
        top_tab_navigator(
            &t,
            vec![tab_page(m.selected_tab == 0, Some("height: 735px;"), acft), tab_page(m.selected_tab == 1, None, vec![])],
        ),
        spacer(),
    ])
}

use crate::w::*;
use mfd_tree::Node;
use serde::Deserialize;

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct ConnectModel {
    pub notify_to_atc: DropdownModel,
    pub active_atc: String,
    pub next_atc: String,
    pub adsc_armed: bool,
    pub adsc_emer_armed: bool,
    pub connected_stations: String,
    pub footer: FooterModel,
}

fn row(cls: &str) -> Node {
    Node::el("div", cls)
}

pub fn connect_body(m: &ConnectModel) -> Node {
    let notify_btn = button(
        &ButtonOpts::new("NOTIFY").style("margin-right: 10px; width: 120px;").disabled(true),
    );
    let row1 = row("mfd-atccom-connect-row")
        .child(
            row("mfd-atccom-connect-col-1")
                .child(Node::el("div", "mfd-atccom-connect mfd-label").text("NOTIFY TO ATC :")),
        )
        .child(row("mfd-atccom-connect-col-2").child(dropdown(
            &DropdownOpts {
                id_prefix: "CAPT_MFD_notifyToAtcDropdown",
                container_style: "width: 150px;",
                align: "center",
                digits: 4,
            },
            &m.notify_to_atc,
        )))
        .child(
            row("mfd-atccom-connect-col-3")
                .child(Node::new("div").style("display: flex; justify-content: center").child(notify_btn)),
        );
    let row2 = row("mfd-atccom-connect-row")
        .style("margin-top: 10px")
        .child(row("mfd-atccom-connect-col-1"))
        .child(
            row("mfd-atccom-connect-col-2").child(Node::el("div", "mfd-atccom-connect mfd-label").text("NOTIFY")),
        );
    let atc_row = |label: &str, value: &str| {
        row("mfd-atccom-connect-atc-row")
            .child(row("mfd-atccom-connect-col-1").child(Node::el("div", "mfd-atccom-connect mfd-label").text(label)))
            .child(row("mfd-atccom-connect-col-2").child(Node::el("span", "mfd-atccom-connect mfd-value").text(value)))
    };
    let row3 = row("mfd-atccom-connect-row mfd-atccom-connect-section-1")
        .child(
            Node::new("div")
                .style("width: 500px")
                .child(atc_row("ACTIVE ATC :", &m.active_atc))
                .child(atc_row("NEXT ATC :", &m.next_atc)),
        )
        .child(
            row("mfd-atccom-connect-col-3")
                .child(Node::new("div").child(button(
                    &ButtonOpts::new("DISCONNECT ALL").style("width: 260px; margin-top: 20px").disabled(true),
                )))
                .child(Node::new("div").child(button(
                    &ButtonOpts::new("MODIFY<br /> MAX UPLINK DELAY")
                        .style("width: 260px; margin-top: 30px")
                        .disabled(true),
                ))),
        );
    let adsc_block = |label: &str, btn_cls: &str, armed: bool| {
        row("mfd-atccom-connect-adsc-block")
            .child(Node::el("div", "mfd-atccom-connect mfd-label").text(label))
            .child(row(btn_cls).child(toggle_button("mfd-adsc-button", armed, false, "ARMED", "OFF", "")))
    };
    let row4 = row("mfd-atccom-connect-row adsc")
        .child(adsc_block("ADS-C", "mfd-atccom-connect-adscbutton", m.adsc_armed))
        .child(adsc_block("ADS-C EMERGENCY", "mfd-atccom-connect-adscEmerbutton", m.adsc_emer_armed));
    let row5 = row("mfd-atccom-connect-row")
        .child(
            Node::el("div", "mfd-atccom-connect mfd-label")
                .style("margin-left: 110px")
                .text("ADS-C CONNECTED GROUND STATIONS :"),
        )
        .child(Node::el("div", "mfd-label").style("margin-left: 23px").text(&m.connected_stations));
    let row6 = row("mfd-atccom-connect-row")
        .style("justify-content: center; margin-top: 10px")
        .child(row("mfd-atccom-connect-connected-centers-list"));
    Node::el("div", "mfd-page-container").child(row1).child(row2).child(row3).child(row4).child(row5).child(row6)
}

pub fn connect_page(m: &ConnectModel) -> Node {
    navigator("CONNECT", connect_body(m), vec![inop_overlay()], footer(&m.footer))
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct RequestModel {
    pub open_menu: Option<usize>,
    pub menus_right_set: Vec<bool>,
    pub frames: Vec<RequestFrame>,
    pub footer: FooterModel,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct RequestFrame {
    pub index: usize,
    pub html_tree_json: String,
}

struct RequestMenuDef {
    label: &'static str,
    id_prefix: &'static str,
    items: &'static [&'static str],
}

const REQUEST_MENUS: &[RequestMenuDef] = &[
    RequestMenuDef {
        label: "VERTICAL",
        id_prefix: "Request_Vertical_Menu",
        items: &["CLIMB TO", "DESCEND TO", "ALT/FL", "BLOCK ALT/FL", "CRUISE CLIMB", "ITP"],
    },
    RequestMenuDef {
        label: "LATERAL",
        id_prefix: "Request_Lateral_Menu",
        items: &[
            "DIRECT TO",
            "OFFSET",
            "WX DEVIATION",
            "HEADING",
            "TRACK",
            "SID/STAR",
            "TAILORED ARRIVAL",
            "REROUTING",
        ],
    },
    RequestMenuDef { label: "SPEED", id_prefix: "Request_Speed_Menu", items: &["SPEED", "SPEED RANGE"] },
    RequestMenuDef {
        label: "CLEARANCE",
        id_prefix: "Request_Clearance_Menu",
        items: &["DEPARTURE", "OCEANIC", "GENERIC"],
    },
    RequestMenuDef {
        label: "WHEN CAN<br />WE EXPECT",
        id_prefix: "Request_WhenCanWeExpect_Menu",
        items: &[
            "HIGHER LEVEL",
            "LOWER LEVEL",
            "CLIMB TO",
            "DESCEND TO",
            "CRUISE CLIMB",
            "SPEED",
            "SPEED RANGE",
            "BACK ON ROUTE",
        ],
    },
    RequestMenuDef {
        label: "OTHER",
        id_prefix: "Request_Other_Menu",
        items: &["FREETEXT", "VOICE CONTACT", "OWN SEPARATION & VMC", "VMC DES"],
    },
    RequestMenuDef {
        label: "ADD TEXT",
        id_prefix: "Request_AddText_Menu",
        items: &[
            "DUE TO WEATHER",
            "DUE TO A/C PERFORMANCE",
            "DUE TO TURBULENCE",
            "DUE TO TECHNICAL",
            "DUE TO MEDICAL",
            "AT PILOTS DISCRETION",
            "FREETEXT",
        ],
    },
];

fn request_menu_button(def: &RequestMenuDef, open: bool) -> Node {
    let arrow_svg = {
        let mut n = Node::new("svg");
        n.svgattr.insert("height".into(), "15".into());
        n.svgattr.insert("width".into(), "15".into());
        let mut a = std::collections::BTreeMap::new();
        a.insert("points".to_string(), "0,7.5 15,15 15,0".to_string());
        a.insert("style".to_string(), "fill:white".to_string());
        a.insert("_d".to_string(), "0".to_string());
        a.insert("_display".to_string(), "inline".to_string());
        n.shapes.push(mfd_tree::Shape { t: "polygon".into(), a });
        n
    };
    let label = label_nodes(Node::new("span"), def.label);
    let btn = Node::el("span", "mfd-button").class_if("opened", open).style("width:180px; padding-right: 5px;").child(
        Node::el("div", "mfd-fms-fpln-button-dropdown")
            .child(Node::el("span", "mfd-fms-fpln-button-dropdown-arrow").child(arrow_svg))
            .child(Node::el("span", "mfd-atccom-request-button-dropdown-label").child(label)),
    );
    let mut items = Node::new("div");
    for (i, it) in def.items.iter().enumerate() {
        items = items.child(
            Node::el("span", "mfd-dropdown-menu-element").id(&format!("{}_{}", def.id_prefix, i)).text(it),
        );
    }
    let display = if open { "block" } else { "none" };
    Node::el("div", "mfd-dropdown-container").child(btn).child(
        Node::el("div", "mfd-atccom-request-button-dropdown-menu")
            .style(&format!("display: {display}; top: 0px; right: 180px;"))
            .child(items),
    )
}

pub fn request_body(m: &RequestModel) -> Node {
    let mut body = Node::new("div").id("atccom-request-body");
    for f in &m.frames {
        body = body.child(Node::el("div", "request-frame").id(&format!("request-frame-{}", f.index)));
    }
    let mut menu = Node::new("div").id("atccom-request-menu");
    for (i, def) in REQUEST_MENUS.iter().enumerate() {
        if i == 6 {
            menu = menu.child(Node::new("div").style("min-height: 40px;"));
        }
        menu = menu.child(request_menu_button(def, m.open_menu == Some(i)));
    }
    let footer_row = Node::el("div", "mfd-atccom-request-footer")
        .child(Node::new("div").child(button(&ButtonOpts::new("CANCEL").style("width: 190px; height:64px;"))))
        .child(
            Node::new("div")
                .style("position:absolute; right:0px")
                .child(button(&ButtonOpts::new("XFR<br /> TO MAILBOX").style("width: 190px; height:64px;"))),
        );
    Node::el("div", "mfd-page-container")
        .child(Node::new("div").id("atccom-request-container").child(body).child(menu))
        .child(footer_row)
}

pub fn request_page(m: &RequestModel) -> Node {
    navigator("CONNECT", request_body(m), vec![], atccom_footer(&m.footer))
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct MsgRecordModel {
    pub footer: FooterModel,
}

pub fn msg_record_body() -> Node {
    let all = button(&ButtonOpts::new("ALL MSG").style("width:205px; height:64px"));
    let mon = button(&ButtonOpts::new("MONITORED MSG").style("width:205px; height:64px; padding: 9px 5px 5px 5px;"));
    Node::el("div", "mfd-page-container").style("position:relative; width:100%").child(
        Node::new("div")
            .style("position:absolute; top:90px; width:100%; height:275px; border-top: 1px solid #fff; border-bottom: 1px solid #fff")
            .child(Node::new("div").style("position:absolute; top:50px; left:65px;").child(all))
            .child(Node::new("div").style("position:absolute; top:165px; left:65px;").child(mon)),
    )
}

pub fn msg_record_page(m: &MsgRecordModel) -> Node {
    navigator("MSG RECORD", msg_record_body(), vec![], footer(&m.footer))
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct DAtisBlockModel {
    pub icao: Option<String>,
    pub type_dropdown: DropdownModel,
    pub atis_code: String,
    pub timestamp: String,
    pub new_atis: bool,
    pub auto_update: bool,
    pub message: Option<String>,
    pub status_label: Option<String>,
    pub update_menu_open: bool,
    pub update_menu_right_set: bool,
    pub icao_editing: bool,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct DAtisModel {
    pub blocks: Vec<DAtisBlockModel>,
    pub footer: FooterModel,
}

fn svg_node(attrs: &[(&str, &str)], shapes: &[(&str, &[(&str, &str)])]) -> Node {
    let mut n = Node::new("svg");
    for (k, v) in attrs {
        n.svgattr.insert(k.to_string(), v.to_string());
    }
    for (t, a) in shapes {
        let mut map = std::collections::BTreeMap::new();
        for (k, v) in *a {
            map.insert(k.to_string(), v.to_string());
        }
        map.insert("_d".into(), "0".into());
        map.insert("_display".into(), "inline".into());
        n.shapes.push(mfd_tree::Shape { t: t.to_string(), a: map });
    }
    n
}

fn datis_icon(cls: &str, visible: bool, svg: Node) -> Node {
    Node::el("div", &format!("d-atis-icon {cls}"))
        .style(if visible { "visibility: visible;" } else { "visibility: hidden;" })
        .child(svg)
}

pub fn datis_block(index: usize, b: &DAtisBlockModel) -> Node {
    let icao_empty = b.icao.as_deref().map(|s| s == "----").unwrap_or(true);
    let is_dep = b.type_dropdown.selected == Some(0);
    let auto_update_not_allowed = is_dep || icao_empty;
    let dropdown_menu_visible = b.status_label.is_none() && b.message.is_none();
    let combined_visible = !dropdown_menu_visible;
    let status_visible = b.status_label.as_deref().map(|s| !s.is_empty()).unwrap_or(false);
    let read_more_visible = b.message.is_some();

    let icao_text = match b.icao.as_deref() {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => "----".to_string(),
    };
    let icao_classes = if b.icao_editing { "editing valueSelected" } else { "" };
    let icao_field = input_field_text(
        &icao_text,
        "width: 106px; margin-left: 5px; position: absolute; top: 12px; height:40px",
        "center",
        4,
        "",
        "",
        icao_classes,
    );
    let type_dd = dropdown(
        &DropdownOpts {
            id_prefix: &format!("CAPT_MFD_atisTypeList_{index}"),
            container_style: "width: 106px; top: 5px; left: 115px;",
            align: "flex-start",
            digits: 3,
        },
        &b.type_dropdown,
    );
    let new_icon = datis_icon(
        "new-atis-symbol",
        b.new_atis,
        svg_node(
            &[("width", "40"), ("height", "30"), ("viewbox", "0 0 40 28"), ("fill", "none")],
            &[
                ("rect", &[("x", "0"), ("y", "0"), ("width", "40"), ("height", "30"), ("fill", "white")]),
                (
                    "rect",
                    &[("x", "2"), ("y", "2"), ("width", "36"), ("height", "25"), ("stroke", "black"), ("stroke-width", "1.5"), ("fill", "none")],
                ),
                ("path", &[("d", "M 2 2 l 16 15 h 4 l 16 -15"), ("stroke", "black"), ("stroke-width", "1.5"), ("fill", "none")]),
                ("path", &[("d", "M 2 27 l 13.8 -11.7"), ("stroke", "black"), ("stroke-width", "1.5"), ("fill", "none")]),
                ("path", &[("d", "M 38 27 l -13.8 -11.7"), ("stroke", "black"), ("stroke-width", "1.5"), ("fill", "none")]),
            ],
        ),
    );
    let auto_update_icon = datis_icon(
        "auto-update-symbol",
        b.auto_update,
        svg_node(
            &[("width", "38"), ("height", "38"), ("viewbox", "0 0 40 37"), ("fill", "none")],
            &[
                (
                    "path",
                    &[("d", "M 36.8 25.84 A 18.24 18.24 0 1 1 37.24 12.54"), ("fill", "none"), ("stroke", "#0f0"), ("stroke-width", "3")],
                ),
                ("polygon", &[("points", "30.5,13.68 39,13.68 39,3.8"), ("fill", "#0f0")]),
            ],
        ),
    );
    let auto_print_icon = datis_icon(
        "auto-print-symbol",
        false,
        svg_node(
            &[("width", "38"), ("height", "38"), ("fill", "none")],
            &[
                ("path", &[("fill", "#aaa"), ("stroke", "#fff"), ("d", "M1 30.3h29.7v-9l-3.9-3.9H6.2l-4.5 3.9v9"), ("style", "stroke-width:1.28822")]),
                ("path", &[("fill", "#aaa"), ("stroke", "#fff"), ("d", "m30.7 30.3 4.5-5.1v-9l-4.5 5v9.1"), ("style", "stroke-width:1.28822")]),
                ("path", &[("fill", "#aaa"), ("stroke", "#fff"), ("d", "m3.6 30.3 2.6 3.9h19.3l2.6-3.9H3.6"), ("style", "stroke-width:1.28822")]),
                ("path", &[("fill", "#aaa"), ("stroke", "#fff"), ("d", "m25.5 34.2 5.2-3.9H28l-2.6 3.9"), ("style", "stroke-width:1.28822")]),
                ("path", &[("fill", "#ccc"), ("stroke", "#fff"), ("d", "M35.8 14.8 32 12.3l-5.2 5.1 3.9 3.9 5.1-6.5"), ("style", "stroke-width:1.28822")]),
                ("path", &[("fill", "#ccc"), ("stroke", "#fff"), ("d", "m6.2 17.4 12.9-5.1h11.6l-5.2 5.1H6.2"), ("style", "stroke-width:1.28822")]),
                ("path", &[("fill", "#000"), ("stroke", "#fff"), ("d", "m8.8 17.4 6.4-11.6H32l-6.5 11.6H8.8"), ("style", "stroke-width:1.28822")]),
                ("path", &[("stroke", "#0f0"), ("d", "M15.9 9.7h11.6M13.3 13.6h11.6"), ("style", "stroke-width:1.28822")]),
                ("path", &[("d", "M1.7 21.5h28.7"), ("style", "fill:#fff;stroke:#fff;stroke-width:1.5777;stroke-opacity:1")]),
            ],
        ),
    );

    let status_btn = button(
        &ButtonOpts::new(b.status_label.as_deref().unwrap_or(" "))
            .style("width: 131px; padding-top: 3px; padding-bottom: 3px; height: 54px;")
            .container("position: absolute; left: 469px; top: 7px;")
            .highlighted(true)
            .visible(status_visible),
    );
    let mut update_print = ButtonOpts::new("UPDATE<br/>OR PRINT")
        .style("width: 159px; padding-left: 5px; padding-top: 3px; padding-bottom: 3px;")
        .container("position: absolute; left: 600px; top: 7px;")
        .visible(combined_visible);
    update_print.id_prefix = format!("CAPT_MFD_datisblock_btn_{index}");
    update_print.menu = Some(vec![
        MenuItem { label: "UPDATE".into(), disabled: icao_empty },
        MenuItem { label: "AUTO UPDATE".into(), disabled: auto_update_not_allowed },
        MenuItem::disabled("PRINT"),
        MenuItem::disabled("AUTO PRINT"),
    ]);
    update_print.menu_open = b.update_menu_open;
    update_print.menu_right_aligned = b.update_menu_right_set || b.update_menu_open;
    let send_request = button(
        &ButtonOpts::new("SEND<br/>REQUEST")
            .style("width: 159px; padding-left: 5px; padding-top: 3px; padding-bottom: 3px;")
            .container("position: absolute; left: 600px; top: 7px;")
            .disabled(icao_empty)
            .visible(dropdown_menu_visible),
    );
    let auto_update_btn = button(
        &ButtonOpts::new("AUTO<br/>UPDATE")
            .style("width: 159px; padding-left: 5px; padding-top: 3px; padding-bottom: 3px;")
            .container("position: absolute; left: 600px; top: 61px;")
            .disabled(auto_update_not_allowed)
            .visible(dropdown_menu_visible),
    );
    let header = Node::el("div", "mfd-atccom-datis-block-header-row")
        .child(icao_field)
        .child(type_dd)
        .child(Node::el("div", "mfd-label atis-version").child(Node::new("span").text(if b.atis_code.is_empty() { " " } else { &b.atis_code })))
        .child(Node::el("div", "mfd-label atis-time").child(Node::new("span").text(if b.timestamp.is_empty() { " " } else { &b.timestamp })))
        .child(new_icon)
        .child(auto_update_icon)
        .child(auto_print_icon)
        .child(Node::new("div").child(status_btn).child(button_menu_fix(update_print)))
        .child(Node::new("div").child(send_request).child(auto_update_btn));
    let read_more = button(
        &ButtonOpts::new(">>>")
            .style("width: 61px; height:33px; padding-left: 16px; position: absolute; right: 55px; top: 213px;")
            .container("position: absolute; top: 0px; width: 100%;")
            .visible(read_more_visible),
    );
    Node::el("div", "mfd-atccom-datis-block")
        .child(header)
        .child(Node::el("div", "mfd-atccom-datis-block-msgarea").text(b.message.as_deref().unwrap_or(" ")))
        .child(Node::new("div").child(read_more))
}

fn button_menu_fix(o: ButtonOpts) -> Node {
    button(&o)
}

pub fn datis_body(m: &DAtisModel) -> Node {
    let mut pc = Node::el("div", "mfd-page-container");
    for (i, b) in m.blocks.iter().enumerate() {
        pc = pc.child(datis_block(i, b));
    }
    pc.child(
        Node::el("div", "mfd-atccom-datis-footer")
            .child(button(
                &ButtonOpts::new("PRINT<br/>ALL")
                    .style("width: 190px;")
                    .container("position: absolute; top: 3px; right: 190px")
                    .disabled(true),
            ))
            .child(button(
                &ButtonOpts::new("UPDATE<br/>ALL")
                    .style("width: 190px;")
                    .container("position: absolute; top: 3px; right: 0px"),
            )),
    )
}

pub fn datis_page(m: &DAtisModel) -> Node {
    navigator("D-ATIS/LIST", datis_body(m), vec![], atccom_footer(&m.footer))
}

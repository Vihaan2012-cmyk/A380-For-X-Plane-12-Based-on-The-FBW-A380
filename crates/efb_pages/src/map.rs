use crate::{button, label, span, Node};
use mfd_tree::Shape;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MapLayerRow {
    pub id: String,
    pub title: String,
    pub on: bool,
    pub in_range: bool,
    pub swatch: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MapVectorLayer {
    pub id: String,
    pub shapes: Vec<Shape>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MapLabel {
    pub x: f64,
    pub y: f64,
    pub text: String,
    pub class: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MapOwnReadout {
    pub gs_kt: f64,
    pub alt_ft: f64,
    pub track_deg: f64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MapPageModel {
    pub width: f64,
    pub height: f64,
    pub projection: String,
    pub track_up: bool,
    pub follow: bool,
    pub zoom: f64,
    pub layers: Vec<MapVectorLayer>,
    pub labels: Vec<MapLabel>,
    pub rows: Vec<MapLayerRow>,
    pub scale_px: f64,
    pub scale_label: String,
    pub own: Option<MapOwnReadout>,
    pub status: String,
    pub airport: Option<String>,
}

const LEGEND: [(&str, &str); 10] = [
    ("ok", "Route"),
    ("dim", "Flown"),
    ("warn", "Own ship"),
    ("ok", "Online traffic"),
    ("grey", "AI traffic"),
    ("warn", "Proximate traffic"),
    ("dim", "Stale position"),
    ("ok", "Light precipitation"),
    ("warn", "Moderate precipitation"),
    ("bad", "Heavy precipitation"),
];

fn ctl(act: &str, classes: &str, text: &str) -> Node {
    button(classes, text).attr("data-act", act)
}

fn stage(m: &MapPageModel) -> Node {
    let px = |v: f64| format!("{v:.0}");
    let mut st = Node::el("div", "map-stage")
        .attr("data-act", "map:canvas")
        .attr("data-proj", &m.projection)
        .style(&format!("width:{}px;height:{}px", px(m.width), px(m.height)));
    for l in &m.layers {
        let mut svg = Node::el("svg", &format!("map-layer map-layer-{}", l.id));
        svg.svgattr.insert("viewBox".into(), format!("0 0 {} {}", px(m.width), px(m.height)));
        svg.svgattr.insert("width".into(), px(m.width));
        svg.svgattr.insert("height".into(), px(m.height));
        svg.shapes = l.shapes.clone();
        st = st.child(svg);
    }
    let labels = m
        .labels
        .iter()
        .map(|l| span(&l.class, &l.text).style(&format!("left:{:.1}px;top:{:.1}px", l.x, l.y)));
    st = st.child(Node::el("div", "map-labels").children(labels));
    if m.own.is_none() {
        st = st.child(Node::el("div", "map-empty").child(label("map-empty-t", "Waiting for the aircraft position")));
    }
    let mut ctls = Node::el("div", "map-ctl")
        .child(ctl("map:zoom-in", "map-btn", "+"))
        .child(ctl("map:zoom-out", "map-btn", "-"))
        .child(ctl("map:orient", "map-btn map-orient", if m.track_up { "TRK UP" } else { "N UP" }).class_if("on", m.track_up))
        .child(ctl("map:follow", "map-btn map-follow", "FOLLOW").class_if("on", m.follow));
    if let Some(a) = &m.airport {
        ctls = ctls.child(span("map-apt mono", a));
    }
    st = st.child(ctls);
    st = st.child(
        Node::el("div", "map-scale")
            .child(Node::el("div", "map-scale-bar").style(&format!("width:{:.0}px", m.scale_px)))
            .child(span("map-scale-t mono", &m.scale_label)),
    );
    if let Some(o) = &m.own {
        st = st.child(
            Node::el("div", "map-own")
                .child(Node::el("div", "home-kv").child(Node::new("span").text("GS")).child(span("v mono", &format!("{:.0} kt", o.gs_kt))))
                .child(Node::el("div", "home-kv").child(Node::new("span").text("ALT")).child(span("v mono", &format!("{:.0} ft", o.alt_ft))))
                .child(Node::el("div", "home-kv").child(Node::new("span").text("TRK")).child(span("v mono", &format!("{:03.0}", o.track_deg.rem_euclid(360.0))))),
        );
    }
    st
}

fn side(m: &MapPageModel) -> Node {
    let rows = m.rows.iter().map(|r| {
        let cls = format!("map-row{}{}", if r.on { " on" } else { "" }, if r.in_range { "" } else { " out" });
        let mut b = Node::el("button", &cls)
            .attr("data-act", &format!("map:layer:{}", r.id))
            .child(span(&format!("map-dot {}", r.swatch), ""))
            .child(span("map-row-t", &r.title));
        if r.on && !r.in_range {
            b = b.child(span("map-row-hint dim", "Zoom"));
        }
        b.child(Node::el("span", "map-switch").class_if("on", r.on).child(span("map-knob", "")))
    });
    let legend = LEGEND
        .iter()
        .map(|(c, t)| Node::el("div", "map-leg").child(span(&format!("map-dot {c}"), "")).child(span("map-leg-t", t)));
    Node::el("div", "map-side")
        .child(Node::el("div", "box").child(label("home-sec", "Layers")).children(rows))
        .child(Node::el("div", "box").child(label("home-sec", "Legend")).children(legend))
        .child(label("map-status mono dim", &m.status))
}

pub fn map_page(m: &MapPageModel) -> Node {
    Node::el("div", "map-page").id("map-content").child(stage(m)).child(side(m))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Kid;

    fn find<'a>(n: &'a Node, cls: &str, out: &mut Vec<&'a Node>) {
        if n.cls.iter().any(|c| c == cls) {
            out.push(n);
        }
        for k in &n.kids {
            if let Kid::Node(c) = k {
                find(c, cls, out);
            }
        }
    }

    #[test]
    fn map_page_has_stage_layers_controls_and_rows() {
        let m = MapPageModel {
            width: 1000.0,
            height: 760.0,
            projection: "merc;0;0;8;0;1000;760".into(),
            follow: true,
            layers: vec![MapVectorLayer { id: "route".into(), shapes: vec![Shape { t: "line".into(), a: Default::default() }] }, MapVectorLayer { id: "own".into(), shapes: vec![] }],
            labels: vec![MapLabel { x: 10.0, y: 20.0, text: "DESDI".into(), class: "map-lbl wpt".into() }],
            rows: vec![
                MapLayerRow { id: "wx".into(), title: "Weather".into(), on: true, in_range: true, swatch: "ok".into() },
                MapLayerRow { id: "amdb".into(), title: "Airport map".into(), on: true, in_range: false, swatch: "grey".into() },
            ],
            scale_px: 120.0,
            scale_label: "20 NM".into(),
            own: Some(MapOwnReadout { gs_kt: 250.0, alt_ft: 9000.0, track_deg: 5.0 }),
            ..Default::default()
        };
        let t = map_page(&m);
        assert_eq!(t.id.as_deref(), Some("map-content"));
        let mut v = Vec::new();
        find(&t, "map-layer", &mut v);
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].svgattr["viewBox"], "0 0 1000 760");
        let mut v = Vec::new();
        find(&t, "map-btn", &mut v);
        let acts: Vec<&str> = v.iter().map(|n| n.attrs["data-act"].as_str()).collect();
        assert_eq!(acts, ["map:zoom-in", "map:zoom-out", "map:orient", "map:follow"]);
        assert!(v[3].cls.iter().any(|c| c == "on") && !v[2].cls.iter().any(|c| c == "on"));
        let mut v = Vec::new();
        find(&t, "map-row-hint", &mut v);
        assert_eq!(v.len(), 1);
        let mut v = Vec::new();
        find(&t, "map-empty", &mut v);
        assert!(v.is_empty());
        let mut v = Vec::new();
        find(&t, "map-leg", &mut v);
        assert_eq!(v.len(), LEGEND.len());
    }
}

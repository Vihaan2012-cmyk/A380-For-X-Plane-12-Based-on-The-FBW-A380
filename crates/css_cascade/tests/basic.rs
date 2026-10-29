use css_cascade::{parse_html, Cascade};

#[test]
fn cascade_specificity_inherit_var_inline() {
    let doc = parse_html(r#"<html><body><div id="a" class="x y" style="padding:2px"><span class="k">t</span></div><p class="x">u</p></body></html>"#);
    let mut c = Cascade::with_defaults(1646.0, 1024.0);
    c.add_host_sheets();
    c.add_sheet(".x{color:#00ff00;--w:12px;font-size:20px}.x.y{color:rgb(1,2,3)}#a .k{margin-left:var(--w);line-height:1.5}div{border:2px solid red}", 1);
    let st = c.compute(&doc);
    let div = doc.els.iter().position(|e| e.tag == "div").unwrap();
    let span = doc.els.iter().position(|e| e.tag == "span").unwrap();
    assert_eq!(st[div].get("color"), Some("rgb(1, 2, 3)"));
    assert_eq!(st[div].get("padding-top"), Some("2px"));
    assert_eq!(st[div].get("border-top-width"), Some("2px"));
    assert_eq!(st[div].get("border-top-color"), Some("rgb(255, 0, 0)"));
    assert_eq!(st[span].get("color"), Some("rgb(1, 2, 3)"));
    assert_eq!(st[span].get("margin-left"), Some("12px"));
    assert_eq!(st[span].get("line-height"), Some("30px"));
    assert_eq!(st[span].get("display"), Some("inline"));
}

#[test]
fn effects_and_hit_properties_reach_the_document() {
    let doc = parse_html(r#"<html><body><div class="g"><span class="t">x</span></div><p class="p">y</p></body></html>"#);
    let mut c = Cascade::with_defaults(1430.0, 1000.0);
    c.add_host_sheets();
    c.add_sheet(
        ".g{--b:1.12;filter:brightness(var(--b));-webkit-backdrop-filter:blur(40px) saturate(130%);background:#000;background-position:center;transform-origin:right top;pointer-events:none;text-overflow:ellipsis}.p{background-position:left 10px top;text-overflow:clip}",
        1,
    );
    let st = c.compute(&doc);
    let g = doc.els.iter().position(|e| e.tag == "div").unwrap();
    let t = doc.els.iter().position(|e| e.tag == "span").unwrap();
    let p = doc.els.iter().position(|e| e.tag == "p").unwrap();
    assert_eq!(st[g].get("filter"), Some("brightness(1.12)"));
    assert_eq!(st[g].get("backdrop-filter"), Some("blur(40px) saturate(1.3)"));
    assert_eq!(st[g].get("background-position"), Some("50% 50%"));
    assert_eq!(st[g].get("transform-origin"), Some("100% 0%"));
    assert_eq!(st[g].get("pointer-events"), Some("none"));
    assert_eq!(st[g].get("text-overflow"), Some("ellipsis"));
    assert_eq!(st[t].get("pointer-events"), Some("none"), "pointer-events inherits");
    assert_eq!(st[t].get("text-overflow"), Some("clip"), "text-overflow does not inherit");
    assert_eq!(st[t].get("filter"), Some("none"));
    assert_eq!(st[p].get("background-position"), Some("left 10px top"));
    assert_eq!(st[p].get("transform-origin"), Some("50% 50%"));
    assert!(st[g].to_data_cs().contains(";backdrop-filter:blur(40px) saturate(1.3);"));
}

#[test]
fn a_stylesheets_paint_reaches_svg_shapes_and_leaves_their_own_attributes_to_the_painter() {
    let doc = parse_html(r#"<html><body><svg class="cut"><ellipse class="marker"></ellipse><path class="flow" fill="red"></path><line stroke="blue"></line></svg><div class="marker">x</div></body></html>"#);
    let mut c = Cascade::with_defaults(1430.0, 1000.0);
    c.add_host_sheets();
    c.add_sheet(":root{--dim:#808080}.cut .marker{fill:#d6d6e6;stroke:var(--dim);stroke-width:1}.flow{fill:none;stroke-linecap:round}", 1);
    let st = c.compute(&doc);
    let at = |tag: &str| doc.els.iter().position(|e| e.tag == tag).unwrap();
    let marker = st[at("ellipse")].to_data_cs();
    assert!(marker.ends_with(";fill:#d6d6e6;stroke:#808080;stroke-width:1"), "{marker}");
    assert!(st[at("path")].to_data_cs().ends_with(";fill:none;stroke-linecap:round"));
    assert!(!st[at("line")].to_data_cs().contains(";stroke:"), "a presentation attribute stays with the painter");
    assert!(!st[at("div")].to_data_cs().contains(";fill:"), "html elements carry no svg paint");
}

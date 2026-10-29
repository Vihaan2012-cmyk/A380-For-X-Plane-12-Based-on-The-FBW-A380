use mfd_pages_c::vert_rev::*;
use std::fs;

const WPTS: &str = "KADOM,PKS,MACLA,APOMA,POLEV,KALUG,AS,ONOXA,NYOMA,SRITI,BLI,JUNHA,IRTAD,AVPIV,SJ,VJB,AROSO,KALIL,BILIK,GOBAS,PUGER,ANSAX,IGEBO,NOPEK,DUBTA,ELSAR,GIRNA,MMV,OPAMO,GULAM,BBM,MADOG,UDULO,OKILA,ERVIS,GUNDI,DARMI,BISET,ADPOP,BOLIS,TOTOX,LOSIM,KAXEM,MUSRU,PAROK,ITURA,ALMOG,VELOD,GEXAN,TAPRA,VAXAS,RUDAT,MIVEK,IMPED,OMDB";

fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| "E:/fbw-build/mfd-pages-c/c2".to_string());
    let base = VertRevModel {
        waypoints: WPTS.split(',').map(String::from).collect(),
        selected_waypoint: "KADOM".into(),
        alt_cstr_disabled: false,
        crz_fl: "340".into(),
        ..Default::default()
    };
    let loaded = VertRevModel {
        spd_cstr_disabled: false,
        speed_limit_speed: "250".into(),
        speed_limit_alt: "10000".into(),
        delete_spd_limit_disabled: false,
        ..base.clone()
    };
    use VertRevEditing::*;
    let cases: [(&str, &VertRevModel, usize, VertRevEditing, bool); 18] = [
        ("0096", &base, 3, None, false),
        ("0099", &base, 3, AltWpt, false),
        ("0101", &loaded, 1, None, false),
        ("0104", &loaded, 1, SpdWpt, false),
        ("0107", &loaded, 2, None, false),
        ("0110", &loaded, 3, None, false),
        ("0113", &loaded, 3, AltWpt, false),
        ("0115", &loaded, 4, None, false),
        ("0118", &loaded, 4, StepWpt(0), false),
        ("0123", &base, 4, None, true),
        ("0125", &base, 4, StepWpt(0), true),
        ("0128", &loaded, 1, None, true),
        ("0131", &loaded, 1, SpdWpt, true),
        ("0134", &loaded, 2, None, true),
        ("0137", &loaded, 3, None, false),
        ("0138", &loaded, 3, AltWpt, false),
        ("0142", &loaded, 4, None, false),
        ("0145", &loaded, 4, StepWpt(0), false),
    ];
    for (name, m, tab, editing, alt_disabled) in cases {
        let mut m = m.clone();
        m.selected_tab = tab;
        m.editing = editing;
        m.alt_cstr_disabled = alt_disabled;
        m.clb_des_visible = !matches!(name, "0123" | "0125");
        m.step_lines[0].visible = name >= "0115";
        fs::write(format!("{}/{}.json", out, name), vert_rev(&m).to_json()).unwrap();
    }
}

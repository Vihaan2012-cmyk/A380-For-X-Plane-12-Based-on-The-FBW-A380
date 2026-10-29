//! View numbering shared by the plugin and the XPHFBW app
//! (docs/briefs/xphfbw-js-bridge.md, "View numbering"): the index of a
//! panel.cfg `[VCockpitNN]` section among the sections that have an active
//! `htmlgauge00` and are not the EFB/OIT/popup views `src/js/msfs/mod.rs`
//! `Cockpit::new` leaves out. `[VCockpitNN]` sections in the plugin's own
//! QuickJS `Cockpit` and the sessions this module's `view_list` describes are
//! two independent engines (only one draws a given screen at a time,
//! xphfbw-js-bridge.md rule 7), so their internal indices need not match;
//! what must match is that the plugin and the app agree with each other,
//! which is why both call this one function.
//!
//! Free of any dependency gated behind the `js` feature (`mod js`, which
//! `panel.cfg` parsing otherwise lives in, needs `rquickjs`/`oxc`): the
//! XPHFBW app links this crate without that feature (app/Cargo.toml), so
//! this module (and the rest of `xphfbw_bridge.rs`) must build without it.

/// Gauge URLs left out of the panel.cfg-driven views (js::msfs::
/// EXCLUDED_GAUGES, which a `--features js` test below cross-checks this
/// against): the OITs' *legacy* page, the popup, and WASM gauges, which are
/// native modules rather than pages a browser can load, and the EFB, whose
/// gauge XPHFBW does not draw at all (this port never rendered it — even
/// were it excluded here). [`EFB_SCREEN`] gets its own hand-made view
/// instead of one from this list (app/src/views.rs's `spawn_efb_view`): the
/// app's own study/settings UI, not FlyByWire's.
///
/// `A380X/OIT/` is *not* excluded (docs/oit.md): panel.cfg's two OIT
/// sections list `A380X/OIT/oit.html` first and `A380X/OITlegacy/` second,
/// so each OIT view is the current OIS page and the legacy one stays out,
/// exactly as the MFD view takes `A380X/MFD/mfd.html` over the terronnd
/// WASM gauge that precedes it.
pub const EXCLUDED_GAUGES: &[&str] = &["A380X/EFB/", "A380X/OITlegacy/", "A380X/popup/", "WasmInstrument/"];

/// The EFB screen's device id (the converted cockpit mesh's material,
/// `SCREEN_EFB`; docs/screens.md, display/screens.rs's `SCREENS`), and the
/// CSS pixel size XPHFBW's off-screen browser for it is created at
/// (panel.cfg's own `$SCREEN_EFB` gauge is 1430x1000; matched here so the
/// converter's UV mapping for that mesh still lines up with what is drawn on
/// it, even though the browser navigates to the app's own page rather than
/// FlyByWire's `A380X/EFB/efb.html`).
pub const EFB_SCREEN: &str = "SCREEN_EFB";
pub const EFB_WIDTH: u32 = 1430;
pub const EFB_HEIGHT: u32 = 1000;

/// The EFB is drawn at twice its CSS size: XPHFBW lays the page out at
/// 1430x1000 and Chromium paints it at a 2x device scale into a 2860x2000
/// buffer and X-Plane device, so the tablet's small settings text stays
/// sharp when it fills much of the view (it looked low-res at 1x,
/// 2026-09-27). Taps still arrive in CSS pixels: the plugin's per-screen
/// `scale` (device texels over CSS size) maps them back.
pub const EFB_SUPERSAMPLE: u32 = 1;

pub const SCREEN_SUPERSAMPLE: f64 = 1.5;

const NATIVE_SCREENS: &[&str] = &["FCU", "SCREEN_DU_RMP_1", "SCREEN_DU_RMP_2", "SCREEN_DU_RMP_3", "Clock", "BAT", "RTPI", "SCREEN_OIT_LEFT", "SCREEN_OIT_RIGHT", "SCREEN_IFE"];

const STANDBY_SCREENS: &[&str] = &["SCREEN_ISIS_1", "SCREEN_ISIS_2"];

/// Texels per CSS pixel for a screen the native displays draw
/// (`FBW_NATIVE_SCALE`, default 1).
pub fn native_scale() -> f64 {
    static SCALE: std::sync::OnceLock<f64> = std::sync::OnceLock::new();
    *SCALE.get_or_init(|| std::env::var("FBW_NATIVE_SCALE").ok().and_then(|v| v.parse::<f64>().ok()).filter(|v| (0.5..=2.0).contains(v)).unwrap_or(1.0))
}

#[cfg(feature = "native-displays")]
fn drawn_natively(screen_id: &str) -> bool {
    crate::native_displays::screen::selection().target_for(screen_id).is_some()
}

#[cfg(not(feature = "native-displays"))]
fn drawn_natively(_: &str) -> bool {
    false
}

pub fn supersample(screen_id: &str) -> f64 {
    if screen_id.eq_ignore_ascii_case(EFB_SCREEN) {
        EFB_SUPERSAMPLE as f64
    } else if drawn_natively(screen_id) && !STANDBY_SCREENS.iter().any(|s| s.eq_ignore_ascii_case(screen_id)) && !NATIVE_SCREENS.iter().any(|s| s.eq_ignore_ascii_case(screen_id)) {
        native_scale() * glass_factor(screen_id)
    } else if screen_id.eq_ignore_ascii_case(EFB_SCREEN) {
        EFB_SUPERSAMPLE as f64
    } else if NATIVE_SCREENS.iter().any(|s| s.eq_ignore_ascii_case(screen_id)) {
        glass_factor(screen_id)
    } else if STANDBY_SCREENS.iter().any(|s| s.eq_ignore_ascii_case(screen_id)) {
        1.5 * glass_factor(screen_id)
    } else {
        screen_supersample() * glass_factor(screen_id)
    }
}

pub const GLASS_SCREENS_ENV: &str = "FBW_GLASS_SCREENS";
pub const GLASS_SCALE_DEFAULT: f64 = 1.5;

pub fn glass_folder(def: &ViewDef) -> String {
    def.gauge_url.split('/').nth(1).unwrap_or("").to_ascii_lowercase()
}

pub fn glass_selection(raw: &str) -> Vec<String> {
    raw.split(',').map(|s| s.trim().to_ascii_lowercase()).filter(|s| !s.is_empty() && s != "off").collect()
}

pub fn glass_screens_for(selected: &[String], defs: &[ViewDef]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for d in defs {
        if !d.screen.is_empty() && selected.iter().any(|s| s == "all" || *s == glass_folder(d)) && !out.iter().any(|o| o.eq_ignore_ascii_case(&d.screen)) {
            out.push(d.screen.clone());
        }
    }
    out
}

fn glass_config() -> &'static (Vec<String>, f64) {
    static GLASS: std::sync::OnceLock<(Vec<String>, f64)> = std::sync::OnceLock::new();
    GLASS.get_or_init(|| {
        if let Ok(v) = std::env::var(GLASS_SCREENS_ENV) {
            let (scale, list) = v.split_once(';').unwrap_or(("1", ""));
            let scale = scale.parse::<f64>().ok().filter(|s| (1.0..=2.0).contains(s)).unwrap_or(1.);
            return (list.split(',').map(str::trim).filter(|s| !s.is_empty()).map(str::to_string).collect(), scale);
        }
        if crate::xp::system_path().is_none() {
            return (Vec::new(), 1.);
        }
        let settings = crate::app_settings::load_now();
        let raw = std::env::var("XPHFBW_GLASS").unwrap_or_else(|_| crate::app_settings::glass_selection_raw());
        let selected = glass_selection(&raw);
        if selected.is_empty() || settings.glass_scale <= 1. {
            return (Vec::new(), 1.);
        }
        let panel = crate::sound_pack::plugin_dir()
            .and_then(|d| Some(d.parent()?.parent()?.join("panel").join("panel.cfg")))
            .and_then(|p| std::fs::read_to_string(p).ok())
            .unwrap_or_default();
        (glass_screens_for(&selected, &view_list(&panel)), settings.glass_scale)
    })
}

pub fn glass_factor(screen_id: &str) -> f64 {
    let (screens, scale) = glass_config();
    if screens.iter().any(|s| s.eq_ignore_ascii_case(screen_id)) { *scale } else { 1. }
}

pub fn glass_env() -> String {
    let (screens, scale) = glass_config();
    format!("{scale};{}", screens.join(","))
}

pub const SCREEN_SUPERSAMPLE_ENV: &str = "FBW_SCREEN_SUPERSAMPLE";

pub fn screen_supersample() -> f64 {
    static SCALE: std::sync::OnceLock<f64> = std::sync::OnceLock::new();
    *SCALE.get_or_init(|| {
        std::env::var(SCREEN_SUPERSAMPLE_ENV)
            .ok()
            .and_then(|v| v.parse::<f64>().ok())
            .filter(|v| (0.5..=2.0).contains(v))
            .or_else(|| crate::xp::system_path().map(|_| crate::app_settings::load_now().display_scale))
            .unwrap_or(SCREEN_SUPERSAMPLE)
    })
}

/// A screen's device size in texels: its CSS size times its supersampling.
/// Used for the X-Plane device, the shared `ScreenBlock` and the browser's
/// paint buffer alike, so all three agree.
pub fn device_size(screen_id: &str, css_width: u32, css_height: u32) -> (u32, u32) {
    let s = supersample(screen_id);
    ((css_width as f64 * s).round() as u32, (css_height as f64 * s).round() as u32)
}

const SDV2_STYLESHEET: &str = r#"<link rel="stylesheet" href="sdv2.css">"#;

pub fn source_patches() -> Vec<crate::source_patch::SourcePatch> {
    vec![crate::source_patch::SourcePatch {
        path: "/Pages/VCockpit/Instruments/A380X/SDv2/sdv2.html".to_string(),
        find: SDV2_STYLESHEET.to_string(),
        replace: format!("{SDV2_STYLESHEET}\n<style>vcockpit-panel {{ font-family: Ecam, sans-serif; }}</style>"),
        reason: "SDv2 is its own layer page here, so it gets the Ecam panel font MSFS gives it from the legacy SD's CdsDisplayUnit stylesheet".to_string(),
    }]
}

/// One `[VCockpitNN]` section with at least one gauge left after
/// `EXCLUDED_GAUGES` is filtered out: one XPHFBW off-screen browser view
/// (xphfbw-app.md's "one off-screen CEF browser per instrument view").
#[derive(Clone, Debug, PartialEq)]
pub struct ViewDef {
    /// The index the bridge protocol uses for this view (xphfbw_bridge.rs's
    /// `Uplink::Call.view`/`Loaded.view`, `Session.downlinks[view]`).
    pub index: u32,
    /// The panel.cfg section name (e.g. `"VCockpit01"`).
    pub section: String,
    /// The first surviving `htmlgaugeNN`'s URL, relative to
    /// `/Pages/VCockpit/Instruments/` (what the browser navigates to).
    pub gauge_url: String,
    /// The page's pixel size (panel.cfg's `pixel_size`), which is also the
    /// screen's own size when the view has one (display/screens.rs's
    /// `ScreenDef::width/height`).
    pub width: u32,
    pub height: u32,
    /// The panel.cfg `texture` without its leading `$` (display/screens.rs's
    /// `ScreenDef::id`); empty for a screenless view (`NO_TEXTURE`, or no
    /// `texture` line at all — SystemsHost, ExtrasHost).
    pub screen: String,
    pub layer: u32,
}

struct Section {
    name: String,
    texture: Option<String>,
    display: (u32, u32),
    gauge_urls: Vec<String>,
}

/// panel.cfg's `[VCockpitNN]` sections, each with the `htmlgaugeNN` urls it
/// lists, in order. A line commented with a leading `;` (or blank) is
/// skipped, as is anything after a `//` comment; a section that is not a
/// `VCockpit` one (or the file ending) closes the section before it, mirrors
/// `js::msfs::parse_panel_cfg`.
fn parse_sections(text: &str) -> Vec<Section> {
    let mut sections: Vec<Section> = Vec::new();
    for raw in text.lines() {
        let line = raw.split("//").next().unwrap_or("").trim();
        if line.starts_with(';') || line.is_empty() {
            continue;
        }
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            if name.to_ascii_lowercase().starts_with("vcockpit") {
                sections.push(Section { name: name.to_string(), texture: None, display: (0, 0), gauge_urls: Vec::new() });
            } else {
                // A section that is not a view ends the one before it.
                sections.push(Section { name: String::new(), texture: None, display: (0, 0), gauge_urls: Vec::new() });
            }
            continue;
        }
        let Some(section) = sections.last_mut() else { continue };
        if section.name.is_empty() {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else { continue };
        let key = key.trim().to_ascii_lowercase();
        let value = value.split(';').next().unwrap_or("").trim();
        match key.as_str() {
            "pixel_size" => {
                let mut it = value.split(',').map(|n| n.trim().parse::<f64>().unwrap_or(0.) as u32);
                section.display = (it.next().unwrap_or(0), it.next().unwrap_or(0));
            }
            "texture" => {
                let t = value.trim_start_matches('$');
                section.texture = (!t.eq_ignore_ascii_case("NO_TEXTURE") && !t.is_empty()).then(|| t.to_string());
            }
            k if k.starts_with("htmlgauge") => {
                if let Some(url) = value.split(',').next().map(str::trim).filter(|u| !u.is_empty()) {
                    section.gauge_urls.push(url.to_string());
                }
            }
            _ => {}
        }
    }
    sections.retain(|s| !s.name.is_empty());
    sections
}

/// The views panel.cfg describes, numbered as xphfbw-js-bridge.md's "View
/// numbering" says: in section order, only sections with a surviving gauge,
/// index 0 for the first one.
pub fn view_list(panel_cfg: &str) -> Vec<ViewDef> {
    let mut out = Vec::new();
    for section in parse_sections(panel_cfg) {
        let gauges = section.gauge_urls.iter().filter(|url| !EXCLUDED_GAUGES.iter().any(|e| url.to_ascii_lowercase().starts_with(&e.to_ascii_lowercase())));
        let screen = section.texture.unwrap_or_default();
        for (layer, gauge_url) in gauges.enumerate() {
            if layer > 0 && screen.is_empty() {
                break;
            }
            out.push(ViewDef {
                index: out.len() as u32,
                section: section.name.clone(),
                gauge_url: gauge_url.clone(),
                width: section.display.0,
                height: section.display.1,
                screen: screen.clone(),
                layer: layer as u32,
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sdv2_font_patch_matches_the_installed_page_once() {
        let Some(root) = std::env::var_os("FBW_HTML_UI") else { return };
        for p in source_patches() {
            let text = std::fs::read_to_string(std::path::Path::new(&root).join(p.path.trim_start_matches('/'))).unwrap();
            assert_eq!(crate::source_patch::count(&text, &p.find), 1, "{}", p.path);
        }
    }

    const PANEL_CFG: &str = r#"
[VCockpit01]
size_mm=768,1024
pixel_size=768,1024
texture=$SCREEN_DU_PFDL
htmlgauge00=A380X/PFD/pfd.html, 0,0,768,1024

[VCockpit02]
;size_mm=768,1024
pixel_size=768,1024
texture=NO_TEXTURE
htmlgauge00=A380X/SystemsHost/index.html, 0,0,1,1

[VCockpit03]
pixel_size=1024,1024
texture=$SCREEN_EFB
htmlgauge00=A380X/EFB/efb.html, 0,0,1024,1024

; a commented-out view, entirely skipped
[VCockpit04]
pixel_size=512,512
;htmlgauge00=A380X/OIT/oit.html, 0,0,512,512

[VCockpit05]
pixel_size=1646,1024
texture=$SCREEN_DU_MFD
htmlgauge00=WasmInstrument/wasminstrument.html?wasm_gauge=terronnd, 622,0,768,1024
htmlgauge01=A380X/MFD/mfd.html, 0,0,1646,1024

[window_titles]
Window00=Main Panel
"#;

    #[test]
    fn glass_screens_follow_the_folder_selection() {
        let views = view_list(PANEL_CFG);
        assert_eq!(glass_screens_for(&glass_selection("all"), &views), vec!["SCREEN_DU_PFDL".to_string(), "SCREEN_DU_MFD".to_string()]);
        assert_eq!(glass_screens_for(&glass_selection(" PFD "), &views), vec!["SCREEN_DU_PFDL".to_string()]);
        assert!(glass_screens_for(&glass_selection("off"), &views).is_empty());
        assert!(glass_selection("off").is_empty());
    }

    #[test]
    fn views_are_numbered_only_over_sections_with_a_surviving_gauge() {
        let views = view_list(PANEL_CFG);
        // VCockpit03 (EFB) and VCockpit04 (no active htmlgauge00) are left
        // out entirely, so VCockpit05 is index 2, not 4.
        assert_eq!(views.len(), 3);
        assert_eq!(views[0], ViewDef { index: 0, section: "VCockpit01".into(), gauge_url: "A380X/PFD/pfd.html".into(), width: 768, height: 1024, screen: "SCREEN_DU_PFDL".into(), layer: 0 });
        assert_eq!(views[1], ViewDef { index: 1, section: "VCockpit02".into(), gauge_url: "A380X/SystemsHost/index.html".into(), width: 768, height: 1024, screen: String::new(), layer: 0 });
        // The first surviving (non-native) gauge is the MFD's html one, not
        // the excluded WasmInstrument terronnd gauge that precedes it.
        assert_eq!(views[2], ViewDef { index: 2, section: "VCockpit05".into(), gauge_url: "A380X/MFD/mfd.html".into(), width: 1646, height: 1024, screen: "SCREEN_DU_MFD".into(), layer: 0 });
    }

    /// panel.cfg's two OIT sections, verbatim (VCockpit19/20): each lists
    /// the current OIS page first and the superseded legacy one second, so
    /// the view is the current page and the legacy one never runs.
    #[test]
    fn each_oit_view_is_the_current_page_not_the_legacy_one() {
        const OIT_CFG: &str = r#"
[VCockpit19]
size_mm=1333,1000
pixel_size=1333,1000
texture=$SCREEN_OIT_LEFT
htmlgauge00=A380X/OIT/oit.html?Index=1, 0,0,1333,1000
htmlgauge01=A380X/OITlegacy/oitlegacy.html, 0,0,1333,1000

[VCockpit20]
size_mm=1333,1000
pixel_size=1333,1000
texture=$SCREEN_OIT_RIGHT
htmlgauge00=A380X/OIT/oit.html?Index=2, 0,0,1333,1000
htmlgauge01=A380X/OITlegacy/oitlegacy.html, 0,0,1333,1000
"#;
        let views = view_list(OIT_CFG);
        assert_eq!(views.len(), 2);
        assert_eq!(views[0].gauge_url, "A380X/OIT/oit.html?Index=1");
        assert_eq!(views[0].screen, OIT_LEFT_SCREEN);
        assert_eq!((views[0].width, views[0].height), (1333, 1000));
        assert_eq!(views[1].gauge_url, "A380X/OIT/oit.html?Index=2");
        assert_eq!(views[1].screen, OIT_RIGHT_SCREEN);
        // Both are screens the plugin makes a device for, in SCREEN_ORDER.
        for v in &views {
            assert!(SCREEN_ORDER.contains(&v.screen.as_str()), "{} is not in SCREEN_ORDER", v.screen);
        }
    }

    #[test]
    fn the_sd_draws_sdv2_over_the_legacy_pages() {
        const SD_CFG: &str = "[VCockpit04]
pixel_size=768,1024
texture=$SCREEN_DU_SD
htmlgauge00=A380X/SD/sd.html?duID=7, 0,0,768,1024
htmlgauge01=A380X/SDv2/sdv2.html?duID=7, 0,0,768,1024
[VCockpit05]
pixel_size=768,1024
texture=$SCREEN_DU_PFDL
htmlgauge00=A380X/PFD/pfd.html, 0,0,768,1024
";
        let views = view_list(SD_CFG);
        let summary: Vec<(u32, &str, &str, u32)> = views.iter().map(|v| (v.index, v.screen.as_str(), v.gauge_url.as_str(), v.layer)).collect();
        assert_eq!(
            summary,
            vec![
                (0, "SCREEN_DU_SD", "A380X/SD/sd.html?duID=7", 0),
                (1, "SCREEN_DU_SD", "A380X/SDv2/sdv2.html?duID=7", 1),
                (2, "SCREEN_DU_PFDL", "A380X/PFD/pfd.html", 0),
            ]
        );
    }

    #[test]
    fn an_empty_panel_cfg_has_no_views() {
        assert_eq!(view_list(""), Vec::new());
        assert_eq!(view_list("[VCockpit01]\npixel_size=1,1\n"), Vec::new());
    }

    /// Cross-checked against `js::msfs`'s own parsing and exclusion, so the
    /// two independent implementations agree on which sections count and in
    /// what order, without this module depending on the `js` feature.
    #[cfg(feature = "js")]
    #[test]
    fn agrees_with_js_msfs_cockpit_new_s_own_skipping() {
        use crate::js::msfs::{parse_panel_cfg, EXCLUDED_GAUGES as JS_EXCLUDED};
        assert_eq!(EXCLUDED_GAUGES, JS_EXCLUDED);
        let panels = parse_panel_cfg(PANEL_CFG);
        let expected: Vec<String> = panels
            .into_iter()
            .filter(|p| p.gauges.iter().any(|g| !JS_EXCLUDED.iter().any(|e| g.url.to_ascii_lowercase().starts_with(&e.to_ascii_lowercase()))))
            .map(|p| p.name)
            .collect();
        let got: Vec<String> = view_list(PANEL_CFG).into_iter().filter(|v| v.layer == 0).map(|v| v.section).collect();
        assert_eq!(got, expected);
    }

    #[test]
    fn paint_off_mask_marks_exactly_the_named_screens() {
        assert_eq!(paint_off_mask([]), 0);
        let mask = paint_off_mask(["SCREEN_DU_NDL", "screen_du_ndr", "Clock", "SCREEN_NOPE"]);
        for (i, s) in SCREEN_ORDER.iter().enumerate() {
            let want = ["SCREEN_DU_NDL", "SCREEN_DU_NDR", "Clock"].contains(s);
            assert_eq!(mask >> i & 1 == 1, want, "{s}");
        }
        assert_eq!(paint_off_bit("SCREEN_NOPE"), 0);
    }

    const INSTALLED_PANEL_CFG: &str = "
[VCockpit01]
pixel_size=1646,1024
texture=$SCREEN_DU_MFD
htmlgauge00=A380X/MFD/mfd.html?duID=2, 0,0,1646,1024
[VCockpit02]
pixel_size=768,1024
texture=NO_TEXTURE
[VCockpit03]
pixel_size=768,1024
texture=$SCREEN_DU_EWD
htmlgauge00=A380X/EWD/ewd.html?duID=6, 0,0,768,1024
[VCockpit04]
pixel_size=768,1024
texture=$SCREEN_DU_SD
htmlgauge00=A380X/SD/sd.html?duID=7, 0,0,768,1024
htmlgauge01=A380X/SDv2/sdv2.html?duID=7, 0,0,768,1024
[VCockpit05]
pixel_size=768,1024
texture=$SCREEN_DU_PFDL
htmlgauge00=A380X/PFD/pfd.html?Index=1&duID=0, 0,0,768,1024
[VCockpit06]
pixel_size=768,1024
texture=$SCREEN_DU_PFDR
htmlgauge00=A380X/PFD/pfd.html?Index=2&duID=3, 0,0,768,1024
[VCockpit07]
pixel_size=768,1024
texture=$SCREEN_DU_NDL
htmlgauge00=WasmInstrument/WasmInstrument.html?wasm_module=terronnd.wasm&wasm_gauge=terronnd,0,0,768,1024,L
htmlgauge01=A380X/ND/nd.html?Index=1&duID=1, 0,0,768,1024
[VCockpit08]
pixel_size=768,1024
texture=$SCREEN_DU_NDR
htmlgauge00=WasmInstrument/WasmInstrument.html?wasm_module=terronnd.wasm&wasm_gauge=terronnd,0,0,768,1024,R
htmlgauge01=A380X/ND/nd.html?Index=2?duID=4, 0,0,768,1024
[VCockpit09]
pixel_size=2560,1280
texture=$FCU
htmlgauge00=A380X/FCU/FCU.html, 0,0,2560,1280
[VCockpit10]
pixel_size=512,512
texture=SCREEN_ISIS_1
htmlgauge00=A380X/ISISlegacy/isislegacy.html, 0,0,512,512
[VCockpit11]
pixel_size=512,512
texture=SCREEN_ISIS_2
htmlgauge00=A380X/SND/snd.html, 0,0,512,512
[VCockpit12]
pixel_size=256,256
texture=$Clock
htmlgauge00=A380X/Clock/clock.html,	0,0, 256, 256
[VCockpit13]
pixel_size=338,128
texture=$RTPI
htmlgauge00=A380X/RTPI/rtpi.html, 0,0,338,128
[VCockpit14]
pixel_size=256,128
texture=$BAT
htmlgauge00=A380X/BAT/bat.html, 0,0,256,128
[VCockpit15]
pixel_size=1430,1000
texture=$SCREEN_EFB
htmlgauge00=A380X/EFB/efb.html, 0,0,1430,1000
[VCockpit16]
pixel_size=1664,1024
texture=SCREEN_DU_RMP_1
htmlgauge00=A380X/RMP/rmp.html?Index=1, 0,0,1664,1024
[VCockpit19]
pixel_size=1333,1000
texture=$SCREEN_OIT_LEFT
htmlgauge00=A380X/OIT/oit.html?Index=1, 0,0,1333,1000
htmlgauge01=A380X/OITlegacy/oitlegacy.html, 0,0,1333,1000
[VCockpit22]
pixel_size=0,0
texture=NO_TEXTURE
htmlgauge00=A380X/SystemsHost/systems-host.html,0,0,1,1
[VCockpit23]
pixel_size=0,0
texture=NO_TEXTURE
htmlgauge00=A380X/ExtrasHost/extras-host.html
[VCockpit25]
pixel_size=512,256
texture=$SCREEN_IFE
htmlgauge00=A380X/IFE/ife.html, 0,0,512,256
";

    fn opened_pages(native: &[&str]) -> Vec<String> {
        let mask = page_stop_mask(native.iter().copied());
        views_to_open(view_list(INSTALLED_PANEL_CFG), mask).into_iter().map(|v| v.gauge_url.split(['/', '?']).nth(1).unwrap_or("").to_string()).collect()
    }

    #[test]
    fn nothing_is_stopped_when_no_screen_is_native() {
        let all = view_list(INSTALLED_PANEL_CFG);
        assert_eq!(views_to_open(all.clone(), 0), all);
        assert_eq!(page_stop_mask([]), 0);
    }

    #[test]
    fn with_every_screen_native_exactly_the_stateless_pages_are_not_opened() {
        let every_screen: Vec<&str> = INSTALLED_PANEL_CFG.lines().filter_map(|l| l.strip_prefix("texture=")).map(|t| t.trim_start_matches('$')).filter(|t| *t != "NO_TEXTURE").collect();
        let pages = opened_pages(&every_screen);
        assert_eq!(pages, ["MFD", "ND", "ND", "OIT", "SystemsHost", "ExtrasHost"]);
        let before = view_list(INSTALLED_PANEL_CFG);
        let stopped: Vec<&str> = before.iter().filter(|v| page_stopped(&v.screen, page_stop_mask(every_screen.iter().copied()))).map(|v| v.screen.as_str()).collect();
        let mut want: Vec<&str> = PAGE_STOPS_WHEN_NATIVE.to_vec();
        want.sort_unstable();
        let mut got = stopped.clone();
        got.sort_unstable();
        got.dedup();
        assert_eq!(got, want);
    }

    #[test]
    fn only_native_screens_lose_their_page() {
        assert_eq!(opened_pages(&["BAT"]).len(), view_list(INSTALLED_PANEL_CFG).len() - 1);
        assert!(!opened_pages(&["Clock"]).contains(&"Clock".to_string()));
        assert!(opened_pages(&["Clock"]).contains(&"BAT".to_string()));
        assert!(opened_pages(&["SCREEN_DU_MFD", "SCREEN_DU_NDL", "SCREEN_OIT_LEFT"]).len() == view_list(INSTALLED_PANEL_CFG).len());
        let sd = opened_pages(&["SCREEN_DU_SD"]);
        assert_eq!(sd.len(), view_list(INSTALLED_PANEL_CFG).len() - 2, "SD and SDv2 share the screen");
        assert!(!sd.iter().any(|p| p == "SD" || p == "SDv2"));
    }

    #[test]
    fn views_keep_their_index_when_pages_are_stopped() {
        let kept = views_to_open(view_list(INSTALLED_PANEL_CFG), page_stop_mask(["FCU"]));
        let fcu = view_list(INSTALLED_PANEL_CFG).into_iter().find(|v| v.screen == "FCU").unwrap().index;
        assert!(kept.iter().all(|v| v.index != fcu));
        assert!(kept.iter().any(|v| v.index == fcu + 1));
    }

    #[test]
    fn every_screen_has_its_own_paint_off_bit() {
        assert!(SCREEN_ORDER.len() <= 64);
        let all = paint_off_mask(SCREEN_ORDER.iter().copied());
        assert_eq!(all.count_ones() as usize, SCREEN_ORDER.len());
    }
}

/// The cockpit screens in the order both sides number them: the plugin's
/// `display::screens::SCREENS` (checked by its tests) and the `screen` index
/// of `Input` records XPHFBW routes to a view's browser.
pub const SCREEN_ORDER: &[&str] = &[
    "SCREEN_DU_PFDL",
    "SCREEN_DU_NDL",
    "SCREEN_DU_PFDR",
    "SCREEN_DU_NDR",
    "SCREEN_DU_EWD",
    "SCREEN_DU_SD",
    "SCREEN_DU_MFD",
    "FCU",
    "SCREEN_ISIS_1",
    "Clock",
    "RTPI",
    "BAT",
    "SCREEN_DU_RMP_1",
    "SCREEN_DU_RMP_2",
    "SCREEN_DU_RMP_3",
    EFB_SCREEN,
    OIT_LEFT_SCREEN,
    OIT_RIGHT_SCREEN,
    "SCREEN_ISIS_2",
];

/// The bit of `SlotHeader::native_paint_off` that stands for `screen`.
pub fn paint_off_bit(screen: &str) -> u64 {
    SCREEN_ORDER.iter().position(|s| s.eq_ignore_ascii_case(screen)).map_or(0, |i| 1u64 << i)
}

/// The mask for the screens the native displays are drawing.
pub fn paint_off_mask<'a>(screens: impl IntoIterator<Item = &'a str>) -> u64 {
    screens.into_iter().fold(0, |mask, s| mask | paint_off_bit(s))
}

/// Screens whose FlyByWire page is not loaded when the native displays draw
/// the screen: the page writes nothing the plugin does not already own
/// (`native_page_state.rs` holds what it did write). Bit `i` of
/// `SlotHeader::native_views_off` stands for entry `i`. The ND stays: its
/// page also runs OANC, the OANS control panel and BTV.
pub const PAGE_STOPS_WHEN_NATIVE: &[&str] =
    &["FCU", "Clock", "RTPI", "BAT", "SCREEN_IFE", "SCREEN_ISIS_1", "SCREEN_ISIS_2", "SCREEN_DU_PFDL", "SCREEN_DU_PFDR", "SCREEN_DU_EWD", "SCREEN_DU_SD", "SCREEN_DU_RMP_1"];

pub fn page_stop_bit(screen: &str) -> u64 {
    PAGE_STOPS_WHEN_NATIVE.iter().position(|s| s.eq_ignore_ascii_case(screen)).map_or(0, |i| 1u64 << i)
}

pub fn page_stop_mask<'a>(native_screens: impl IntoIterator<Item = &'a str>) -> u64 {
    native_screens.into_iter().fold(0, |mask, s| mask | page_stop_bit(s))
}

pub fn page_stopped(screen: &str, mask: u64) -> bool {
    page_stop_bit(screen) & mask != 0
}

/// The views to open: every view except those of a screen whose page is
/// stopped by `mask`. Each view keeps the `index` it has in the full list.
pub fn views_to_open(defs: Vec<ViewDef>, mask: u64) -> Vec<ViewDef> {
    defs.into_iter().filter(|d| !page_stopped(&d.screen, mask)).collect()
}

/// The two OIT screens' device ids (panel.cfg's `$SCREEN_OIT_LEFT` and
/// `$SCREEN_OIT_RIGHT`, VCockpit19/20), appended to [`SCREEN_ORDER`] so no
/// screen already in it changes index.
pub const OIT_LEFT_SCREEN: &str = "SCREEN_OIT_LEFT";
pub const OIT_RIGHT_SCREEN: &str = "SCREEN_OIT_RIGHT";

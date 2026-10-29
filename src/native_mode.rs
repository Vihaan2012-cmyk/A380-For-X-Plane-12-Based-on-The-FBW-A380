//! Shadow or authoritative, per native system, decided here only.
//!
//! `FBW_NATIVE_SYSTEMS` (else `nativeSystems` in xphfbw.json) names the
//! authoritative systems; `all` takes each group's ready set. `FBW_NATIVE`
//! (else `native`) set to `all` makes every system authoritative and every
//! screen native; so does an install without XPHFBW (a native-only release).
//! `native_shadow.rs` and `native_systems_host.rs` read [`systems_spec`].

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::xphfbw_bridge_views::{page_stopped, ViewDef};

pub const ENV: &str = "FBW_NATIVE";
pub const SETTING: &str = "native";
pub const SYSTEMS_ENV: &str = "FBW_NATIVE_SYSTEMS";
pub const SYSTEMS_SETTING: &str = "nativeSystems";
pub const CEF_NOT_STARTED: &str = "cef: not started (all native)";
pub const XPHFBW_EXE: &str = "XPHFBW/XPHFBW.exe";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
    Shadow,
    Syshost,
    Fms,
}

/// Every native system: its group, name, and whether the group's `all` takes it.
pub const SYSTEMS: &[(Group, &str, bool)] = &[
    (Group::Shadow, "gpws", true),
    (Group::Shadow, "tcas", true),
    (Group::Shadow, "hosts_misc", true),
    (Group::Syshost, "vhf", true),
    (Group::Syshost, "xpdr", true),
    (Group::Syshost, "amu", true),
    (Group::Syshost, "btv", false),
    (Group::Syshost, "efis_taws", true),
    (Group::Syshost, "fws", false),
    (Group::Syshost, "light_sync", true),
    (Group::Syshost, "key_interceptor", true),
    (Group::Syshost, "pilot_seat", false),
    (Group::Syshost, "pushbutton_check", true),
    (Group::Syshost, "baro_unit", false),
    (Group::Syshost, "ready", true),
    (Group::Syshost, "terrain_feed", false),
    (Group::Syshost, "oit_health", true),
    (Group::Syshost, "datalink", false),
    (Group::Fms, "fms", false),
];

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Modes {
    pub native_all: bool,
    pub off: bool,
    pub authoritative: BTreeSet<&'static str>,
}

fn tokens(raw: &str) -> impl Iterator<Item = String> + '_ {
    raw.split(|c: char| c == ',' || c == ';' || c.is_whitespace()).filter(|t| !t.is_empty()).map(str::to_ascii_lowercase)
}

impl Modes {
    pub fn parse(native: &str, systems: &str) -> Modes {
        let mut m = Modes { native_all: tokens(native).any(|t| t == "all"), ..Modes::default() };
        if m.native_all {
            m.authoritative = SYSTEMS.iter().map(|s| s.1).collect();
            return m;
        }
        for t in tokens(systems) {
            match t.as_str() {
                "off" | "none" => m.off = true,
                "all" => m.authoritative.extend(SYSTEMS.iter().filter(|s| s.2).map(|s| s.1)),
                other => m.authoritative.extend(SYSTEMS.iter().find(|s| s.1 == other).map(|s| s.1)),
            }
        }
        m
    }

    pub fn authoritative(&self, name: &str) -> bool {
        !self.off && self.authoritative.contains(name)
    }

    pub fn all_logic_native(&self) -> bool {
        SYSTEMS.iter().all(|s| self.authoritative(s.1))
    }

    /// The text `native_shadow::Config::parse` and `native_systems_host::Config::parse` read.
    pub fn spec(&self) -> String {
        if self.off {
            return "off".to_string();
        }
        SYSTEMS.iter().filter(|s| self.authoritative(s.1)).map(|s| s.1).collect::<Vec<_>>().join(",")
    }

    pub fn log_line(&self) -> String {
        let each = SYSTEMS.iter().map(|s| format!("{}={}", s.1, if self.authoritative(s.1) { "authoritative" } else { "shadow" })).collect::<Vec<_>>().join(" ");
        format!("native modes: native={} {each}", if self.native_all { "all" } else if self.off { "off" } else { "per-system" })
    }
}

fn setting(key: &str) -> String {
    crate::xp::system_path()
        .map(|root| crate::settings_files::app_settings_path(&root))
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .and_then(|v| v.get(key).and_then(serde_json::Value::as_str).map(str::to_string))
        .unwrap_or_default()
}

fn env_or_setting(env: &str, key: &str) -> String {
    std::env::var(env).ok().filter(|v| !v.trim().is_empty()).unwrap_or_else(|| setting(key))
}

fn installed_plugin_dir() -> Option<PathBuf> {
    #[cfg(feature = "js")]
    {
        crate::js_bridge::plugin_dir()
    }
    #[cfg(not(feature = "js"))]
    {
        None
    }
}

/// False only when the plugin folder is known and holds no XPHFBW.exe.
pub fn cef_installed(plugin_dir: Option<&Path>) -> bool {
    plugin_dir.is_none_or(|d| d.join(XPHFBW_EXE).is_file())
}

pub fn native_raw(cef_installed: bool) -> String {
    let raw = env_or_setting(ENV, SETTING);
    if raw.trim().is_empty() && !cef_installed {
        return "all".to_string();
    }
    raw
}

pub fn current() -> Modes {
    let installed = cef_installed(installed_plugin_dir().as_deref());
    Modes::parse(&native_raw(installed), &env_or_setting(SYSTEMS_ENV, SYSTEMS_SETTING))
}

pub fn systems_spec() -> String {
    current().spec()
}

pub fn native_all() -> bool {
    current().native_all
}

/// Why XPHFBW is still needed: logic left in shadow, and views whose page
/// neither stops nor sits on a native-drawn screen. Empty: not needed.
pub fn cef_needed(modes: &Modes, views: &[ViewDef], stop_mask: u64, native_screen: impl Fn(&str) -> bool) -> Vec<String> {
    let mut why = Vec::new();
    let shadow: Vec<&str> = SYSTEMS.iter().map(|s| s.1).filter(|n| !modes.authoritative(n)).collect();
    if !shadow.is_empty() {
        why.push(format!("shadow logic [{}]", shadow.join(",")));
    }
    for v in views {
        if v.screen.is_empty() || page_stopped(&v.screen, stop_mask) || native_screen(&v.screen) {
            continue;
        }
        why.push(format!("{} drawn by its page", v.screen));
    }
    why
}

#[cfg(feature = "native-displays")]
fn screen_is_native(screen: &str) -> bool {
    crate::native_displays::screen::selection().target_for(screen).is_some()
}

#[cfg(not(feature = "native-displays"))]
fn screen_is_native(_: &str) -> bool {
    false
}

/// The launcher's check: logs each system's mode, then whether XPHFBW starts.
/// `aircraft_dir` holds `panel/panel.cfg`; without it XPHFBW starts.
pub fn cef_wanted(aircraft_dir: Option<&Path>) -> bool {
    let modes = current();
    crate::log(&modes.log_line());
    let Some(cfg) = aircraft_dir.and_then(|d| std::fs::read_to_string(d.join("panel").join("panel.cfg")).ok()) else {
        return true;
    };
    let views = crate::xphfbw_bridge_views::view_list(&cfg);
    let why = cef_needed(&modes, &views, stop_mask(), screen_is_native);
    if why.is_empty() {
        crate::log(CEF_NOT_STARTED);
        return false;
    }
    if modes.native_all {
        crate::log(&format!("cef: started for {}", why.join("; ")));
    }
    true
}

#[cfg(feature = "js")]
fn stop_mask() -> u64 {
    crate::native_page_state::stopped_pages_mask()
}

#[cfg(not(feature = "js"))]
fn stop_mask() -> u64 {
    0
}

static NOTICES: Mutex<(BTreeSet<String>, Vec<String>)> = Mutex::new((BTreeSet::new(), Vec::new()));

pub fn notice_text(what: &str) -> String {
    format!("A380 for X-Plane 12: the native {what} failed. Please send X-Plane's Log.txt with your report.")
}

/// Once per `what` for the session: logs, posts to the EFB, and queues an
/// on-screen message for [`show_notices`]. Any thread.
pub fn failure_notice(what: &str, reason: &str) -> bool {
    let Ok(mut n) = NOTICES.lock() else { return false };
    if !n.0.insert(what.to_string()) {
        return false;
    }
    let text = notice_text(what);
    n.1.push(text.clone());
    drop(n);
    crate::log(&format!("native failure notice: {what}: {reason}"));
    crate::notices::post(text);
    true
}

pub fn take_notices() -> Vec<String> {
    NOTICES.lock().map(|mut n| std::mem::take(&mut n.1)).unwrap_or_default()
}

/// Main thread only: shows queued notices with X-Plane's on-screen message.
pub fn show_notices() {
    let pending = take_notices();
    if pending.is_empty() {
        return;
    }
    let Some(f) = crate::xp::Xplm::xplm_symbol("XPLMSpeakString") else { return };
    let speak: unsafe extern "C" fn(*const std::ffi::c_char) = unsafe { std::mem::transmute::<*mut std::ffi::c_void, _>(f) };
    for text in pending {
        if let Ok(c) = std::ffi::CString::new(text) {
            unsafe { speak(c.as_ptr()) };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xphfbw_bridge_views::{page_stop_mask, view_list, PAGE_STOPS_WHEN_NATIVE};

    const CFG: &str = "[VCockpit01]\ntexture=$SCREEN_DU_PFDL\nhtmlgauge00=A380X/PFD/pfd.html, 0,0,1,1\n[VCockpit02]\ntexture=$SCREEN_DU_NDL\nhtmlgauge00=A380X/ND/nd.html, 0,0,1,1\n[VCockpit03]\ntexture=SCREEN_DU_MFD\nhtmlgauge00=A380X/MFD/mfd.html, 0,0,1,1\n[VCockpit04]\ntexture=NO_TEXTURE\nhtmlgauge00=A380X/SystemsHost/systems-host.html,0,0,1,1\n";

    fn views() -> Vec<ViewDef> {
        view_list(CFG)
    }

    #[test]
    fn native_all_makes_every_system_authoritative() {
        let m = Modes::parse("all", "");
        assert!(m.native_all && m.all_logic_native());
        assert!(Modes::parse("ALL", "off").all_logic_native(), "native all wins over nativeSystems");
        assert!(m.log_line().starts_with("native modes: native=all gpws=authoritative"));
    }

    #[test]
    fn systems_all_keeps_each_groups_ready_set() {
        let m = Modes::parse("", "all");
        assert!(m.authoritative("gpws") && m.authoritative("vhf") && !m.authoritative("fws") && !m.authoritative("fms"));
        assert!(!m.all_logic_native());
        assert!(Modes::parse("", "fws, FMS").authoritative("fms"));
        assert_eq!(Modes::parse("", "off,vhf").spec(), "off");
        assert_eq!(Modes::parse("", "").spec(), "");
        assert!(Modes::parse("", "tcas").log_line().contains("tcas=authoritative") && Modes::parse("", "tcas").log_line().contains("gpws=shadow"));
    }

    #[cfg(feature = "js")]
    #[test]
    fn the_table_matches_both_hosts() {
        use crate::native_shadow::{Config as ShadowConfig, System};
        use crate::native_systems_host::{Config as HostConfig, Module};
        let shadow: Vec<&str> = SYSTEMS.iter().filter(|s| s.0 == Group::Shadow).map(|s| s.1).collect();
        assert_eq!(shadow, System::ALL.iter().map(|s| s.name()).collect::<Vec<_>>());
        let host: Vec<&str> = SYSTEMS.iter().filter(|s| s.0 == Group::Syshost && s.1 != crate::datalink_native::NAME).map(|s| s.1).collect();
        assert_eq!(host, Module::ALL.iter().map(|m| m.name()).collect::<Vec<_>>());
        let all = HostConfig::parse("all");
        for m in Module::ALL {
            assert_eq!(all.authoritative(m), SYSTEMS.iter().find(|s| s.1 == m.name()).unwrap().2, "{}", m.name());
        }
        let every = Modes::parse("all", "").spec();
        assert!(Module::ALL.iter().all(|m| HostConfig::parse(&every).authoritative(*m)));
        assert!(System::ALL.iter().all(|s| ShadowConfig::parse(&every).authoritative(*s)));
        let some = Modes::parse("", "vhf,gpws").spec();
        assert!(HostConfig::parse(&some).authoritative(Module::Vhf) && !HostConfig::parse(&some).authoritative(Module::Amu));
        assert!(ShadowConfig::parse(&some).authoritative(System::Gpws) && !ShadowConfig::parse(&some).authoritative(System::Tcas));
    }

    #[test]
    fn cef_is_not_needed_only_when_everything_is_native() {
        let all = Modes::parse("all", "");
        let screens = ["SCREEN_DU_PFDL", "SCREEN_DU_NDL", "SCREEN_DU_MFD"];
        let mask = page_stop_mask(screens);
        assert!(PAGE_STOPS_WHEN_NATIVE.contains(&"SCREEN_DU_PFDL"));
        assert!(cef_needed(&all, &views(), mask, |s| screens.contains(&s)).is_empty());
        let why = cef_needed(&all, &views(), page_stop_mask(["SCREEN_DU_PFDL"]), |s| s == "SCREEN_DU_PFDL" || s == "SCREEN_DU_NDL");
        assert_eq!(why, ["SCREEN_DU_MFD drawn by its page"]);
        let why = cef_needed(&Modes::parse("", "all"), &views(), mask, |s| screens.contains(&s));
        assert_eq!(why.len(), 1);
        assert!(why[0].starts_with("shadow logic [btv,fws,pilot_seat,baro_unit,terrain_feed,datalink,fms]"), "{why:?}");
    }

    #[test]
    fn a_missing_xphfbw_means_native_all() {
        let dir = std::env::temp_dir().join(format!("native_mode_{}", std::process::id()));
        std::fs::create_dir_all(dir.join("XPHFBW")).unwrap();
        assert!(!cef_installed(Some(&dir)));
        std::fs::write(dir.join(XPHFBW_EXE), b"").unwrap();
        assert!(cef_installed(Some(&dir)));
        assert!(cef_installed(None));
        let _ = std::fs::remove_dir_all(&dir);
        if std::env::var(ENV).is_err() {
            assert!(!native_raw(false).trim().is_empty());
        }
    }

    #[test]
    fn each_failure_is_noticed_once() {
        assert!(failure_notice("test screen", "draw"));
        assert!(!failure_notice("test screen", "draw again"));
        let shown = take_notices();
        assert!(shown.iter().any(|t| t == &notice_text("test screen")));
        assert!(notice_text("FMS").contains("Log.txt"));
        assert!(take_notices().is_empty());
    }
}

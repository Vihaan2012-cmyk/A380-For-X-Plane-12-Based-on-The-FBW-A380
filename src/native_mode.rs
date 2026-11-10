//! Shadow or authoritative, per native system, decided here only.
//!
//! `FBW_NATIVE_SYSTEMS` (else `nativeSystems` in xphfbw.json) names the
//! authoritative systems; `all` takes each group's ready set. `FBW_NATIVE`
//! (else `native`) set to `all` makes every system authoritative and every
//! screen native; so does an X-Plane whose Chromium cannot run FlyByWire's pages.
//! `native_shadow.rs` and `native_systems_host.rs` read [`systems_spec`].

use std::collections::BTreeSet;
use std::sync::Mutex;

pub const ENV: &str = "FBW_NATIVE";
pub const SETTING: &str = "native";
pub const SYSTEMS_ENV: &str = "FBW_NATIVE_SYSTEMS";
pub const SYSTEMS_SETTING: &str = "nativeSystems";

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

/// FlyByWire's pages can run: X-Plane's Chromium is there for the browser screens.
fn pages_available() -> bool {
    #[cfg(feature = "js")]
    {
        crate::browser_screens::enabled()
    }
    #[cfg(not(feature = "js"))]
    {
        true
    }
}

pub fn work_wanted(modes: &Modes, comparisons_asked: bool) -> bool {
    comparisons_asked || (!modes.off && (modes.native_all || !modes.authoritative.is_empty()))
}

/// The native modules run each frame only when one drives something or `FBW_NATIVE_SHADOW=1`
/// asks for their comparisons against the pages.
pub fn any_native_work() -> bool {
    static WORK: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *WORK.get_or_init(|| work_wanted(&current(), std::env::var("FBW_NATIVE_SHADOW").is_ok_and(|v| v == "1")))
}

pub fn native_raw(pages_available: bool) -> String {
    let raw = env_or_setting(ENV, SETTING);
    if raw.trim().is_empty() && !pages_available {
        return "all".to_string();
    }
    raw
}

pub fn current() -> Modes {
    Modes::parse(&native_raw(pages_available()), &env_or_setting(SYSTEMS_ENV, SYSTEMS_SETTING))
}

pub fn systems_spec() -> String {
    current().spec()
}

pub fn native_all() -> bool {
    current().native_all
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
    #[test]
    fn native_work_runs_only_when_something_uses_it() {
        assert!(!super::work_wanted(&super::Modes::parse("", ""), false));
        assert!(super::work_wanted(&super::Modes::parse("", ""), true));
        assert!(super::work_wanted(&super::Modes::parse("all", ""), false));
        assert!(super::work_wanted(&super::Modes::parse("", "fms"), false));
        assert!(!super::work_wanted(&super::Modes::parse("", "off,fms"), false));
    }

    use super::*;

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
    fn no_pages_means_native_all() {
        if std::env::var(ENV).is_err() {
            assert_eq!(native_raw(false).trim(), "all");
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

//! The XPHFBW app's own settings ("Simulation (XPHFBW)" in
//! `docs/briefs/xphfbw-app.md`), read from `Output/preferences/xphfbw.json`
//! (`settings_files::app_settings_path`) and applied to the modules whose
//! behaviour they gate. See `docs/briefs/xphfbw-js-bridge.md`, "B — App
//! settings in the plugin".
//!
//! xphfbw.json is written by the app's settings window
//! (`app/src/settings.rs`) and read here: a background thread polls its
//! mtime every [`POLL_EVERY`] and reloads only when it changed, so a
//! setting the pilot changes in the app's window is picked up without
//! restarting X-Plane. [`current`] lazily starts that thread on first use
//! and returns the latest snapshot; it never blocks on disk I/O itself.
//!
//! What "live" means per key (the rest apply at the next aircraft load,
//! i.e. the next time the plugin's `Plugin::new` builds the owning struct
//! — this module cannot rebuild another module's state from inside a poll
//! thread):
//! - `randomFailures` / `failureRate`: live. `random_failures.rs`'s
//!   `RandomFailures::apply_requests` (called every tick already) resyncs
//!   from here whenever [`generation`] has moved since it last looked,
//!   unless the Study panel queued its own change that tick (that wins).
//! - `persistence`: live. `persistence.rs`'s `Persistence::save` checks it
//!   on every call (periodic autosave, exit save, and "reset airframe"),
//!   so turning it off stops further writes to
//!   `fbw_a380x_airframe.json` from that point on; the file already on
//!   disk is left untouched either way, and in-memory wear/damage/failure
//!   state keeps updating regardless (this setting is about the file, not
//!   the simulation).
//! - `stateDumps` / `stateDumpFrames` / `stateDumpKeep`: live.
//!   `state_dump.rs`'s `StateDump::tick`/`write` check every call.
//! - `coldStartGroundPower`: next aircraft load. `efb.rs`'s cold-start
//!   block runs exactly once, on the first `Efb::update` after
//!   `Efb::new` (i.e. once per aircraft load).
//! - `cabinVisible`: live. `cabin_option.rs`'s `CabinOption::update`
//!   pushes it into `fbw/options/cabin_visible` (the dataref the converted
//!   cabin objects' `ANIM_show` reads) every tick it changes.
//! - `walkaround`: live. Off (the default) removes every walkaround item
//!   and fits none at a cold and dark start (`walkaround.rs`).
//! - `systemsOutOfProcess`: next aircraft load. [`systems_out_of_process`]
//!   is read by `start_systems` (lib.rs, agent A), which only runs once,
//!   at `Plugin::new`.
//!
//! ## Rule 8 (the settings-files mutex)
//!
//! Every writer of xphfbw.json, the flyPad ini, or the NXDataStore JSON
//! holds the named mutex `Local\XPHFBW_settings_files` around its whole
//! read-modify-write, and writes to a temp file and renames
//! (`docs/briefs/xphfbw-js-bridge.md` race rule 8). [`with_settings_lock`]
//! is that shared entry point: `app/src/settings.rs::save` and this
//! plugin's flyPad-settings writer (`js_bridge.rs`'s `FlyPadSettings::set`)
//! both wrap their read-modify-write in it. This module only *reads*
//! xphfbw.json, and every writer renames into place atomically, so a
//! reader never needs the lock itself.

use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{OnceLock, RwLock};
use std::time::Duration;

#[path = "native_mode.rs"]
pub mod native_mode;

use serde_json::{Map, Value};

/// How often the background thread checks xphfbw.json's mtime.
const POLL_EVERY: Duration = Duration::from_secs(2);

/// The "Simulation (XPHFBW)" panel's settings, with FlyByWire/XPHFBW's
/// shipped defaults (must match `app/ui/index.html`'s `DEFAULTS`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SatcomAntenna {
    #[default]
    None,
    Gimballed,
    Victs,
}

#[derive(Clone, Default, PartialEq, Eq)]
pub struct MaskedText(String);

impl MaskedText {
    pub fn new(value: &str) -> Self {
        Self(value.trim().to_string())
    }

    pub fn reveal(&self) -> &str {
        &self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl std::fmt::Debug for MaskedText {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let n = self.0.chars().count();
        if n == 0 {
            f.write_str("\"\"")
        } else {
            write!(f, "\"{}\"", "*".repeat(n.min(8)))
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AppSettings {
    /// `xphfbw.ilsSource` is SCENERY: localizers come from X-Plane's own
    /// navaid files, scenery packs included, instead of the navigation data.
    pub ils_from_scenery: bool,
    /// The pilot's own aircraft identity (`xphfbw.registration`, `msn`,
    /// `selcal`); empty means the livery's, see [`effective`].
    pub registration: String,
    pub msn: String,
    pub selcal: String,
    /// `xphfbw.satcomAntenna`; `None` when the key is not in the file, so the
    /// flyPad's MODEL_SATCOM_ENABLED stays in charge.
    pub satcom_antenna: Option<SatcomAntenna>,
    pub systems_out_of_process: bool,
    pub cold_start_ground_power: bool,
    pub random_failures: bool,
    pub failure_rate: f64,
    pub persistence: bool,
    pub state_dumps: bool,
    pub state_dump_frames: u64,
    pub state_dump_keep: usize,
    /// The EFB's "Passenger cabin" switch (Settings > Aircraft).
    pub cabin_visible: bool,
    pub walkaround: bool,
    /// Head-up displays fitted: 0 none, 1 captain, 2 both (`xphfbw.hud`).
    pub hud_seats: u8,
    /// How hard the screens are sharpened as they are copied into X-Plane's
    /// devices, 0 (off) to 1 (display/gl.rs's `draw_bridge`).
    pub display_sharpen: f64,
    pub display_glow: f64,
    pub display_line_weight: f64,
    pub display_sheen: f64,
    pub roll_steering: bool,
    pub cabin_scene: u8,
    pub cabin_brightness: f64,
    pub display_scale: f64,
    pub screens_on_integrated_gpu: bool,
    pub ground_equipment: bool,
    pub gse_operator: u8,
    pub glass_scale: f64,
    pub cold_gate_start: bool,
    pub weight_variant: u8,
    pub engine_rating: u8,
    pub replay_minutes: f64,
    pub derates_enabled: bool,
    pub takeoff_derates: [f64; 5],
    pub climb_derates: [f64; 5],
    pub copilot_callouts: bool,
    pub copilot_gear_up: bool,
    pub copilot_voice: u8,
    pub copilot_sop: bool,
    pub copilot_after_landing: bool,
    pub copilot_after_start: bool,
    pub copilot_auto_checklists: bool,
    pub binaural_audio: bool,
    pub std_pax_weight_kg: f64,
    pub std_bag_weight_kg: f64,
    pub hoppie_logon: MaskedText,
    pub vatsim_token: MaskedText,
    pub ivao_token: MaskedText,
    pub sayintentions_key: MaskedText,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            ils_from_scenery: false,
            registration: String::new(),
            msn: String::new(),
            selcal: String::new(),
            satcom_antenna: None,
            systems_out_of_process: true,
            cold_start_ground_power: true,
            random_failures: false,
            failure_rate: 1.0,
            persistence: true,
            state_dumps: false,
            state_dump_frames: crate::state_dump::DEFAULT_EVERY_TICKS,
            state_dump_keep: crate::state_dump::DEFAULT_KEEP_PER_SESSION,
            cabin_visible: true,
            walkaround: false,
            hud_seats: 0,
            display_sharpen: 0.6,
            display_glow: 0.,
            display_line_weight: 0.,
            display_sheen: 0.5,
            roll_steering: false,
            cabin_scene: 0,
            cabin_brightness: 1.0,
            display_scale: crate::xphfbw_bridge_views::SCREEN_SUPERSAMPLE,
            screens_on_integrated_gpu: false,
            ground_equipment: true,
            gse_operator: 0,
            glass_scale: crate::xphfbw_bridge_views::GLASS_SCALE_DEFAULT,
            cold_gate_start: true,
            weight_variant: crate::aircraft_options::DEFAULT_WEIGHT_VARIANT,
            engine_rating: crate::aircraft_options::DEFAULT_ENGINE_RATING,
            replay_minutes: 30.0,
            derates_enabled: true,
            takeoff_derates: [4.0, 8.0, 12.0, 16.0, 20.0],
            climb_derates: [3.0, 6.0, 9.0, 12.0, 15.0],
            copilot_callouts: true,
            copilot_gear_up: false,
            copilot_voice: 0,
            copilot_sop: true,
            copilot_after_landing: false,
            copilot_after_start: false,
            copilot_auto_checklists: false,
            binaural_audio: true,
            std_pax_weight_kg: 84.0,
            std_bag_weight_kg: 20.0,
            hoppie_logon: MaskedText::default(),
            vatsim_token: MaskedText::default(),
            ivao_token: MaskedText::default(),
            sayintentions_key: MaskedText::default(),
        }
    }
}

fn as_bool(map: &Map<String, Value>, key: &str, default: bool) -> bool {
    match map.get(key) {
        Some(Value::Bool(b)) => *b,
        Some(Value::String(s)) => s == "true",
        _ => default,
    }
}

fn as_f64(map: &Map<String, Value>, key: &str, default: f64) -> f64 {
    match map.get(key) {
        Some(Value::Number(n)) => n.as_f64().unwrap_or(default),
        Some(Value::String(s)) => s.trim().parse().unwrap_or(default),
        _ => default,
    }
}

/// Parses the keys this module owns out of xphfbw.json's map (unprefixed:
/// `app/src/settings.rs::load` already strips `xphfbw.` before writing the
/// file). Anything missing, mistyped, or out of the UI's own range
/// (`app/ui/index.html`) falls back to the default rather than producing a
/// setting that could panic a caller (a zero frame interval, for example).
fn parse(map: &Map<String, Value>) -> AppSettings {
    let defaults = AppSettings::default();
    let copilot_mode = as_bool(map, "copilotMode", false);
    AppSettings {
        ils_from_scenery: as_text(map, "ilsSource").eq_ignore_ascii_case("SCENERY"),
        registration: as_text(map, "registration").to_ascii_uppercase(),
        msn: as_text(map, "msn"),
        selcal: as_text(map, "selcal").to_ascii_uppercase(),
        satcom_antenna: satcom_antenna(map),
        systems_out_of_process: as_bool(map, "systemsOutOfProcess", defaults.systems_out_of_process),
        cold_start_ground_power: as_bool(map, "coldStartGroundPower", defaults.cold_start_ground_power),
        random_failures: as_bool(map, "randomFailures", defaults.random_failures),
        failure_rate: as_f64(map, "failureRate", defaults.failure_rate).clamp(0.0, 1_000_000.0),
        persistence: as_bool(map, "persistence", defaults.persistence),
        state_dumps: as_bool(map, "stateDumps", defaults.state_dumps),
        state_dump_frames: (as_f64(map, "stateDumpFrames", defaults.state_dump_frames as f64) as u64).clamp(1, 1_000_000),
        state_dump_keep: (as_f64(map, "stateDumpKeep", defaults.state_dump_keep as f64) as u64).clamp(1, 1_000_000) as usize,
        cabin_visible: as_bool(map, "cabinVisible", defaults.cabin_visible),
        walkaround: as_bool(map, "walkaround", defaults.walkaround),
        hud_seats: defaults.hud_seats,
        display_sharpen: as_f64(map, "displaySharpen", defaults.display_sharpen).clamp(0.0, 1.0),
        display_glow: as_f64(map, "displayGlow", defaults.display_glow).clamp(0.0, 1.0),
        display_line_weight: as_f64(map, "displayLineWeight", defaults.display_line_weight).clamp(0.0, 1.0),
        display_sheen: as_f64(map, "displaySheen", defaults.display_sheen).clamp(0.0, 1.0),
        roll_steering: as_bool(map, "rollSteering", defaults.roll_steering),
        cabin_scene: as_f64(map, "cabinScene", defaults.cabin_scene as f64).clamp(0.0, 11.0) as u8,
        cabin_brightness: as_f64(map, "cabinBrightness", defaults.cabin_brightness).clamp(0.0, 1.0),
        display_scale: as_f64(map, "displayScale", defaults.display_scale).clamp(1.0, 2.0),
        screens_on_integrated_gpu: map.get("displayGpu").and_then(Value::as_str) == Some("integrated"),
        ground_equipment: as_bool(map, "groundEquipment", defaults.ground_equipment),
        gse_operator: gse_operator(map),
        glass_scale: as_f64(map, "glassScale", defaults.glass_scale).clamp(1.0, 2.0),
        cold_gate_start: as_bool(map, "coldGateStart", defaults.cold_gate_start),
        weight_variant: map
            .get("weightVariant")
            .and_then(Value::as_str)
            .and_then(crate::aircraft_options::weight_variant_index)
            .unwrap_or(defaults.weight_variant),
        engine_rating: crate::aircraft_options::rating_for_type(
            map.get("engineType").and_then(Value::as_str),
            map.get("engineRating").and_then(Value::as_str).and_then(crate::aircraft_options::engine_rating_index).unwrap_or(defaults.engine_rating),
        ),
        replay_minutes: as_f64(map, "replayMinutes", defaults.replay_minutes).clamp(1.0, 1440.0),
        derates_enabled: as_bool(map, "deratesEnabled", defaults.derates_enabled),
        takeoff_derates: derate_levels(map, "takeoffDerates", defaults.takeoff_derates),
        climb_derates: derate_levels(map, "climbDerates", defaults.climb_derates),
        copilot_callouts: copilot_mode || as_bool(map, "copilotCallouts", defaults.copilot_callouts),
        copilot_gear_up: copilot_mode || as_bool(map, "copilotGearUp", defaults.copilot_gear_up),
        copilot_voice: map
            .get("copilotVoice")
            .and_then(Value::as_str)
            .and_then(|v| crate::copilot::VOICES.iter().position(|n| *n == v))
            .map_or(defaults.copilot_voice, |i| i as u8),
        copilot_sop: copilot_mode || as_bool(map, "copilotSop", defaults.copilot_sop),
        copilot_after_landing: copilot_mode || as_bool(map, "copilotAfterLanding", defaults.copilot_after_landing),
        copilot_after_start: copilot_mode || as_bool(map, "copilotAfterStart", defaults.copilot_after_start),
        copilot_auto_checklists: copilot_mode || as_bool(map, "copilotAutoChecklists", defaults.copilot_auto_checklists),
        binaural_audio: as_bool(map, "binauralAudio", defaults.binaural_audio),
        std_pax_weight_kg: as_f64(map, "stdPaxWeightKg", defaults.std_pax_weight_kg).clamp(10.0, 250.0),
        std_bag_weight_kg: as_f64(map, "stdBagWeightKg", defaults.std_bag_weight_kg).clamp(1.0, 250.0),
        hoppie_logon: MaskedText::new(&as_text(map, "hoppieLogon")),
        vatsim_token: MaskedText::new(&as_text(map, "vatsimToken")),
        ivao_token: MaskedText::new(&as_text(map, "ivaoToken")),
        sayintentions_key: MaskedText::new(&as_text(map, "sayIntentionsKey")),
    }
}

fn as_text(map: &Map<String, Value>, key: &str) -> String {
    match map.get(key) {
        Some(Value::String(s)) => s.trim().to_string(),
        Some(Value::Number(n)) => n.to_string(),
        _ => String::new(),
    }
}

fn satcom_antenna(map: &Map<String, Value>) -> Option<SatcomAntenna> {
    match as_text(map, "satcomAntenna").to_ascii_uppercase().as_str() {
        "NONE" => Some(SatcomAntenna::None),
        "GIMBALLED" => Some(SatcomAntenna::Gimballed),
        "VICTS" => Some(SatcomAntenna::Victs),
        _ => None,
    }
}

impl AppSettings {
    /// Whether MODEL_SATCOM_ENABLED is on: the antenna choice when the app
    /// has one, else the flyPad's own value.
    pub fn satcom_installed(&self, flypad: bool) -> bool {
        self.satcom_antenna.map_or(flypad, |a| a != SatcomAntenna::None)
    }
}

/// The pilot's text over the livery's: the first non-empty of the two.
pub fn effective(user: &str, livery: &str) -> String {
    let user = user.trim();
    if user.is_empty() { livery.trim().to_string() } else { user.to_string() }
}

fn derate_levels(map: &Map<String, Value>, key: &str, default: [f64; 5]) -> [f64; 5] {
    let Some(text) = map.get(key).and_then(Value::as_str) else { return default };
    let mut out = default;
    for (k, part) in text.split(',').take(5).enumerate() {
        if let Ok(v) = part.trim().trim_end_matches('%').parse::<f64>() {
            out[k] = v.clamp(0.0, 40.0);
        }
    }
    out
}

fn gse_operator(map: &Map<String, Value>) -> u8 {
    match map.get("gseOperator") {
        Some(Value::String(s)) => match s.trim().to_ascii_lowercase().as_str() {
            "dnata" => 1,
            "swissport" => 2,
            "menzies" => 3,
            other => other.parse::<u8>().ok().filter(|n| *n <= 3).unwrap_or(0),
        },
        Some(Value::Number(n)) => n.as_u64().filter(|n| *n <= 3).unwrap_or(0) as u8,
        _ => 0,
    }
}

/// Reads and parses xphfbw.json at `path`; the shipped defaults if it does
/// not exist or does not parse (a fresh install, or a write caught
/// mid-rename — the writer renames into place, so a reader only ever sees
/// either the old file or the new one, never a partial one, but a reader
/// that races the very first `create_dir_all` can still see "missing").
fn load_from_path(path: &Path) -> AppSettings {
    let map = std::fs::read_to_string(path)
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default();
    parse(&map)
}

/// The live settings plus a generation counter, reusable in isolation
/// (unit-tested on its own instance, never the process-wide [`GLOBAL`],
/// so a test here can never race another module's test over shared
/// mutable state).
struct Store {
    settings: RwLock<AppSettings>,
    generation: AtomicU64,
}

impl Store {
    fn new(initial: AppSettings) -> Self {
        Self { settings: RwLock::new(initial), generation: AtomicU64::new(0) }
    }

    fn get(&self) -> AppSettings {
        self.settings.read().unwrap_or_else(std::sync::PoisonError::into_inner).clone()
    }

    /// Replaces the settings and bumps the generation, even if `settings`
    /// is unchanged (called only when xphfbw.json's mtime moved, so every
    /// call here is a real reload worth telling pollers about).
    fn set(&self, settings: AppSettings) {
        *self.settings.write().unwrap_or_else(std::sync::PoisonError::into_inner) = settings;
        self.generation.fetch_add(1, Ordering::SeqCst);
    }

    fn generation(&self) -> u64 {
        self.generation.load(Ordering::SeqCst)
    }
}

static GLOBAL: OnceLock<Store> = OnceLock::new();
static WATCHER_STARTED: OnceLock<()> = OnceLock::new();

fn global() -> &'static Store {
    GLOBAL.get_or_init(|| Store::new(AppSettings::default()))
}

/// Starts the background poller the first time anything asks for the
/// current settings; a no-op every time after. Outside X-Plane (unit
/// tests, `cargo test`), `crate::xp::system_path` returns `None` forever,
/// so the thread just idles and every caller keeps the shipped defaults —
/// nothing here touches the filesystem in that case.
fn ensure_watcher_started() {
    WATCHER_STARTED.get_or_init(|| {
        // Do the first load synchronously so the very first tick already
        // has whatever xphfbw.json says, rather than one poll interval of
        // defaults.
        reload_if_available();
        let spawned = std::thread::Builder::new().name("fbw app settings".into()).spawn(|| loop {
            std::thread::sleep(POLL_EVERY);
            reload_if_available();
        });
        if let Err(e) = spawned {
            crate::log(&format!("app settings: could not start the watcher thread ({e}); xphfbw.json changes need an aircraft reload"));
        }
    });
}

/// Reloads from xphfbw.json if its mtime moved since the last successful
/// reload (or this is the first check); a no-op otherwise, and a no-op if
/// X-Plane's folder is not known yet.
fn reload_if_available() {
    static LAST_MTIME: OnceLock<std::sync::Mutex<Option<std::time::SystemTime>>> = OnceLock::new();
    let Some(root) = crate::xp::system_path() else { return };
    let path = crate::settings_files::app_settings_path(&root);
    let modified = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
    let cell = LAST_MTIME.get_or_init(|| std::sync::Mutex::new(None));
    let mut last = cell.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if *last == modified && last.is_some() {
        return;
    }
    *last = modified;
    drop(last);
    global().set(load_from_path(&path));
}

/// The current settings; starts the watcher on first call (see
/// [`ensure_watcher_started`]). Cheap — a clone of a small struct
/// under a read lock — so every tick can call this.
pub fn load_now() -> AppSettings {
    crate::xp::system_path().map(|root| load_from_path(&crate::settings_files::app_settings_path(&root))).unwrap_or_default()
}

pub fn native_screens_raw() -> String {
    if native_mode::native_all() {
        return "all".to_string();
    }
    crate::xp::system_path()
        .map(|root| crate::settings_files::app_settings_path(&root))
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|v| v.get("nativeScreens").and_then(Value::as_str).map(str::to_string))
        .unwrap_or_default()
}

pub fn native_inline() -> bool {
    crate::xp::system_path()
        .map(|root| crate::settings_files::app_settings_path(&root))
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|v| v.get("nativeInline").and_then(Value::as_bool))
        .unwrap_or(false)
}

pub fn glass_selection_raw() -> String {
    crate::xp::system_path()
        .map(|root| crate::settings_files::app_settings_path(&root))
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|v| v.get("glass").and_then(Value::as_str).map(str::to_string))
        .unwrap_or_default()
}

pub fn current() -> AppSettings {
    ensure_watcher_started();
    global().get()
}

/// Bumps every time xphfbw.json is (re)loaded, including the first load.
/// `random_failures.rs` uses this to notice a change without re-reading
/// the file itself every tick.
pub fn generation() -> u64 {
    ensure_watcher_started();
    global().generation()
}

/// `xphfbw.systemsOutOfProcess`, for `start_systems` (lib.rs, agent A) to
/// decide whether to launch `fbw_a380_systems_server.exe`/XPHFBW.exe
/// separately or run the systems in-process.
pub fn systems_out_of_process() -> bool {
    current().systems_out_of_process
}

// ---------------------------------------------------------------------
// Rule 8: the cross-process settings-files mutex.
// ---------------------------------------------------------------------

/// The name rule 8 (`docs/briefs/xphfbw-js-bridge.md`) specifies for the
/// datastore JSON/flyPad ini/xphfbw.json read-modify-write lock.
/// `persistence.rs`'s own `file_mutex()` follows the same pattern for
/// `fbw_a380x_airframe.json`, under its own name.
const SETTINGS_FILES_MUTEX: &str = r"Local\XPHFBW_settings_files";

/// Holds the rule-8 settings-files mutex for the duration of `f`, released
/// even if `f` panics (`xphfbw_bridge::NamedMutex::with` itself does not
/// catch a panic in its closure, so this wraps `f` in `catch_unwind`
/// first: that inner closure never panics, so `NamedMutex::with`'s own
/// `ReleaseMutex` always runs before this function re-raises the panic).
/// If the OS mutex cannot even be created (should not happen on Windows),
/// runs `f` unguarded rather than losing the write entirely — no worse
/// than before this rule existed.
pub fn with_settings_lock<R>(f: impl FnOnce() -> R) -> R {
    let Some(mutex) = crate::xphfbw_bridge::NamedMutex::create(SETTINGS_FILES_MUTEX) else {
        return f();
    };
    match mutex.with(|| std::panic::catch_unwind(std::panic::AssertUnwindSafe(f))) {
        Ok(r) => r,
        Err(payload) => std::panic::resume_unwind(payload),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(pairs: &[(&str, Value)]) -> Map<String, Value> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.clone())).collect()
    }

    #[test]
    fn credentials_are_read_and_never_logged_in_clear() {
        let s = parse(&map(&[("hoppieLogon", Value::from(" abc123 ")), ("vatsimToken", Value::from("tok-vatsim")), ("sayIntentionsKey", Value::from("sai-key"))]));
        assert_eq!((s.hoppie_logon.reveal(), s.vatsim_token.reveal(), s.sayintentions_key.reveal()), ("abc123", "tok-vatsim", "sai-key"));
        assert!(s.ivao_token.is_empty());
        let logged = format!("{s:?}");
        for secret in ["abc123", "tok-vatsim", "sai-key"] {
            assert!(!logged.contains(secret), "{secret} leaked");
        }
        assert!(logged.contains("hoppie_logon: \"******\""));
    }

    #[test]
    fn defaults_when_the_key_is_missing() {
        let s = parse(&Map::new());
        assert_eq!(s, AppSettings::default());
    }

    #[test]
    fn copilot_mode_turns_on_everything_the_first_officer_does() {
        let mut map = Map::new();
        map.insert("copilotMode".into(), Value::Bool(true));
        map.insert("copilotCallouts".into(), Value::Bool(false));
        let s = parse(&map);
        assert!(s.copilot_callouts && s.copilot_gear_up && s.copilot_sop && s.copilot_after_start && s.copilot_after_landing && s.copilot_auto_checklists);
        let off = parse(&Map::new());
        assert!(!off.copilot_gear_up && !off.copilot_after_start && !off.copilot_auto_checklists);
    }

    #[test]
    fn parses_every_key_the_app_writes() {
        let m = map(&[
            ("systemsOutOfProcess", Value::Bool(false)),
            ("coldStartGroundPower", Value::Bool(false)),
            ("randomFailures", Value::Bool(true)),
            ("failureRate", Value::String("2.5".into())),
            ("persistence", Value::Bool(false)),
            ("stateDumps", Value::Bool(false)),
            ("stateDumpFrames", Value::String("50".into())),
            ("stateDumpKeep", Value::String("10".into())),
        ]);
        let s = parse(&m);
        assert!(!s.systems_out_of_process);
        assert!(!s.cold_start_ground_power);
        assert!(s.random_failures);
        assert_eq!(s.failure_rate, 2.5);
        assert!(!s.persistence);
        assert!(!s.state_dumps);
        assert_eq!(s.state_dump_frames, 50);
        assert_eq!(s.state_dump_keep, 10);
    }

    /// The EFB's "Passenger cabin" switch: shown unless the pilot turns it
    /// off, and a string from an older writer reads the same as a bool.
    #[test]
    fn the_cabin_is_shown_unless_switched_off() {
        assert!(parse(&Map::new()).cabin_visible);
        assert!(!parse(&map(&[("cabinVisible", Value::Bool(false))])).cabin_visible);
        assert!(!parse(&map(&[("cabinVisible", Value::String("false".into()))])).cabin_visible);
        assert!(parse(&map(&[("cabinVisible", Value::Bool(true))])).cabin_visible);
    }

    #[test]
    fn the_first_officer_settings_read_by_name() {
        let d = parse(&Map::new());
        assert!(d.copilot_callouts && !d.copilot_gear_up && d.copilot_voice == 0);
        let s = parse(&map(&[("copilotVoice", Value::String("cori".into())), ("copilotGearUp", Value::Bool(true)), ("copilotCallouts", Value::Bool(false))]));
        assert!(!s.copilot_callouts && s.copilot_gear_up);
        assert_eq!(s.copilot_voice, 2);
        assert_eq!(parse(&map(&[("copilotVoice", Value::String("nobody".into()))])).copilot_voice, 0);
        assert!(d.copilot_sop);
        let w = parse(&map(&[("stdPaxWeightKg", Value::String("95".into())), ("stdBagWeightKg", Value::String("0".into()))]));
        assert_eq!((w.std_pax_weight_kg, w.std_bag_weight_kg), (95.0, 1.0));
    }

    #[test]
    fn ils_source_is_navdata_unless_it_says_scenery() {
        assert!(!parse(&Map::new()).ils_from_scenery);
        assert!(!parse(&map(&[("ilsSource", Value::String("NAVDATA".into()))])).ils_from_scenery);
        assert!(parse(&map(&[("ilsSource", Value::String("SCENERY".into()))])).ils_from_scenery);
        assert!(parse(&map(&[("ilsSource", Value::String(" scenery ".into()))])).ils_from_scenery);
    }

    #[test]
    fn identity_text_is_trimmed_and_registration_and_selcal_are_upper_case() {
        let s = parse(&map(&[
            ("registration", Value::String(" a6-edy ".into())),
            ("msn", Value::String(" 0123 ".into())),
            ("selcal", Value::String("ab-cd".into())),
        ]));
        assert_eq!((s.registration.as_str(), s.msn.as_str(), s.selcal.as_str()), ("A6-EDY", "0123", "AB-CD"));
        let empty = parse(&Map::new());
        assert!(empty.registration.is_empty() && empty.msn.is_empty() && empty.selcal.is_empty());
    }

    #[test]
    fn the_pilots_value_wins_over_the_liverys_and_empty_falls_back() {
        assert_eq!(effective("G-XLEF", "A6-EDY"), "G-XLEF");
        assert_eq!(effective("  ", "A6-EDY"), "A6-EDY");
        assert_eq!(effective("", ""), "");
    }

    #[test]
    fn the_satcom_antenna_overrides_the_flypad_only_when_the_app_chose_one() {
        let unset = parse(&Map::new());
        assert!(unset.satcom_installed(true) && !unset.satcom_installed(false));
        let pick = |v: &str| parse(&map(&[("satcomAntenna", Value::String(v.into()))]));
        assert!(!pick("NONE").satcom_installed(true));
        assert!(pick("GIMBALLED").satcom_installed(false));
        assert!(pick("VICTS").satcom_installed(false));
        assert_eq!(pick("bogus").satcom_antenna, None);
    }

    #[test]
    fn a_zero_frame_interval_falls_back_to_one_rather_than_panicking_a_modulo() {
        let m = map(&[("stateDumpFrames", Value::String("0".into()))]);
        assert_eq!(parse(&m).state_dump_frames, 1);
    }

    #[test]
    fn an_unparsable_number_falls_back_to_the_default() {
        let m = map(&[("failureRate", Value::String("not a number".into()))]);
        assert_eq!(parse(&m).failure_rate, AppSettings::default().failure_rate);
    }

    #[test]
    fn a_missing_file_reads_as_the_shipped_defaults() {
        let path = std::env::temp_dir().join(format!("xphfbw-app-settings-missing-{}.json", std::process::id()));
        let _ = std::fs::remove_file(&path);
        assert_eq!(load_from_path(&path), AppSettings::default());
    }

    #[test]
    fn round_trips_through_a_real_file() {
        let path = std::env::temp_dir().join(format!("xphfbw-app-settings-{}.json", std::process::id()));
        std::fs::write(&path, r#"{"randomFailures": true, "failureRate": "3", "stateDumpFrames": "75"}"#).unwrap();
        let s = load_from_path(&path);
        assert!(s.random_failures);
        assert_eq!(s.failure_rate, 3.0);
        assert_eq!(s.state_dump_frames, 75);
        // Untouched keys keep their default.
        assert!(s.persistence);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn a_corrupt_file_reads_as_the_shipped_defaults_instead_of_failing() {
        let path = std::env::temp_dir().join(format!("xphfbw-app-settings-corrupt-{}.json", std::process::id()));
        std::fs::write(&path, b"{ not json").unwrap();
        assert_eq!(load_from_path(&path), AppSettings::default());
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn a_local_store_tracks_generation_and_the_latest_value() {
        let store = Store::new(AppSettings::default());
        assert_eq!(store.generation(), 0);
        let mut changed = AppSettings::default();
        changed.random_failures = true;
        store.set(changed.clone());
        assert_eq!(store.generation(), 1);
        assert!(store.get().random_failures);
        store.set(changed); // Same value, still a real reload: still bumps.
        assert_eq!(store.generation(), 2);
    }

    #[test]
    fn the_settings_lock_runs_the_closure_and_returns_its_value() {
        assert_eq!(with_settings_lock(|| 42), 42);
    }

    #[test]
    fn the_settings_lock_can_be_taken_again_after_a_previous_call_released_it() {
        with_settings_lock(|| {});
        with_settings_lock(|| {});
    }

    /// A real mutual-exclusion test: many threads each do a
    /// load-yield-store (not an atomic RMW) on a shared counter inside the
    /// lock. If `with_settings_lock` failed to exclude, interleaved
    /// load/store pairs would lose increments and the final count would
    /// come out low; with correct exclusion it always matches exactly.
    #[test]
    fn the_settings_lock_serialises_concurrent_writers() {
        use std::sync::atomic::AtomicI64;
        use std::sync::Arc;
        let counter = Arc::new(AtomicI64::new(0));
        let handles: Vec<_> = (0..8)
            .map(|_| {
                let counter = counter.clone();
                std::thread::spawn(move || {
                    for _ in 0..500 {
                        with_settings_lock(|| {
                            let v = counter.load(Ordering::Relaxed);
                            std::thread::yield_now();
                            counter.store(v + 1, Ordering::Relaxed);
                        });
                    }
                })
            })
            .collect();
        for h in handles {
            h.join().unwrap();
        }
        assert_eq!(counter.load(Ordering::Relaxed), 8 * 500);
    }

    #[test]
    fn a_panic_inside_the_lock_still_releases_it() {
        let result = std::panic::catch_unwind(|| {
            with_settings_lock(|| panic!("boom"));
        });
        assert!(result.is_err());
        // If the mutex were left held, this would deadlock the test run
        // (WaitForSingleObject with INFINITE); reaching this line proves
        // it was released.
        with_settings_lock(|| {});
    }
}

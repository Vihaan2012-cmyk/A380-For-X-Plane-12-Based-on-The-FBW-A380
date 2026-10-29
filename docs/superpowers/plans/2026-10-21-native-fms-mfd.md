# Native FMS and MFD Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** move the A380's FMS into Rust (`crates/fms` in `fbw_a380_systems`) and draw the MFD natively with a separate C++ plugin, `a380_displays`. This plan is the roadmap for all eight phases, with Phase 1 (the oracle) in full detail.

**Architecture:** FlyByWire's FMS runs offline in the plugin's QuickJS harness and is recorded as JSON goldens. The Rust FMS is built against those goldens, phase by phase. The C++ plugin reads a JSON model from a data dataref and writes actions back.

**Tech stack:**
- Rust 2021: the plugin and the new `crates/fms`.
- QuickJS (rquickjs, feature `js`) for the oracle.
- C++17 with CMake, the X-Plane SDK, NanoVG (GL3), stb_truetype and nlohmann/json for the displays plugin.

**Spec:** `docs/superpowers/specs/2026-10-21-native-fms-mfd-design.md`

## Global Constraints

- **Builds:**
  - Cargo uses only `CARGO_TARGET_DIR=E:/fbw-build/target`.
  - Build the instruments from the fork with `XP_UI=E:/fbw-int/clean-avionics/lib` and `XP_KIT` unset.
- **Never open or read:**
  - E:/fbw-int/xp-avionics, E:/fbw-int/xp-kit, E:/fbw-int/adapt-orig;
  - any node_modules/@microsoft;
  - E:/fbw-build/golden, E:/fbw-build/kit-new, E:/fbw-build/html-ui-xpkit.
- **Do not translate:** framework utilities used by the FMS (EventBus, Subject, MathUtils, AeroMath, UnitType, BitFlags, UUID, Wait). Rust gets its own. Also do not translate `utils/LzUtf8.js` or `wtsdk.ts`.
- **Comments:** no explanatory comment paragraphs in code.
- **Commits:** commit as `git -c user.name=vihaan2012-cmyk -c user.email=vihaan2012-cmyk@users.noreply.github.com`, with no co-author line. The user pushes.
- **Edits:** use the Edit/Write tools, not Bash heredocs, for files containing backslashes.
- **X-Plane:** never launch it without asking the user that time.
- **Agents:** none unless the user allows them for this job.

---

## Phase overview

| Phase | Deliverable | Done when |
|---|---|---|
| 1 Oracle | `src/js/msfs/fms_oracle.rs`, `tools/fms_oracle/`, goldens in `E:/fbw-build/fms-oracle/` | 4 scenarios recorded twice with identical output |
| 2 Plan + uplink | `crates/fms` with `plan`, `uplink`, `NavDatabase` trait, comparator | Phase 1 scenarios match on every plan field |
| 3 LNAV | `lnav` geometry, transitions, path vectors, sequencing, guidance laws; `fly` scenario op | EFIS vector events and guidance vars match along 3 simulated flights |
| 4 VNAV + rest | `vnav`, `perf`, `fuel`, `messages`, `efis` symbols, `phase` | every recorded var and generic-data event matches in all scenarios |
| 5 Live | `src/fms_live.rs`: shadow mode (`fms shadow:` log), then authoritative when `a380_displays` claims the MFD | one user flight with zero shadow differences |
| 6 C++ foundation | `E:/fbw-int/a380-displays`: device claim, NanoVG, fonts, input, model/action datarefs, headless PNG renderer | a test page drawn in the sim (user launches); under 0.3 ms per frame |
| 7 MFD pages | every page the current MFD has, drawn from the model | each page's PNG matches its Chromium capture; user check in the sim |
| 8 Cut-over | release ships native FMS + MFD; the CEF MFD page leaves the install | release built from the exact install |

Phases 2 to 8 each get their own detailed plan when they start, written against the goldens Phase 1 produces.

---

## Phase 1: Oracle

### File map

- Modify `src/js/mod.rs`: the `Host` trait gains `generic_data` and `set_now`.
- Modify `src/js/msfs/mod.rs`: `ViewHost` passes GENERIC_DATA to its outer host.
- Modify `src/js/msfs/fms_offline.rs`: `FmsHost` forwards `generic_data`, and sets its inner host's time each step.
- Create `src/js/msfs/fms_oracle.rs`: the `Recording` host, scenario parsing, the runner and golden writing.
- Create `src/js/msfs/fms_oracle_snapshot.js`: serialises FlyByWire's FMS state inside the MFD view.
- Modify `src/js/msfs/mod.rs`: `mod fms_oracle;` behind `#[cfg(test)]`, after `fms_offline`.
- Create `tools/fms_oracle/make_pkg.py`: builds the MFD-only oracle package.
- Create `tools/fms_oracle/test_make_pkg.py`.
- Create `tools/fms_oracle/scenarios/*.json`: four scenarios.
- Fork `E:/fbw-int/fbw-aircraft-ts`: `FlightManagementComputer.ts` exposes `globalThis.__xpFmc`.

### Task 1: Recording host

**Files:**
- Modify: `src/js/mod.rs` (`pub trait Host`, after `fn trigger`)
- Modify: `src/js/msfs/mod.rs` (`impl Host for ViewHost`, the `"GENERIC_DATA"` arm of `fn trigger`)
- Modify: `src/js/msfs/fms_offline.rs` (`impl<H: Host> Host for FmsHost<H>`, `fn step`)
- Create: `src/js/msfs/fms_oracle.rs`

**Interfaces:**
- Produces:
  - `Host::generic_data(&mut self, key: &str, json: &str)` and `Host::set_now(&mut self, ms: f64)`, both defaulting to no-ops;
  - `pub struct Recording<H: Host> { pub inner: H, pub now_ms: f64, pub events: Vec<serde_json::Value> }`;
  - `Recording::new(inner: H) -> Self`.

- [ ] **Step 1: Write the failing test** at the end of the new `src/js/msfs/fms_oracle.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::js::msfs::fms_offline::VarStore;

    #[test]
    fn recording_keeps_changes_and_generic_data_in_order() {
        let mut r = Recording::new(VarStore::new());
        r.set_now(50.);
        r.set_var("L:A", "number", 1.);
        r.set_var("L:A", "number", 1.);
        r.generic_data("K", "[1]");
        r.set_now(100.);
        r.set_var("L:A", "number", 2.);
        assert_eq!(r.events.len(), 3);
        assert_eq!(r.events[0]["t"], 50.);
        assert_eq!(r.events[1]["key"], "K");
        assert_eq!(r.events[2]["value"], 2.);
        assert_eq!(r.inner.vars.get("L:A"), Some(&2.));
    }
}
```

At the top of the same file:

```rust
use std::collections::HashMap;

use serde_json::json;

use crate::js::{CallReply, Host, LogLevel};
```

In `src/js/msfs/mod.rs`, after the existing `#[cfg(test)] pub(crate) mod fms_offline;`, add:

```rust
#[cfg(test)]
mod fms_oracle;
```

- [ ] **Step 2: Run the test and confirm it fails**

Run: `CARGO_TARGET_DIR=E:/fbw-build/target cargo test --release --features js --lib fms_oracle::tests`
Expected: compile error, because `Recording` is not defined.

- [ ] **Step 3: Implement**

1. In `src/js/mod.rs`, inside `pub trait Host`, after `fn trigger(...) {}`:

```rust
    fn generic_data(&mut self, _key: &str, _json: &str) {}
    fn set_now(&mut self, _ms: f64) {}
```

2. In `src/js/msfs/mod.rs`, in `impl Host for ViewHost<'_>` → `fn trigger`, in the `"GENERIC_DATA" =>` arm, after `crate::tod_pause::on_generic_data(&s(0), &s(1));`:

```rust
                self.outer.generic_data(&s(0), &s(1));
```

3. In `src/js/msfs/fms_offline.rs`:
   - in `impl<H: Host> Host for FmsHost<H>`, add:

```rust
    fn generic_data(&mut self, key: &str, json: &str) {
        self.inner.generic_data(key, json)
    }
```

   - in `pub fn step`, change the body to:

```rust
        self.pump_nav(cockpit);
        let now = self.now_ms;
        self.inner.set_now(now);
        cockpit.tick(self, now);
        self.now_ms += STEP_MS;
```

4. In `src/js/msfs/fms_oracle.rs`, above the tests:

```rust
pub struct Recording<H: Host> {
    pub inner: H,
    pub now_ms: f64,
    pub events: Vec<serde_json::Value>,
    last: HashMap<String, u64>,
}

impl<H: Host> Recording<H> {
    pub fn new(inner: H) -> Self {
        Self { inner, now_ms: 0., events: Vec::new(), last: HashMap::new() }
    }
}

impl<H: Host> Host for Recording<H> {
    fn get_var(&mut self, name: &str, unit: &str) -> f64 {
        self.inner.get_var(name, unit)
    }

    fn set_var(&mut self, name: &str, unit: &str, value: f64) {
        if self.last.insert(name.to_string(), value.to_bits()) != Some(value.to_bits()) {
            self.events.push(json!({ "t": self.now_ms, "var": name, "value": value }));
        }
        self.inner.set_var(name, unit, value)
    }

    fn log(&mut self, level: LogLevel, message: &str) {
        self.inner.log(level, message)
    }

    fn get_string(&mut self, name: &str) -> String {
        self.inner.get_string(name)
    }

    fn set_string(&mut self, name: &str, value: &str) {
        self.inner.set_string(name, value)
    }

    fn call(&mut self, name: &str, args_json: &str) -> CallReply {
        self.inner.call(name, args_json)
    }

    fn trigger(&mut self, name: &str, args_json: &str) {
        self.inner.trigger(name, args_json)
    }

    fn send_event(&mut self, name: &str, values: &[f64]) {
        self.inner.send_event(name, values)
    }

    fn get_magvar(&mut self, lat: f64, lon: f64) -> f64 {
        self.inner.get_magvar(lat, lon)
    }

    fn generic_data(&mut self, key: &str, json: &str) {
        self.events.push(json!({ "t": self.now_ms, "key": key, "json": json }));
        self.inner.generic_data(key, json)
    }

    fn set_now(&mut self, ms: f64) {
        self.now_ms = ms;
        self.inner.set_now(ms)
    }
}
```

- [ ] **Step 4: Run the test and confirm it passes**

Run: `CARGO_TARGET_DIR=E:/fbw-build/target cargo test --release --features js --lib fms_oracle::tests`
Expected: PASS.

Then run `cargo test --release --features js --lib js::` (same target dir). Expected: no new failures against the count before the change. Run it once before Step 3 to get that count.

- [ ] **Step 5: Commit**

```bash
git add src/js/mod.rs src/js/msfs/mod.rs src/js/msfs/fms_offline.rs src/js/msfs/fms_oracle.rs
git commit -m "fms oracle: record the MFD's variable writes and generic-data events"
```

### Task 2: FMC handle in the fork

**Files:**
- Modify: `E:/fbw-int/fbw-aircraft-ts/fbw-a380x/src/systems/instruments/src/MFD/FMC/FlightManagementComputer.ts` (constructor, after `this.#mfdReference = mfdReference;`)

**Interfaces:**
- Produces: `globalThis.__xpFmc`, an array indexed by `FmcIndex` that holds each `FlightManagementComputer`.

- [ ] **Step 1: Add the handle**

```ts
    const xpFmcs = ((globalThis as any).__xpFmc ??= []);
    xpFmcs[instance] = this;
```

- [ ] **Step 2: Build the instruments**

From `E:/fbw-int/fbw-aircraft-ts`, with `XP_UI=E:/fbw-int/clean-avionics/lib` set and `XP_KIT` unset, run:

```
node E:/fbw-int/plugin/tools/js-build/fbw-mach.cjs build --config fbw-a380x/mach.config.js --work-in-config-dir
```

Expected: the build succeeds, and `grep -l __xpFmc fbw-a380x/out/flybywire-aircraft-a380-842/html_ui/Pages/VCockpit/Instruments/A380X/MFD/*.js` prints the MFD bundle.

- [ ] **Step 3: Commit in the fork**

```bash
git -C E:/fbw-int/fbw-aircraft-ts add fbw-a380x/src/systems/instruments/src/MFD/FMC/FlightManagementComputer.ts
git -C E:/fbw-int/fbw-aircraft-ts commit -m "mfd: expose the FMCs on globalThis.__xpFmc for the offline oracle"
```

### Task 3: Oracle package

**Files:**
- Create: `tools/fms_oracle/make_pkg.py`
- Create: `tools/fms_oracle/test_make_pkg.py`

**Interfaces:**
- Produces `E:/fbw-build/fms-oracle-pkg/` containing `html_ui/` (from the fork's build), a `panel.cfg` with only `[VCockpit01]` (the MFD), and `panel.xml`.
- The runner reads it from `FMS_ORACLE_PKG`.

- [ ] **Step 1: Write the failing test** in `tools/fms_oracle/test_make_pkg.py`:

```python
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from make_pkg import mfd_only

CFG = "[Window Titles]\nWindow00=X\n\n[VCockpit01]\ntexture=SCREEN_DU_MFD\nhtmlgauge00=A380X/MFD/mfd.html?duID=2, 0,0,1646,1024\n\n[VCockpit02]\ntexture=SCREEN_DU_PFDL\nhtmlgauge00=A380X/PFD/pfd.html\n"

out = mfd_only(CFG)
assert "[Window Titles]" in out
assert "SCREEN_DU_MFD" in out
assert "SCREEN_DU_PFDL" not in out
assert "[VCockpit02]" not in out
print("ok")
```

- [ ] **Step 2: Run it and confirm it fails**

Run: `python tools/fms_oracle/test_make_pkg.py`
Expected: `ModuleNotFoundError: No module named 'make_pkg'`.

- [ ] **Step 3: Implement** `tools/fms_oracle/make_pkg.py`:

```python
import shutil
import sys
from pathlib import Path

FORK = Path("E:/fbw-int/fbw-aircraft-ts")
HTML_UI = FORK / "fbw-a380x/out/flybywire-aircraft-a380-842/html_ui"
PANEL = FORK / "fbw-a380x/src/base/flybywire-aircraft-a380-842/SimObjects/AirPlanes/FlyByWire_A380X/attachments/flybywire/Part_Interior_Cockpit/panel"
DEFAULT_DEST = Path("E:/fbw-build/fms-oracle-pkg")


def mfd_only(cfg: str) -> str:
    out, keep = [], True
    for line in cfg.splitlines():
        s = line.strip()
        if s.startswith("[VCockpit"):
            keep = s == "[VCockpit01]"
        if keep:
            out.append(line)
    return "\n".join(out) + "\n"


def main(dest: Path, replace: bool):
    if dest.exists():
        if not replace:
            sys.exit(f"{dest} exists; pass --replace to rebuild it")
        shutil.rmtree(dest)
    shutil.copytree(HTML_UI, dest / "html_ui")
    (dest / "panel.cfg").write_text(mfd_only((PANEL / "panel.cfg").read_text(encoding="utf-8")), encoding="utf-8")
    shutil.copyfile(PANEL / "panel.xml", dest / "panel.xml")
    print(f"{dest}: html_ui + MFD-only panel.cfg")


if __name__ == "__main__":
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    main(Path(args[0]) if args else DEFAULT_DEST, "--replace" in sys.argv)
```

- [ ] **Step 4: Run the test, then build the package**

Run: `python tools/fms_oracle/test_make_pkg.py`
Expected: `ok`.

Run: `python tools/fms_oracle/make_pkg.py`
Expected: `E:\fbw-build\fms-oracle-pkg: html_ui + MFD-only panel.cfg`. The new `panel.cfg` contains `SCREEN_DU_MFD` and no other `texture=` line.

- [ ] **Step 5: Commit**

```bash
git add tools/fms_oracle/make_pkg.py tools/fms_oracle/test_make_pkg.py
git commit -m "fms oracle: MFD-only package from the xp-ui build"
```

### Task 4: Snapshot script

**Files:**
- Create: `src/js/msfs/fms_oracle_snapshot.js`

**Interfaces:**
- Consumes `globalThis.__xpFmc` (Task 2).
- Produces one JSON string with `{ plans: { active, temporary, secondary[] }, alternate, guidance, fmgc }`.
  - Each plan is `{ version, activeLegIndex, origin, destination, legs[], performance }`.
  - Each leg is `{ i, ident, type, effectiveType, flags, annotation, lat, lon, descriptor, constraintType, alt, spd, hold, calc, def }`, or `{ i, disco: true }`.
  - If there is no FMC, the result is `{ error }`.

- [ ] **Step 1: Write the script**

```js
(() => {
  const fmc = (globalThis.__xpFmc || []).find(Boolean);
  if (!fmc) return JSON.stringify({ error: 'no FMC handle' });
  const path = new Set();
  const plain = (v, depth) => {
    if (v === null || v === undefined) return null;
    const t = typeof v;
    if (t === 'number') return Number.isFinite(v) ? v : String(v);
    if (t === 'string' || t === 'boolean') return v;
    if (t !== 'object') return undefined;
    if (depth <= 0 || path.has(v)) return undefined;
    if (typeof v.get === 'function' && typeof v.sub === 'function') return plain(v.get(), depth);
    path.add(v);
    try {
      if (Array.isArray(v) || ArrayBuffer.isView(v)) return Array.from(v).slice(0, 400).map((x) => plain(x, depth - 1));
      if (v instanceof Map) return Object.fromEntries([...v.entries()].slice(0, 400).map(([k, x]) => [String(k), plain(x, depth - 1)]));
      const out = {};
      for (const k of Object.keys(v)) {
        if (k.startsWith('_') || k === 'bus' || k === 'subs' || k === 'flightPlan' || k === 'parent' || k === 'segment') continue;
        const x = plain(v[k], depth - 1);
        if (x !== undefined) out[k] = x;
      }
      return out;
    } finally {
      path.delete(v);
    }
  };
  const leg = (e, i) => {
    if (!e) return null;
    if (e.isDiscontinuity) return { i, disco: true };
    const d = e.definition || {};
    const loc = d.waypoint && d.waypoint.location;
    const c = e.calculated;
    return {
      i,
      ident: e.ident,
      type: e.type,
      effectiveType: e.effectiveType,
      flags: e.flags,
      annotation: e.annotation,
      lat: loc ? loc.lat : null,
      lon: loc ? loc.long : null,
      descriptor: e.waypointDescriptor,
      constraintType: e.constraintType,
      alt: plain(e.altitudeConstraint, 3),
      spd: plain(e.speedConstraint, 3),
      hold: plain(e.modifiedHold || e.defaultHold, 3),
      calc: c
        ? {
            distance: c.distance,
            cumulativeDistance: c.cumulativeDistance,
            cumulativeDistanceToEnd: c.cumulativeDistanceToEnd,
            distanceWithTransitions: c.distanceWithTransitions,
            trueTrack: c.trueTrack,
            endsInTooSteepPath: c.endsInTooSteepPath,
          }
        : null,
      def: plain(d, 3),
    };
  };
  const plan = (p) =>
    p
      ? {
          version: p.version,
          activeLegIndex: p.activeLegIndex,
          origin: p.originAirport ? p.originAirport.ident : null,
          destination: p.destinationAirport ? p.destinationAirport.ident : null,
          legs: p.allLegs.map(leg),
          performance: plain(p.performanceData, 2),
        }
      : null;
  const fps = fmc.flightPlanInterface;
  const secondary = [];
  for (let n = 1; n <= 3; n++) {
    if (fps.hasSecondary && fps.hasSecondary(n)) secondary.push(plan(fps.secondary(n)));
  }
  const active = fps.hasActive ? fps.active : null;
  return JSON.stringify({
    plans: { active: plan(active), temporary: fps.hasTemporary ? plan(fps.temporary) : null, secondary },
    alternate: active && active.alternateFlightPlan ? plan(active.alternateFlightPlan) : null,
    guidance: plain(fmc.guidanceController && fmc.guidanceController.vnavDriver, 3),
    fmgc: plain(fmc.fmgc && fmc.fmgc.data, 2),
  });
})()
```

- [ ] **Step 2: Commit** (Task 5 tests it, against the live MFD)

```bash
git add src/js/msfs/fms_oracle_snapshot.js
git commit -m "fms oracle: snapshot script for FlyByWire's flight plans and FMGC data"
```

### Task 5: Scenario runner and goldens

**Files:**
- Modify: `src/js/msfs/fms_oracle.rs`
- Create: `tools/fms_oracle/scenarios/uplink-egll-kjfk.json`

**Interfaces:**
- Consumes:
  - `Recording` (Task 1);
  - `fms_offline::{new_cockpit, attach_navdata, power_up_on_ground, load_route, canned_ofp_json, Mfd, VarStore, FmsHost}`;
  - the snapshot script (Task 4).
- Scenario JSON: `{ "name": str, "lat": f64, "lon": f64, "steps": [step] }`. Each step is one of:
  - `{"uplink": "canned"}`
  - `{"run_ms": n}`
  - `{"fms": "<async JS body; fmc and fps are in scope>"}`
  - `{"page": ["ACTIVE", "F-PLN"]}`
  - `{"var": ["L:NAME", "unit", value]}`
  - `{"snapshot": "label"}`
- Golden: `E:/fbw-build/fms-oracle/<name>.json` = `{ scenario, navdata, snapshots: [{ label, t, state, mfd }], events }`.
  - `state` is the parsed snapshot JSON, and `mfd` is `Mfd::positioned_texts`.
  - The navdata field is `nav.date_range()`.

- [ ] **Step 1: Write the scenario and the ignored test**

`tools/fms_oracle/scenarios/uplink-egll-kjfk.json`:

```json
{
  "name": "uplink-egll-kjfk",
  "lat": 51.4706,
  "lon": -0.4619,
  "steps": [
    { "uplink": "canned" },
    { "run_ms": 10000 },
    { "snapshot": "inserted" },
    { "page": ["ACTIVE", "F-PLN"] },
    { "run_ms": 3000 },
    { "snapshot": "fpln-page" }
  ]
}
```

In `fms_oracle.rs` tests:

```rust
    #[test]
    #[ignore]
    fn record_fms_oracle() {
        let dir = std::path::PathBuf::from(std::env::var("FMS_ORACLE_SCENARIOS").unwrap_or_else(|_| "tools/fms_oracle/scenarios".into()));
        let out = std::path::PathBuf::from(std::env::var("FMS_ORACLE_OUT").unwrap_or_else(|_| r"E:\fbw-build\fms-oracle".into()));
        let only = std::env::var("FMS_ORACLE_ONLY").ok();
        std::fs::create_dir_all(&out).unwrap();
        let mut names: Vec<_> = std::fs::read_dir(&dir).unwrap().filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "json")).collect();
        names.sort();
        for path in names {
            let scenario: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
            let name = scenario["name"].as_str().unwrap().to_string();
            if only.as_deref().is_some_and(|o| o != name) {
                continue;
            }
            let golden = run_scenario(&scenario).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert!(golden["events"].as_array().unwrap().iter().any(|e| e["key"].as_str().is_some_and(|k| k.starts_with("A32NX_EFIS_VECTORS_"))), "{name}: no EFIS vector events recorded");
            std::fs::write(out.join(format!("{name}.json")), serde_json::to_string_pretty(&golden).unwrap()).unwrap();
            println!("{name}: {} snapshots, {} events", golden["snapshots"].as_array().unwrap().len(), golden["events"].as_array().unwrap().len());
        }
    }
```

- [ ] **Step 2: Run it and confirm it fails**

Run: `CARGO_TARGET_DIR=E:/fbw-build/target cargo test --release --features js --lib fms_oracle -- --ignored --nocapture`
Expected: compile error, because `run_scenario` is not defined.

- [ ] **Step 3: Implement the runner** in `fms_oracle.rs`, above the tests:

```rust
use std::path::PathBuf;

use super::fms_offline::{attach_navdata, canned_ofp_json, load_route, new_cockpit, power_up_on_ground, FmsHost, Mfd, VarStore};
use super::Cockpit;

const SNAPSHOT_JS: &str = include_str!("fms_oracle_snapshot.js");
const STEP_MS: f64 = 50.;

type OracleHost = FmsHost<Recording<VarStore>>;

fn wait_ready(cockpit: &mut Cockpit, host: &mut OracleHost) -> Result<Mfd, String> {
    let mfd = Mfd::locate(cockpit)?;
    for _ in 0..120 {
        host.steps(cockpit, 20);
        if cockpit.all_loaded() && mfd.install_queries(cockpit).is_ok() && mfd.find(cockpit, "ACTIVE", true, ".mfd-page-selector-label").is_some() {
            return Ok(mfd);
        }
    }
    Err("the MFD never showed its page selectors".into())
}

fn fms_op(cockpit: &mut Cockpit, host: &mut OracleHost, mfd: &Mfd, body: &str) -> Result<(), String> {
    let start = format!(
        "(() => {{ globalThis.__xpOp = null; const fmc = (globalThis.__xpFmc || []).find(Boolean); const fps = fmc.flightPlanInterface; (async () => {{ {body} }})().then(() => {{ globalThis.__xpOp = 'ok'; }}, (e) => {{ globalThis.__xpOp = 'error: ' + ((e && e.stack) || e); }}); return 'started'; }})()"
    );
    cockpit.eval_in(&mfd.view, &start)?;
    for _ in 0..400 {
        host.step(cockpit);
        let state = cockpit.eval_in(&mfd.view, "String(globalThis.__xpOp)")?;
        if state == "ok" {
            return Ok(());
        }
        if state.starts_with("error") {
            return Err(format!("{body}: {state}"));
        }
    }
    Err(format!("{body}: still running after 20 s"))
}

fn snapshot(cockpit: &Cockpit, host: &OracleHost, mfd: &Mfd, label: &str) -> Result<serde_json::Value, String> {
    let raw = cockpit.eval_in(&mfd.view, SNAPSHOT_JS)?;
    let state: serde_json::Value = serde_json::from_str(&raw).map_err(|e| format!("snapshot {label}: {e}"))?;
    if let Some(err) = state.get("error") {
        return Err(format!("snapshot {label}: {err}"));
    }
    Ok(json!({ "label": label, "t": host.now_ms, "state": state, "mfd": mfd.positioned_texts(cockpit) }))
}

pub fn run_scenario(scenario: &serde_json::Value) -> Result<serde_json::Value, String> {
    let pkg = PathBuf::from(std::env::var("FMS_ORACLE_PKG").unwrap_or_else(|_| r"E:\fbw-build\fms-oracle-pkg".into()));
    let xplane = PathBuf::from(std::env::var("FMS_XPLANE_ROOT").unwrap_or_else(|_| r"D:\Steam Games\steamapps\common\X-Plane 12".into()));
    let mut cockpit = new_cockpit(&pkg)?;
    let mut host = attach_navdata(Recording::new(VarStore::new()), &xplane)?;
    let lat = scenario["lat"].as_f64().ok_or("scenario lat")?;
    let lon = scenario["lon"].as_f64().ok_or("scenario lon")?;
    let steps = scenario["steps"].as_array().ok_or("scenario steps")?;
    let uplinks = steps.first().is_some_and(|s| s.get("uplink").is_some());
    let mut mfd = None;
    if !uplinks {
        power_up_on_ground(&mut host, lat, lon);
        mfd = Some(wait_ready(&mut cockpit, &mut host)?);
    }
    let mut snapshots = Vec::new();
    for step in steps {
        if step.get("uplink").is_some() {
            load_route(&mut cockpit, &mut host, &canned_ofp_json())?;
            mfd = Some(Mfd::locate(&cockpit)?);
            continue;
        }
        let m = mfd.as_ref().ok_or("no MFD yet")?;
        if let Some(ms) = step.get("run_ms").and_then(|v| v.as_f64()) {
            host.steps(&mut cockpit, (ms / STEP_MS).ceil() as usize);
        } else if let Some(body) = step.get("fms").and_then(|v| v.as_str()) {
            fms_op(&mut cockpit, &mut host, m, body)?;
        } else if let Some(page) = step.get("page").and_then(|v| v.as_array()) {
            let s = |i: usize| page.get(i).and_then(|v| v.as_str()).unwrap_or("");
            m.select_page(&mut cockpit, &mut host, s(0), s(1))?;
        } else if let Some(var) = step.get("var").and_then(|v| v.as_array()) {
            let name = var.first().and_then(|v| v.as_str()).ok_or("var name")?;
            let unit = var.get(1).and_then(|v| v.as_str()).unwrap_or("number");
            let value = var.get(2).and_then(|v| v.as_f64()).ok_or("var value")?;
            host.inner.inner.set_var(name, unit, value);
        } else if let Some(label) = step.get("snapshot").and_then(|v| v.as_str()) {
            snapshots.push(snapshot(&cockpit, &host, m, label)?);
        } else {
            return Err(format!("unknown step {step}"));
        }
    }
    Ok(json!({
        "scenario": scenario["name"],
        "navdata": host.nav.date_range(),
        "snapshots": snapshots,
        "events": std::mem::take(&mut host.inner.events),
    }))
}
```

- [ ] **Step 4: Build the package, run the test, and check the result**

Run: `python tools/fms_oracle/make_pkg.py --replace`, then `CARGO_TARGET_DIR=E:/fbw-build/target cargo test --release --features js --lib fms_oracle -- --ignored --nocapture`.

Expected:
- the test prints `uplink-egll-kjfk: 2 snapshots, N events` and passes;
- in `E:/fbw-build/fms-oracle/uplink-egll-kjfk.json`, `snapshots[0].state.plans.active.legs` lists EGLL ... KJFK, with lat/lon on every non-discontinuity leg;
- `snapshots[1].mfd` contains the F-PLN page texts.

- [ ] **Step 5: Commit**

```bash
git add src/js/msfs/fms_oracle.rs tools/fms_oracle/scenarios/uplink-egll-kjfk.json
git commit -m "fms oracle: scenario runner recording FlyByWire's FMS as JSON goldens"
```

### Task 6: Three more scenarios and a determinism check

**Files:**
- Create: `tools/fms_oracle/scenarios/city-pair-procedures.json`
- Create: `tools/fms_oracle/scenarios/edits-tmpy.json`
- Create: `tools/fms_oracle/scenarios/secondary.json`
- Create: `tools/fms_oracle/same.py`

**Interfaces:**
- Consumes the runner from Task 5.
- Produces `tools/fms_oracle/same.py A B`. It compares two golden folders and ignores nothing. It exits 0 only when every file is byte-identical.

- [ ] **Step 1: Write the scenarios**

`city-pair-procedures.json`:

```json
{
  "name": "city-pair-procedures",
  "lat": 51.4706,
  "lon": -0.4619,
  "steps": [
    { "fms": "await fps.newCityPair('EGLL', 'KJFK', 'KBOS');" },
    { "run_ms": 3000 },
    { "fms": "const r = fps.active.availableOriginRunways.map((x) => x.ident).sort(); await fps.setOriginRunway(r[0]);" },
    { "run_ms": 2000 },
    { "fms": "const d = [...fps.active.availableDepartures].sort((a, b) => a.ident.localeCompare(b.ident)); if (d.length) await fps.setDepartureProcedure(d[0].databaseId);" },
    { "run_ms": 2000 },
    { "fms": "const a = [...fps.active.availableArrivals].sort((x, y) => x.ident.localeCompare(y.ident)); if (a.length) await fps.setArrival(a[0].databaseId);" },
    { "run_ms": 2000 },
    { "fms": "const p = [...fps.active.availableApproaches].sort((x, y) => x.ident.localeCompare(y.ident)); if (p.length) await fps.setApproach(p[0].databaseId);" },
    { "run_ms": 5000 },
    { "snapshot": "procedures" }
  ]
}
```

`edits-tmpy.json`:

```json
{
  "name": "edits-tmpy",
  "lat": 51.4706,
  "lon": -0.4619,
  "steps": [
    { "uplink": "canned" },
    { "run_ms": 5000 },
    { "snapshot": "before" },
    { "fms": "await fps.deleteElementAt(3);" },
    { "run_ms": 1000 },
    { "snapshot": "tmpy-after-delete" },
    { "fms": "await fps.temporaryInsert();" },
    { "run_ms": 2000 },
    { "fms": "await fps.insertDiscontinuityAfter(5); if (fps.hasTemporary) await fps.temporaryInsert();" },
    { "run_ms": 2000 },
    { "fms": "await fps.setPilotEnteredAltitudeConstraintAt(2, false, { altitudeDescriptor: '+', altitude1: 5000 }); if (fps.hasTemporary) await fps.temporaryInsert();" },
    { "run_ms": 2000 },
    { "fms": "await fps.addOrUpdateCruiseStep(8, 39000); if (fps.hasTemporary) await fps.temporaryInsert();" },
    { "run_ms": 5000 },
    { "snapshot": "after-edits" }
  ]
}
```

`secondary.json`:

```json
{
  "name": "secondary",
  "lat": 51.4706,
  "lon": -0.4619,
  "steps": [
    { "uplink": "canned" },
    { "run_ms": 5000 },
    { "fms": "await fps.secondaryCopyFromActive(1, true);" },
    { "run_ms": 2000 },
    { "fms": "await fps.deleteElementAt(2, false, 3);" },
    { "run_ms": 2000 },
    { "snapshot": "sec1-edited" },
    { "fms": "await fps.activeAndSecondarySwap(1, true);" },
    { "run_ms": 5000 },
    { "snapshot": "swapped" }
  ]
}
```

In `secondary.json`, plan index 3 is `FlightPlanIndex.FirstSecondary`, i.e. SEC 1.

`same.py`:

```python
import sys
from pathlib import Path

a, b = Path(sys.argv[1]), Path(sys.argv[2])
names = sorted({p.name for p in a.glob("*.json")} | {p.name for p in b.glob("*.json")})
bad = [n for n in names if not (a / n).exists() or not (b / n).exists() or (a / n).read_bytes() != (b / n).read_bytes()]
for n in bad:
    print("differs:", n)
print(f"{len(names) - len(bad)}/{len(names)} identical")
sys.exit(1 if bad else 0)
```

- [ ] **Step 2: Record twice and compare**

1. Run the Task 5 command with `FMS_ORACLE_OUT=E:/fbw-build/fms-oracle-run2`.
2. Run it again with the default output folder.
3. Run `python tools/fms_oracle/same.py E:/fbw-build/fms-oracle E:/fbw-build/fms-oracle-run2`.

Expected: `4/4 identical`.

If a file differs, find which value moves between runs (a wall-clock time, a random id or an unseeded uuid). Pin it in `setup_fixed_date`/`setup_seeded_crypto` or in `FmsHost`. Never drop it from the golden. The two runs must match before Phase 2 starts.

If a scenario step throws (for example, no approach at KJFK in this navdata), fix the scenario's JS so it picks something that exists, and record the AIRAC from `navdata` in the commit message.

- [ ] **Step 3: Commit**

```bash
git add tools/fms_oracle/scenarios tools/fms_oracle/same.py
git commit -m "fms oracle: procedures, temporary-plan edits and secondary-plan scenarios, recorded deterministically"
```

---

## Phase 2 onwards: what each plan will contain

- **Phase 2:**
  - Create `crates/fms` as a workspace member with no X-Plane dependency, and add `trait NavDatabase` (airports, runways, procedures, fixes, airways, navaids) implemented for `src/navdata::NavData` behind the plugin.
  - Translate the plan data model, segments, procedure stringing, discontinuities, temporary/secondary/alternate plans and the SimBrief uplink.
  - Add `tools/fms_oracle/compare.rs` (or a test): it replays each scenario's `fms` steps against the Rust FMS through a small command enum, and compares `plans` field by field with the spec's tolerances.
  - Translate FlyByWire's `*.spec.ts` for flightplanning into Rust tests.
- **Phase 3:**
  - Add a `fly` scenario op to the oracle: it moves the aircraft along the active plan at a set speed and writes position, track, ground speed and the ADIRS/GPS words the FMS reads.
  - Translate the LNAV geometry, transitions, path vectors, sequencing and guidance laws.
  - Compare the `A32NX_EFIS_VECTORS_*` events and the guidance variables.
- **Phase 4:**
  - Translate VNAV, predictions, performance, fuel, messages, EFIS symbols and the flight phase manager.
  - Every recorded variable and event must match.
- **Phase 5:**
  - Add `src/fms_live.rs`. In shadow mode it rebuilds the pilot's entries from FlyByWire's flight plan sync events, and logs `fms shadow:` differences.
  - It is authoritative only when `a380_displays` has claimed `SCREEN_DU_MFD`. The Rust display code skips claimed screens.
- **Phase 6:**
  - Create the C++ repo `E:/fbw-int/a380-displays` (CMake, X-Plane SDK `D:/A380/SDK`, NanoVG, nlohmann/json).
  - It covers: the device claim message, dimming and power, fonts, input callbacks, the model/action datarefs, and a headless renderer executable that writes PNGs from a model file.
  - Its first page is a test pattern for an in-sim check that the user launches.
- **Phase 7:**
  - Build the page widgets (field, dropdown, button, list, message line).
  - Then build the pages in groups: FMS ACTIVE, POSITION, SEC, DATA, ATCCOM, SURV, FCU BKUP.
  - Each page's PNG is compared with its Chromium capture.
- **Phase 8:**
  - The release ships the native FMS and MFD; the MFD bundle leaves `html_ui`.
  - Update `sync_release.py` with the new repo, and update THIRD_PARTY_NOTICES (NanoVG zlib, nlohmann/json MIT, stb public domain).

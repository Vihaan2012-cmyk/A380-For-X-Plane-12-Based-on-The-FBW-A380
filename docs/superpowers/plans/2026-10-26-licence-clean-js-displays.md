# Licence-clean JS displays

**Goal:** run FlyByWire's own A380 instruments (GPL-3 TypeScript) as the cockpit displays in X-Plane, with no Microsoft or Asobo code, package, tool or derivative anywhere in what we build or ship.

**What "zero risk" can mean:** no engineering plan can make legal risk literally zero; only a court or the rights holder can. This plan removes every source of Microsoft and Asobo material we know of, proves it with tools on every build, and records where each shipped file came from. After Tier 1, one argued question would remain: we reimplement an interface that FlyByWire's code calls. Tier 2 removes that too.

## Where the risk is today

| # | Source | Problem | Status |
|---|---|---|---|
| 1 | `@microsoft/msfs-sdk` (Avionics Framework) | MIT plus an addendum: the software "and any derivative works" are licensed only for use in MSFS. The SDK EULA also bans using any SDK component to develop a competing flight sim product. | Replaced at bundle time by `E:/fbw-int/clean-avionics` (3,516 lines, written from a spec, gate.py 2.34%). But the fork's `package.json` still installs `@microsoft/msfs-sdk` for the build. |
| 2 | `xp-kit` / `xp-avionics` | Shown on 21 Oct to copy SDK internals | Pulled. Must never come back. |
| 3 | Our shims for MSFS's page globals (`plugin/app/js/msfs/*`, `plugin/src/js/msfs/*`): `SimVar`, `Coherent`, `Simplane`, `Avionics.Utils`, `BaseInstrument`, `RegisterViewListener`, stored data | Written while reading Asobo's `fs-base-ui` scripts; the comments cite `SimPlane.js` and `common.js` line numbers. That makes them derived from MSFS game files. | **Not clean.** Must be rewritten. |
| 4 | Six FlyByWire files adapted from Microsoft code | Rewritten black-box on 13 Oct | Keep them under the gate. |
| 5 | FlyByWire's instruments (A380 about 125k lines, shared about 62k; 488 files import 94 SDK symbols) | GPL-3 is fine and its obligations are listed below. But the code is written against the SDK's interface. | Remaining question: Tier 2. |
| 6 | Fonts, images, CSS | `A380X_FCU.ttf` is third-party (the installer already downloads it). Anything from MSFS `html_ui` is Asobo's. | Needs an inventory. |
| 7 | Chromium (CEF) | BSD, with bundled notices | Ship the notices; build with no proprietary codecs. |

## Rules for every phase

- No Microsoft or Asobo package, tool, file or source in the X-Plane build tree or its `node_modules`, not even for type-checking.
- **Clean-room roles.**
  - The spec author may read FlyByWire source, public documentation, observed behaviour and our goldens.
  - The implementer reads only the spec.
  - Neither reads `@microsoft/msfs-sdk`, `msfs-avionics-mirror`, `fs-base-ui`, `xp-kit`, `xp-avionics` or `adapt-orig`.
- Similarity gates are run by a separate checker against those sources, and every release ships the gate reports.
- Everything shipped is either GPL-3 (FlyByWire-derived) or our own code, with FlyByWire credited. No FlyByWire, Microsoft or Asobo logos.

## Tier 1: clean runtime under FlyByWire's unchanged instruments

Rough effort is 7 to 10 working days. Every step ends with tests or a report.

1. **Provenance inventory (½ day).**
   - Add a script that lists every file in an install or release package with its origin: FlyByWire GPL, ours (clean), ours (unclean) or third-party, plus its licence.
   - It fails on any file of unknown origin, and writes `PROVENANCE.csv` into the release.
   - First run should flag items 1, 3 and 6.
2. **Microsoft-free build (1 day).**
   - Give the X-Plane build its own `package.json` and lockfile, with no `@microsoft/*`.
   - Resolve `@microsoft/msfs-sdk` imports to `clean-avionics` through a bundler alias, with types from its `spec/api.d.ts`.
   - CI fails if `node_modules` contains `@microsoft`, or if any bundle holds SDK strings or source maps. `install.sh` already checks the bundles; extend that to the build tree.
3. **Harden `clean-avionics` (1 to 2 days).**
   - Fill in whatever the 94 imported symbols still need, spec first.
   - Rerun `gate.py` on every file.
   - Acceptance: the behaviour probe (50 items) matches, and the goldens stay at 328/340 pixel-identical or better.
4. **Clean rewrite of the MSFS page globals (2 to 3 days).**
   - Write a spec from FlyByWire's call sites only:
     - `SimVar` (about 1,200 calls)
     - `BaseInstrument` (49)
     - `Avionics.Utils` (47)
     - `Coherent.call/on/trigger` (77)
     - `Simplane` (29)
     - `RegisterViewListener` (18)
     - stored data (7)
     - `LaunchFlowEvent` and `Utils.Translate`
   - The implementer writes new shims from that spec, and the old ones are deleted, not edited.
   - Add a second gate against `fs-base-ui` from the user's own MSFS install, run only by the checker.
5. **Assets (½ day).**
   - Inventory fonts: OFL is fine, anything third-party is fetched by the installer.
   - Images and SVG must come from FlyByWire (GPL) or be our own.
   - Nothing from MSFS `html_ui`.
6. **Host (½ day).**
   - Chromium (XPHFBW, currently `XPHFBW.off`) runs the instruments.
   - Each screen picks JS or native through the existing per-screen `nativeScreens` switch, so the two can run side by side.
7. **Verify (1 to 2 days).**
   - Goldens, the probe, then one full flight in the sim.
   - `package_release.py` refuses to package without passing gate and provenance reports.
8. **GPL compliance (½ day).**
   - Public source, including build scripts and `clean-avionics`.
   - `LICENSE`, `THIRD_PARTY_LICENSES.txt` and the FlyByWire notices.

**What remains after Tier 1:** Microsoft could argue that FlyByWire's SDK-shaped code, or our compatible reimplementation, falls under the "derivative works" addendum. Case law leans against that: Google v. Oracle (US, 2021) on reimplementing interfaces, and SAS v. WPL (EU, 2012) on functionality not being protected. That is still an argument, not a certainty.

## Tier 2: no Microsoft-shaped interface

Rough effort is 2 to 4 weeks.

- A codemod moves FlyByWire's instrument sources onto our own component and event API. The 94 imported symbols each map to a differently named and structured equivalent. The instruments then compile only against our API, and `clean-avionics` stops pretending to be the SDK.
- The logic is unchanged and still GPL-3 FlyByWire. Goldens and the probe are the acceptance tests, as in Tier 1.
- Result: nothing we build or ship has the SDK's interface, which closes the remaining question.

## Outside engineering

- Optional: one hour with an open-source lawyer on the remaining question, before the first public release.
- Tell FlyByWire what ships; it is GPL, so this is a courtesy.
- Microsoft has said it will not answer this kind of question, so there is no one to ask there.

## Decisions needed

1. Tier 1 only, or Tier 1 then Tier 2?
2. Do JS displays replace the native ones, or sit beside them per screen? On 22 Oct the decision was native-only, to drop Microsoft code entirely.

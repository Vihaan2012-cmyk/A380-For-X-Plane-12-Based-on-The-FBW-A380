# MFD pages B: INIT, FUEL & LOAD, PERF

**Goal:** native page trees (`mfd_tree::Node`) for FlyByWire's A380X MFD FMS ACTIVE pages INIT, FUEL & LOAD and PERF, as pure functions from a page model struct to the DOM FBW's TSX renders.

**Crate:** `crates/mfd_pages_b` (own `Cargo.toml`, path dependency on `crates/mfd_tree`). Build with `CARGO_TARGET_DIR=E:/fbw-build/target`.

## Layout

- `src/widgets.rs`: minimal swappable helpers (`field`, `Dropdown`, `button`, `radio_group`, `conditional`, `confirmation_dialog`, svgs). Replace with `mfd_pages_a` widgets once ready.
- `src/formats.rs`: the `format()` half of the DataEntryFormats used by these pages, plus weight and `fmcFormatValue` text helpers.
- `src/init.rs`: `InitModel`, `init_page` from `MfdFmsInit.tsx` (render at 412 to 790; button label logic from `CpnyFplnButtonUtils.ts` and `CpnyWindButtonUtils.tsx`).
- `src/fuel.rs`: `FuelModel`, `fuel_load_page` from `MfdFmsFuelLoad.tsx` (render at 436 to 918).
- `src/perf.rs`: `MfdFmsPerf.tsx` render at 1357 to 3500 (all six tabs are always in the DOM, only `display` and the tab-bar `active` classes change). Done (1520 lines in perf.rs).
- `src/bin/emit.rs`: `emit <init|fuel|perf> <model.json> <out.json>`.

## Proof

Output folder `E:/fbw-build/mfd-pages-b`: `models/*.json` (page models, documented below), `out/<capture>.json`, `run_compare.py`.

The captures contain several `mfd-page-container` nodes (the FPLN dialogs come first), so `compare.py ... cls=mfd-page-container` selects the wrong one. `run_compare.py` reuses `compare.py`'s `norm`/`diff` and picks the page container that is a direct child of `mfd-navigator-container` in the left MFD.

Model values were parsed from the capture text by hand (no raw FMS golden exists for these pages in `E:/fbw-build/fms-oracle`); model fields are raw values (kg, minutes, ft, indices) plus UI state (open dropdown, flags).

## Status

- [x] widgets, formats
- [x] INIT: 0 differences on 0224 and 0227
- [x] FUEL & LOAD: 0 differences on 0203 and 0206
- [x] PERF: 0 differences on all 12 PERF captures (0171, 0173, 0176, 0179, 0181, 0184, 0187, 0189, 0192, 0195, 0198, 0200); `E:/fbw-build/mfd-pages-b/run_perf.py` builds per-capture models from `models/perf_base.json` and compares
- [x] unit tests in `src/tests.rs` (INIT, FUEL, PERF model to tree assertions)
- [ ] PERF states without captures: noise enabled, V-speed confirmation, derated dialog, approach/CLEAR EO dialogs, in-flight phases (covered only by unit assertions)

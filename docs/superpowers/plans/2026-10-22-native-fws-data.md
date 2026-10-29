# Native FWS data: abnormal procedures and alert metadata

Crate `crates/fws_data` (standalone, no dependencies, own `[workspace]`): FlyByWire's A380X ECAM abnormal procedure data as static Rust tables. fws25 owns the alert condition logic (`src/fws/`); fwsdataB27 owns normal procedures, memos, limitations and INOP SYS text (`crates/fws_ecl`). This crate only carries the static metadata and refers to conditions by name.

## Source

All under `E:/fbw-int/fbw-aircraft/fbw-a380x/src/systems`:

| FBW file | Content | Rust |
|---|---|---|
| `instruments/src/MsfsAvionicsCommon/EcamMessages/AbnormalSensed/ata*.ts`, `secondary-failures.ts` | `AbnormalProcedure` per id: title, sensed, recommendation, items | `Procedure`, `Item` |
| `.../EcamMessages/AbnormalNonSensedProcedures.ts` | non-sensed overview list (id, category) | `Procedure.non_sensed` |
| `.../AbnormalSensed/ata*.ts` `EcamDeferredProcAta*` | `DeferredProcedure`: fromAbnormalProcs, type, items | `Deferred` |
| `systems-host/CpiomC/FlightWarningSystem/FwsAbnormalSensed.ts` (`ewdAbnormalSensed`) | per-alert FWS entry | `Alert` |
| `.../FwsAbnormalNonSensed.ts` (`ewdAbnormalNonSensed`, `ewdDeferredProcs`) | same for non-sensed and deferred | `Alert` |

## Generation

`tools/fws_data/extract.py` regenerates everything; nothing is hand-copied.

1. `entry.ts` is bundled with FBW's esbuild (like `app/tools/efb_docs`) and run with node: it prints every procedure dictionary, one per source file, with the real runtime values.
2. `fws_meta.cjs` loads FBW's own `typescript` and parses the three `Fws*` files. For each entry of the `ewd*` object literals it reads the static metadata out of the AST: flightPhaseInhib, failure, sysPage, auralWarning, monitorConfirmTime, cancel, nonProcedureKey, notActiveWhenItemActive, the names `this.fws.X` / `L:` vars used by `simVarIsActive`, the shape of `whichItemsToShow/Checked/Active/Timer`, and the string ids returned by `inopSysAllPhases`, `inopSysApprLdg`, `redundLoss`, `limitationsAllPhases`, `limitationsApprLdg`, `limitationsPfd`, `info`.
3. `extract.py` merges both, counts the raw `NNNNNNNNN: {` entries of every source file by regex as an independent check, and writes `crates/fws_data/src/generated/*.rs`.

Run: `python tools/fws_data/extract.py`. Test: `cargo test --manifest-path crates/fws_data/Cargo.toml`.

## Types (`src/types.rs`)

`Procedure { id, ata, title, plain_title, system, colour, kind, recommendation, items, alert }`, `Item { name, kind (Action / Condition / Plain), sensed, level, style, label_not_completed, label_completed, colon_if_completed, time, append_time_if_elapsed }`, `Alert` (FWS metadata), `Level`, `Phases` (bit set of FWC phases 1..12), `SdPage`, `Aural`, `Deferred`, `DeferredType`, `Recommendation`, `Category`.

Field mapping is documented on each field in the source.

## Tests

- count of procedures per ATA source file equals the independent regex count of the FBW tables;
- every Alert refers to a Procedure (or is a `nonProcedureKey` entry), every `notActiveWhenItemActive` id exists as an alert except 7 dangling FBW references, which are pinned;
- spot checks: engine 1 fire (260800005), hydraulic green system pressure low, gear not downlocked.

## Not covered

Conditions (`simVarIsActive` logic, `whichItems*` bodies) are fws25's. INOP SYS / limitation / info texts are fwsdataB27's; only their ids are linked here.

## Result (22 Oct)

996 procedures (958 sensed, 31 non-sensed overview entries, 7 secondary failures), 8 deferred, 291 alerts (`ewdAbnormalSensed` 268, `ewdAbnormalNonSensed` 16, deferred 7). FBW itself defines alerts for only 283 of the 996 procedures (the rest are data without a FWS trigger); 8 alerts have no procedure (7 deferred, 1 `nonProcedureKey`).

FBW data quirks kept as is and pinned in the tests: 7 `notActiveWhenItemActive` ids that point at no alert (314800004, 314800003, 340800010, 340800071), and 990900011 whose `whichItemsToShow` has 15 entries for 12 items. `failure: -1` (`Level::Hidden`) appears on 290800040 and 314800006. `flightPhaseInhib: this.fws.phase56Inhibition` is resolved to `[5, 6]` from FwsCore. FBW's circular import between `EcamMessages/index.ts` and the ATA modules leaves `FMS_PRED_UNRELIABLE_CHECKLIST_ITEM` undefined while the tables load; extract.py re-inserts it (count checked against the `...FMS_PRED_UNRELIABLE_CHECKLIST_ITEM` uses in the source) and `entry.ts` must keep importing `EcamMessages` first.

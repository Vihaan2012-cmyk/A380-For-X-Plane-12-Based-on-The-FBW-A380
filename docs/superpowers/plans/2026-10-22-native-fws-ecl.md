# Native FWS data: normal procedures, memos, limitations, status (fwsdataB27)

Date: 22 October 2026. Crate `crates/fws_ecl` (standalone, own `[workspace]`, no dependencies). Generator `tools/fws_ecl/extract.mjs`. fws25 owns the live evaluation, fwsdataA27 owns abnormal procedures (`crates/fws_data`).

## Source (FlyByWire GPL, `fbw-aircraft/fbw-a380x/src/systems`)

| Data | File |
|---|---|
| Normal procedures (ECL), deferred ids | `instruments/src/MsfsAvionicsCommon/EcamMessages/NormalProcedures.ts` |
| Sensed flags per ECL item | `systems-host/CpiomC/FlightWarningSystem/FwsNormalChecklists.ts` (`sensedItems`) |
| Memo, info, limitation, inop sys texts, PFD memo list, line constants | `EcamMessages/index.ts` |
| Memo rules (phase inhibit, active input, code selection, inhibit) | `FwsMemos.ts` (`ewdMemos`, `ewdToLdgMemos`) |
| Limitation rules | `FwsLimitations.ts` |
| Info rules | `FwsInformation.ts` |
| Status INOP SYS rules | `FwsInopSys.ts` |

## Approach

1. `extract.mjs` bundles `tools/fws_ecl/entry.ts` with FBW's esbuild and runs it in node for the text tables and the checklists (the same technique as `app/tools/efb_docs/build.py`).
2. The same script parses the five Fws*.ts files with the TypeScript compiler API (no execution, so no msfs-sdk) and pulls each rule's static fields. The live conditions stay as source text plus a list of the `this.fws.<name>` / `SimVar` inputs they read, for fws25 to wire.
3. It writes `src/normal_data.rs`, `src/message_data.rs`, `src/rule_data.rs` (generated, do not edit).
4. Pure data mappings are hand-written in `src/logic.rs`: T.O and LDG memo line selection, ADIRS align memo selection, memo and limitation colour priority, LAND ASAP replacing LAND ANSA, escape-code parsing.

## Types (`src/types.rs`)

`LineStyle`, `Item` (`ItemKind::{Action, Condition, Special}`, level, style, timed), `NormalProcedure` (items plus one `Option<Check>` per item), `Message` (id, raw text with escape codes, colour, plain text), `MemoRule` (`MemoSelect`, `MemoInhibit`, phase inhibit, inputs), `LimitationRule`, `InfoRule`, `InopRule`, `RulePhase`.

## Tests

Counts against FBW's tables (checked at generation time and against literal numbers), items vs sensed-flag lengths, every rule id has a text, T.O memo items, LDG memo items, a limitation (`MAX SPEED: 310/.86`), an inop sys item, selection functions.

## Left

Live condition evaluation (fws25), abnormal sensed/non-sensed procedures and deferred procedures (fwsdataA27), the ECL line generator state machine (`ProcedureLinesGenerator.ts`), FwsCore memo ordering beyond the pure helpers.

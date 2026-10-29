# Native TCAS (crate `crates/tcas`)

Goal: replace the SystemsHost JavaScript `LegacyTcasComputer` (traffic tracking, TA/RA threat levels, RA sense selection, inhibitions, outputs) with a standalone Rust crate, proven against FlyByWire's own JS.

Sources (FlyByWire A380X): `fbw-a380x/src/systems/systems-host/Misc/tcas/components/LegacyTcasComputer.ts` (1377 lines) and `.../lib/TcasConstants.ts` (849 lines). FBW has no TCAS tests, so the proof is an offline oracle.

## Crate

- Standalone `crates/tcas` (own Cargo.toml, std only, serde_json as dev-dependency), no X-Plane or plugin dependency.
- `Tcas::update(dt_ms, &Inputs, &[RawTraffic]) -> Update`. One call per 1 s tick (FBW `REFRESH_RATE`), dt in ms, first dt is 0 as in FBW.
- Inputs are the L:/SimVar values FBW reads (ARINC 429 words stay raw f64 as stored in L:vars); outputs are the L:vars FBW writes (`A32NX_TCAS_*`), the `A32NX_TCAS_L/R_TRAFFIC` generic data lists and the aural calls.
- FBW's async `GET_AIR_TRAFFIC` result is modelled as resolved right after the synchronous part of the same tick, so a fetch feeds the next tick's threat logic exactly as in the JS.
- Framework utilities (MathUtils distance/heading/ellipse, ARINC 429 decode, JS `Array.sort` semantics including NaN comparators, `Math.round`) are independent Rust equivalents.
- FBW quirks are kept on purpose (stale `hrzDistance`, sense override chain, `null` separations in the reversal logic).

## Oracle

- `E:/fbw-build/tcas-oracle/`: esbuild bundles the real `LegacyTcasComputer.ts` with stubs for the SDK/event bus/sound manager/SimVar/Coherent and runs scripted scenarios in Node (V8, not QuickJS; trig results are compared with tolerance).
- `gen.mjs` writes `golden/<scenario>.json` (inputs and per-tick outputs, non-finite numbers as strings). Copies live in `crates/tcas/tests/golden/`.
- `tests/oracle.rs` replays every golden through the Rust crate and compares discrete outputs exactly, contact positions to 1e-12 and tracked taus, rates and distances to 1e-6 relative.
- Scenarios (74, 3,000+ ticks): head-on above/below with and without RA compliance, reversal against a counter-manoeuvring intruder, crossing, overtaking, TA-only, altitude-select and EFIS traffic filters, ground and bugged traffic, junk zero positions, slew, radio-altitude inhibition bands, FL410, STBY, XPDR/air-data/IR switching, power loss, ARINC failures, GPWS warning, display cap, 300-contact memory cull, and 40 seeded random encounters.

## Left to the plugin

Feed `Inputs` from the Vars store, feed `RawTraffic` from `mapdata::traffic`, write `Update` back to the L:vars and the generic data topics, route aural calls to the sound triggers, then stop the JS computer.

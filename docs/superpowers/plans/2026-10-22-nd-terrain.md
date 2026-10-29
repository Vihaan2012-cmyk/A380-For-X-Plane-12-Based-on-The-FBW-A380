# ND terrain (TERR) and VD terrain profile from X-Plane probes

Goal: colour-banded terrain on the native ND like the A380's TAWS display, and the terrain profile on
the VD, both from what X-Plane actually renders (XPLMProbeTerrainXYZ).

## 1. Audit (what already exists)

- **FlyByWire (MSFS):** EGPWC (Rust) -> terronnd.wasm gauge -> SimBridge terrain worker (gpu.js
  kernels, terrain.map SRTM tiles) -> PNG frame under nd.html; the ND page only draws the TERR legend
  from `L:A32NX_EGPWC_ND_<L|R>_TERRAIN_MIN/MAX_ELEVATION(_MODE)`. Details: docs/map-data.md section 1.
- **Plugin, src/mapdata/terrain/ (existing, wired):** a line-for-line CPU port of that SimBridge chain
  (normal + peaks modes, density patterns, histogram percentiles, cut-off near destination, VD profile
  with grey area), fed from X-Plane's **DSF elevation rasters** (scenery.rs/dsf.rs/tiles.rs), not from
  probes. Output: a 768 x 1024 RGBA image per side, handed to the native ND as its underlay
  (src/native_displays/screen.rs, `crate::mapdata::plugin::terrain_layer`, WXR image preferred when
  shown). Not verified in sim (map-data.md "Only X-Plane can verify").
  Gaps vs the real A380: no solid caution/warning areas from the TAWS look-ahead; the DSF raster is
  the base mesh only (no custom-mesh/airport flattening, no overlay edits); no probe source.
- **crates/gpws taws.rs:** look-ahead FLTA/obstacle/PDA/TCF logic over a `TerrainSampler` trait;
  outputs alert levels and one threat point, not an area.
- **a380-displays:** displays/nd/nd.cpp draws the TERR legend (`terrainThresholds`) and VD flags;
  nd/vd.cpp states the VD terrain fill is in the layer image; core c_api has `a380d_set_underlay`.
  Before this change nothing in the displays drew terrain itself.

## 2. Design

### Sampler (crates/terrain_map, plain Rust, own empty [workspace])

- `grid::GeoGrid`: square geo-referenced lattice (lat step = cell/60 deg, lon step fixed at anchor
  latitude), cells `Unknown | Miss{tries} | Land(ft) | Water(ft)`. Recentres when the aircraft moves
  more than 1/8 of the grid, copying the overlap (antimeridian safe); re-anchors past 2 deg of latitude.
- `scheduler::TerrainMap`: four levels, 176 x 176 cells of 0.25 / 1 / 4 / 8 nm (GENERIC), covering
  ARC 20 up to the 640 nm ring. `plan(lat, lon, track, radius, budget)` returns at most `budget`
  probe requests per frame: 3/4 on the level the displays need, rest on the others; nearest first,
  cells ahead of the track (or within a quarter grid) before those behind; misses retried every
  120 frames, at most 3 times (GENERIC). `store(req, Probe)` drops results for a grid that moved.
  Main-thread cost = budget x one XPLMProbeTerrainXYZ + two coordinate conversions; everything else
  (classification, profile, JSON) can run on a worker from a snapshot.
- `bands::classify`: FlyByWire's rules (SimBridge navigationdisplay.ts as ported in
  navigation_display.rs): histogram over the cells in range (ahead half in ARC), reference altitude =
  altitude + VS/2 when VS <= -1000 fpm, gear offset 250/500 ft, normal-mode thresholds (green low/high,
  amber low/high, red high), peaks mode when the highest terrain is below reference - gear offset
  (green low/high/solid by percentiles), cut-off near the destination (`cut_off_altitude`), water and
  no-data cells. The TERR legend values (min/max, modes) are computed as FBW's analyzeMetadata.
- `alert::AlertArea`: cells inside the TAWS look-ahead cone (track, half angle + start width) within
  gs x caution/warning time (capped at the look-ahead distance) whose terrain + clearance reaches the
  predicted altitude become solid amber (caution) or solid red (warning), only while that alert is
  active. Constants come from gpws `taws::params` in the plugin hook.
- `profile::profile_along`: highest terrain across a cut `2 x TAWS RNP` wide (max 10 nm) along a
  polyline (track line, manual azimuth or the flight plan), at a fixed step.
- `publish`: bus payloads `a380.terrain.nd.<L|R>` (grid origin/steps, mode, legend values, base64
  band codes) and `a380.terrain.vd.<L|R>` (step, grey-area start, elevations, kinds L/W/U).
  `{"off":true}` hides either.

### Colour rules and sources

| Band | Rule | Source |
|---|---|---|
| Red, high density | terrain >= ref + 2000 ft | FBW SimBridge (normal mode) |
| Amber, high density | ref + 1000 .. + 2000 ft | FBW SimBridge |
| Amber, low density | ref - 500 (250 gear down) .. + 1000 ft | FBW SimBridge |
| Green, high density | ref - 1000 .. lower amber limit | FBW SimBridge |
| Green, low density | ref - 2000 (or percentile/half-max when flat) .. ref - 1000 | FBW SimBridge |
| Peaks: green low/high/solid | percentiles 85/95 %, 65 %/95 % of the elevation span | FBW SimBridge |
| Cyan dots | water | FBW SimBridge |
| Magenta dots | no terrain data | FCOM DSC-34-20-20-20 (ND, terrain colours note); FBW |
| Solid amber / solid red | terrain conflicting with the caution / warning criteria | FCOM DSC-34-20-20-10 (TAD, terrain alerts) |
| VD: brown terrain, cyan water, grey beyond the turn | always, no alert colours | FCOM DSC-34-20-20-10 (terrain function); DSC-31-20-40-20 (grey area); FBW vertical display colours |
| VD cut width | 2 x TAWS RNP, max 10 nm | FCOM DSC-31-20-40-10 |
| Shown in ROSE and ARC only, auto-shown on alert | | FCOM DSC-34-20-20-20, DSC-34-20-20-10 |

Dot densities (4 x 4 ordered dither, low 4/16, high 8/16, water 3/16, no-data 8/16) are GENERIC:
FBW's pattern images are not reused in the native layer.

### Display layers (a380-displays)

- `displays/nd/terrain.cpp` (new): `draw_terrain(canvas, in, frame)`, called once in nd.cpp before
  `blackAreas` (where the underlay would be). ND part: inverse-maps every device pixel of the map
  area (y 128..720 CSS, inside the ARC 498 px / ROSE 250 px ring, not PLAN) through the Frame's
  rotation/scale (local flat earth) to a grid cell, applies the band colour and dither, caches the
  image by (gen, position, up, scale, mode). VD part: columns from the bottom to the terrain height
  in the VD box (150, 800, 540 x 200) using `vd_vertical_range`, brown/cyan/grey/magenta.
- Hook: one line in nd.cpp, one declaration in nd_layers.h. vd.cpp and oans.cpp untouched.

## 3. Plugin hook (not done: src/lib.rs / Cargo.toml are not mine)

1. Cargo.toml: `terrain_map = { path = "crates/terrain_map" }`.
2. New module (e.g. `src/terrain_probe.rs`), slot `tick-after-systems`:
   - `XPLMCreateProbe(xplm_ProbeY)` once; each frame `map.plan(lat, lon, track, radius, 256)`
     (budget GENERIC; measure in sim), where radius = ND range (ARC) or range/2 (ROSE), at least the
     VD range.
   - per request: `XPLMWorldToLocal(lat, lon, 0)` -> `XPLMProbeTerrainXYZ(probe, x, 0, z)`; on
     `xplm_ProbeHitTerrain`: `XPLMLocalToWorld(info.locationX/Y/Z)` -> metres x 3.2808 ft,
     `water = info.is_wet`; else `Probe::Miss`. Probes only hit loaded scenery: for misses
     (ranges >= 160 nm) fall back to `mapdata` DSF tiles (`MapHandler::extract_elevation`) instead
     of storing a miss.
   - every 500 ms per side (GENERIC; FBW scanline cycle is 600 ms): `classify` with altitude/VS/gear
     from the EGPWC inputs, `arc_up_deg = Some(true heading)` in ARC, `cut_off_altitude` from
     `EGPWC_DEST_LAT/LONG`, `AlertArea` from `gpws::Taws::outputs().flta` + `taws::params`
     (CAUTION/WARNING_TIME_S or the terminal values, CONE_HALF_ANGLE_DEG, CONE_START_WIDTH_NM,
     MAX_LOOKAHEAD_NM, clearance profile). Publish `nd_json` when TERR on ND is active
     (`L:A32NX_EGPWC_ND_<L|R>_TERRAIN_ACTIVE`) or an alert is active, else `{"off":true}`; write the
     legend values to `L:A32NX_EGPWC_ND_<L|R>_TERRAIN_MIN/MAX_ELEVATION(_MODE)`.
   - VD: path = aircraft + FMS legs when managed, else the track line; `profile_along(.., vd_range,
     vd_range / 270, cut_width_nm(TAWS RNP))`; `vd_json`, always on (FCOM: VD shows terrain even
     without TERR).
   - Hand both strings to the ND inputs with `a380d_set_bus` (native_displays inputs feed).
3. src/native_displays/screen.rs: do not set the SimBridge terrain underlay for a side while the probe
   payload is published (WXR underlay unchanged), or the terrain is drawn twice.
4. gpws: implement `TerrainSampler` with `|lat, lon| map.elevation_ft(lat, lon)` so the TAWS
   look-ahead and the ND use the same terrain.

## 4. Status

- crates/terrain_map: grid, scheduler, bands, alert, profile, publish; 25 tests pass
  (`cargo test --release`, CARGO_TARGET_DIR=E:/fbw-build/target). `examples/payload.rs` writes demo
  payloads for a synthetic ridge.
- a380-displays: nd/terrain.cpp + hook build in E:/fbw-build/a380-displays/terr51 (nd only). Demo
  render: E:/fbw-build/terr-native/arc40.png (v12 0097 ARC 40 state + synthetic payload).
- Not verified in X-Plane: probe cost, scenery-load misses, dither look against the real display.

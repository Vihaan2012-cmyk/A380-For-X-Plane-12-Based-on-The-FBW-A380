# WXR additions (crates/wxr_extras)

Goal: three things a real A380 radar does that `src/wxr` does not: predictive windshear (PWS) with its alerts and ND icon, a proper turbulence (spectrum-width) detector for the WX+T magenta returns, and weather on the ND vertical display (VD). All logic lives in `crates/wxr_extras`, a standalone crate with no simulator dependency; the plugin only supplies a sampler and consumes outputs.

## What exists

`src/wxr/` (docs/wxr.md has the long version):

- Data source: `XPLMGetWeatherAtLocation` (`xp::weather_at_location`, main thread only, "not intended to be used per-frame"), one point per call. Fields used today: `precip_rate_alt` (0..1 ratio), `turbulence_alt` (0..1), `temperature_alt`, cloud layer tops. Fields fetched by `xp.rs` `WeatherInfoRaw` but unused by WXR: `wind_dir_alt`, `wind_spd_alt`, `thermal_climb`, `wind_layers[].shear/turbulence/gust_speed`. There is no reflectivity, Doppler or vertical-wind field in the SDK, and no discrete microburst feature in X-Plane's weather.
- `sampler.rs`: `WeatherSampler` trait on (lat, lon, alt) returning `Sample { precip_rate_alt, turbulence_alt, temp_c, storm_top_m }`; `XplmSampler` is the live one.
- `mod.rs`: round-robin sweep, 61 azimuths (+-60 deg) by 24 range bins, 6 samples per tick per active side; geo-referenced point store; `NATIVE_IMAGE` ids `WXR_L`/`WXR_R` (768x1024) drawn under the native ND; selectors `A380X_EFIS_{L,R}_ACTIVE_OVERLAY`, `A32NX_EFIS_{L,R}_ND_MODE/RANGE`, controls `A380X_WXR_{L,R}_{ELEVN_TILT,ELEVN_FT,TILT_DEG,GAIN_MAN,GAIN_PCT,MODE_MAP}`, `A380X_WXR_OFF`, `A380X_WXR_TURB_OFF`, `fbw/cockpit/SWITCH_RADAR_GCS`; failure follows AESU power through `source_patches` (`wxr1Failed/wxr2Failed`).
- `physics.rs::classify`: rain rate = 60 p^1.3 mm/h, Marshall-Palmer Z = 200 R^1.6, ARINC 708 bands (20/30/40/50 dBZ), two-way attenuation with 6 dB compensation, partial beam fill, phase correction, turbulence = magenta when `turbulence_alt >= 0.5`, inside 40 NM, in precipitation.
- Gaps: no windshear anything on the radar side (only reactive `gpws` crate `windshear_warning` from F-factor, and `deep/environment/wind_shear.rs` which is an aircraft-motion model, not detection); turbulence is a bare threshold on one point value; nothing draws weather on the VD (FBW only has the "WXR INOP" flag; the SURV page's `pred_ws_auto`, `wxr_on_vd`, `turb_auto` are unbound subjects, `crates/mfd_pages_d/src/surv.rs` has the buttons).

## How the real thing works (public descriptions only)

Sources: Honeywell IntuVue RDR-4000 and Collins WXR-2100 MultiScan public product and pilot guides, RTCA DO-220 (airborne weather radar with forward-looking windshear detection), TSO-C63d/TSO-C117a, FAA AC 00-54 and AC 25-12 summaries, Bowles' F-factor papers, ARINC 708 colours. Numbers below are the commonly published figures; all are `PwsConfig`/`TurbConfig` fields and flagged unverified against a type-specific manual.

Predictive windshear (PWS):

- The antenna makes dedicated low-elevation scans ahead of the aircraft, independent of the ND tilt, measuring reflectivity and Doppler radial velocity (also in dry air from ground-clutter Doppler). Radial wind change along each radial gives the longitudinal gradient; downdraft is inferred from reflectivity and spectrum width. These combine in the F-factor hazard metric F = (V/g) dWx/dx - Wh/V (Wx tailwind component, Wh updraft); F above about 0.105 is hazardous. Shear that increases airspeed (negative F) is not alerted.
- Active automatically (AUTO) on the takeoff roll once takeoff power is set and in the air below roughly 1200-2300 ft RA depending on make; this plan uses 1500 ft as the user asked. Scan sector about +-40 deg, alert sector about +-25 deg, range up to about 5 NM; cautions out to about 3 NM, warnings inside about 1.5 NM.
- Alerts: caution = amber W/S AHEAD plus aural "MONITOR RADAR DISPLAY"; warning = red W/S AHEAD plus aural "WINDSHEAR AHEAD" (takeoff) or "GO AROUND, WINDSHEAR AHEAD" (approach), repeated while it persists. ND shows the windshear icon (red or amber chevron arc) at the bearing and range of the hazard, also when aural alerts are inhibited. PRED W/S OFF/FAIL is a status message.
- Inhibitions: PWS OFF on the SURV page, radar unpowered or failed, invalid radio altitude or air data, above the ceiling, on the ground except the takeoff roll; aurals and PFD text inhibited between 100 kt on the roll and 50 ft after lift-off and below 50 ft on approach; lower priority than TCAS RA, GPWS warnings and reactive windshear.

Turbulence:

- Turbulence is the width of the Doppler velocity spectrum inside a resolution volume, only meaningful where there is precipitation to scatter from. Magenta is painted when the width exceeds about 5 m/s (about 10 kt, moderate or worse) inside about 40 NM, in WX+T or TURB, and automatically below about 2300 ft RA in AUTO. Beyond 40 NM turbulence is not detectable.
- Width comes from two parts: genuine turbulent eddies and shear across the beam volume.

Weather on the VD:

- With the SURV "ON VD" option the radar builds a vertical slice along the VD's path (the track line or selected azimuth): reflectivity versus altitude and distance, same four colours as the ND, magenta where turbulence is detected, with storm tops and bases visible. It follows the VD range and altitude scales.

## Data needed from X-Plane

| Need | X-Plane source | Status |
| --- | --- | --- |
| Wind vector at point | `wind_dir_alt` (from, deg true), `wind_spd_alt` (m/s) at the queried altitude: u = -s sin(d), v = -s cos(d) | in struct, unused |
| Vertical wind | `thermal_climb` (m/s) as a floor, plus a downdraft term from convective intensity (`precip_rate_alt`, `turbulence_alt`, cell recipe in `deep/environment/weather_cells.rs::downdraft_peak_ms`) | needs sim check |
| Precip, turbulence | `precip_rate_alt`, `turbulence_alt` | in use |
| Temperature, tops | `temperature_alt`, cloud layers (`storm_top_m`) | in use |

Caveats: X-Plane has no discrete microbursts, so PWS only fires on whatever horizontal wind gradient and convective downdraft proxy X-Plane's field has, plus anything the plugin injects. The existing deep `MicroburstEvent`/`WeatherCellField` can be laid over the live sampler (a `WeatherSampler` wrapper adds the burst field to the XPLM sample), which also makes the feature testable in the sim. Cost: a PWS scan is 17 x 20 = 340 point samples; the scanner is time-sliced (budget per step) so the live cost matches the existing WXR budget; cells older than 12 s are discarded. The vertical structure of `precip_rate_alt` with altitude is unverified; VD falls back to cloud top for the upper edge.

## Crate design (crates/wxr_extras)

- `sample`: `Point3 { east_m, north_m, alt_m }` in a plugin-chosen local frame, `WeatherPoint { wind_east_ms, wind_north_ms, vertical_ms, turbulence, precip, temp_c, cloud_top_m }`, trait `WeatherSampler`.
- `geom`: `Ownship`, beam point (heading + azimuth + range, tilt, earth curvature).
- `refl`: rain rate, dBZ, `Level`, phase and attenuation helpers (same constants as `src/wxr/physics.rs`).
- `pws`: `PwsScanner::step(inputs, sampler, budget) -> PwsOutput { status, severity, aural, pfd_ws_ahead, icon, alerts_inhibited }`, F-factor per cell from the radial wind gradient, cluster confirmation, region by nearest hazard range, hold time, aural events with repeat, inhibits.
- `turb`: spectrum width, `TurbulenceScanner` returning magenta returns with their local positions, mode and auto rules.
- `vd`: `VdScanner` builds a range-by-altitude `VdImage` of `Level`s, coherent per sweep, with attenuation along each row.
- `synthetic`: test fields (microburst with outflow ring and downdraft, convective cells) also usable by plugin tests.

Tests: `cargo test --manifest-path crates/wxr_extras/Cargo.toml` with `CARGO_TARGET_DIR=E:/fbw-build/target`.

## Plugin hooks (not done; plugin src untouched)

1. `src/wxr/sampler.rs`: add `XplmFieldSampler` implementing `wxr_extras::WeatherSampler`: local frame = plugin-held origin lat/lon (`east = (lon-lon0) * 111320 * cos(lat0)`, `north = (lat-lat0) * 110574`); wind from `wind_dir_alt/wind_spd_alt` (needs `WeatherSample` extended with those two fields plus `thermal_climb`, `src/deep/weather.rs` and `xp.rs::weather_at_location`); optional overlay of `deep` microburst/cell field. Main thread only.
2. `src/wxr/mod.rs::tick_with`: after the sweep, call `PwsScanner::step` (budget about 12 per tick, only when the PWS gate is open) with `PwsInputs` built from: `sim/flightmodel/position/*` (elevation, true_psi, local velocities), radio altitude (`A32NX_RA_1_RADIO_ALTITUDE` or the FWS input already used by `gpws`), on-ground, TAS/GS from ADIRS, VS, FWC flight phase to `PwsPhase`, thrust levers (takeoff power), `A380X_WXR_PRED_WS_AUTO` (new L:var from the SURV button, now an unbound subject), radar power/failed (`wxr_failed`), a flag from `gpws` (reactive windshear, `Gpws::windshear_warning`), TCAS RA (`crates/tcas`) and GPWS warning activity as `higher_priority_aural`.
3. Outputs to write: `L:A380X_PWS_STATUS` (0 off, 1 standby, 2 active, 3 fail), `L:A380X_PWS_SEVERITY` (0/1/2 after inhibits), `L:A380X_PWS_ICON_{AZ,RANGE,HALFWIDTH,SEVERITY}` (icon drawn by the native ND: `a380_displays` ND screen as a red/amber chevron arc over the WXR underlay, and by the PFD as the W/S AHEAD message, red or amber), `Aural` events to the native sound path (`fbw/snd/*` custom aurals "windshear ahead", "go around windshear ahead", "monitor radar display"; reuse the `gpws` sound-event route), and a `WS AHEAD` FWS line only if the FWS crate wants a master caution (the real aircraft does not give one for the caution).
4. Turbulence: in `physics::classify`, replace `cell.turbulence >= 0.5` by the scanner's returns (or call `turb::sigma_v` per cell with `radial_neighbours` from the neighbouring polar cells already sampled); feed `turb_off`/mode from `A380X_WXR_TURB_OFF` and `A380X_WXR_MODE_TURB` (new), `turb_auto` from the SURV page; magenta returns go into the existing point store as `Level::Magenta`.
5. VD: new `NATIVE_IMAGE` ids `WXR_VD_L`/`WXR_VD_R` from `wxr::native_image` backed by `VdScanner` (budget about 24 per tick while "ON VD" is set and the VD is shown; bearing from the VD path or `A32NX_VD_*` track; range and altitude scale from the VD L:vars); drawn by the native VD behind the terrain/profile layer, using `VdImage::rgba`. Inop flag stays on `a32nx_aesu_wxr_failed_*`.
6. SURV page (`crates/mfd_pages_d/src/surv.rs`): bind `pred_ws_auto`, `turb_auto`, `wxr_on_vd` to L:vars read by items 2 to 5 (the controls exist, the variables do not).

## Not covered

- Terrain masking of the PWS beam (the scanner floors the beam at 15 m above ground derived from RA; a terrain probe hook per cell is possible).
- Real clutter-based dry microburst physics; the sampler's wind field stands in for Doppler.
- Exact figures per radar make; the config exposes every threshold.

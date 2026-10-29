# TAWS aurals, obstacle database and positional warning aurals

Scope: the four TAWS look-ahead aurals that `crates/gpws/src/taws.rs` asks for, the obstacle data behind its obstacle check, and how warning aurals (GPWS, TCAS, FWS) could be placed in the flight deck through Steam Audio. Numbers marked GENERIC are our own choices, not from FlyByWire or a public source.

## 1. TAWS aurals

`crates/gpws/src/sound.rs` names them `aural_terrain_ahead`, `aural_terrain_ahead_pull_up`, `aural_obstacle_ahead`, `aural_obstacle_ahead_pull_up`. None of them is in FlyByWire's Wwise banks (`sound.xml` has no such event), so today they fall through `play_instrument_sound` to nothing.

What the aircraft's sound sources hold:

| Source | Where | Words found | Shipped in releases |
| --- | --- | --- | --- |
| FlyByWire Wwise banks | `plugins/fbw_a380_systems/sound/*.PCK`, from the user's own MSFS package | "pull up", "too low terrain", "sink rate", no "ahead", no "obstacle" | No (`package_release.py` excludes `*.pck`) |
| A380X_SoundMod voice pack | `plugins/fbw_a380_systems/sound/voices/*.wav`, read by `src/sound/voice_pack.rs` | `co_terr` (terrain terrain), `co_terrahead` (terrain ahead), `co_tapull` (terrain ahead pull up), `co_pull` (pull up), `co_tooterr`; no "obstacle" | No (`package_release.py` excludes `/voices/`); no licence text for it anywhere in the repo |
| x_A380 FMOD pack (FTSiM+ demo) | `fmod/*.bank` | `/Warning/Gpws`, `GpwsB`, `GpwsC`, `Pull`, `Sink`, `DSink`, `TTerr`; no "obstacle" or "ahead" string in any bank | No; the installer asks the user for the free demo zip |

`tools/taws_sounds/build.py <voices dir> <out dir>` builds the aurals from the voice pack: a phrase the pack holds whole is used as recorded (trimmed, levelled to the median peak of the `co_*` callouts), else it is spliced from words (60 ms gap and 8 ms fades, GENERIC). The word boundary inside `co_terrahead` is found at the energy minimum in the middle of the phrase (0.59 s on the current file; `--cut co_terrahead=SECONDS` overrides). `--splice-only` builds every aural by splicing, for comparison.

Output in `E:/fbw-build/taws-sounds` (not installed):
- `aural_terrain_ahead.wav` 0.96 s, `co_terrahead` whole.
- `aural_terrain_ahead_pull_up.wav` 1.97 s, `co_tapull` whole.
- `spliced/`: the same two spliced from `co_terr` + "ahead" from `co_terrahead` (+ `co_pull`), 1.08 s and 1.74 s.
- `manifest.json`: sources per output and the words still missing.

The two obstacle aurals are not built: no pack has the word "obstacle". The tool builds them once a single-word recording is passed with `--obstacle WAV` (it then splices it with "ahead" and "pull up" from the same voice pack); the code path was checked with a stand-in word.

To play them, `voice_pack::MAPPING` needs four rows mapping the `aural_*` names to the generated file stems, and the generated WAVs need to sit in `sound/voices`. Because the voice pack itself is not shipped, these outputs are not shippable either; they are for the user's own install.

Open: a source for "obstacle". Options: a voice pack that has it, or recording it with the same Piper voices the first officer uses (would not match the callout voice).

## 2. Obstacle database

`tools/obstacles/build.py <out> --faa --osm NAME=S,W,N,E ...`:
- FAA Digital Obstacle File `DAILY_DOF_DAT.ZIP` (US Government work, public domain). Each record gives AGL and AMSL height; records whose action is D (dismantled) are dropped.
- OpenStreetMap through Overpass (three public mirrors tried in turn): `man_made=mast|tower|chimney|communications_tower` and `power=tower`, all with a `height` tag, kept at 60 m and above (`MIN_OSM_HEIGHT_M`). OSM gives no base elevation unless `ele` is tagged, so most OSM obstacles carry AGL only. ODbL: releases must credit "(c) OpenStreetMap contributors" and offer the derived database under ODbL.
- OSM obstacles within 150 m of an FAA one are dropped (GENERIC).
- Raw downloads stay in `<out>/raw`; `--offline` rebuilds from them, a run without it updates them. `manifest.json` records currency date, counts, licences.

This run: full FAA file (currency 10/15/26, 658,802 obstacles) plus OSM test boxes around Sydney (7 kept of 80 fetched) and Dubai (4 of 6). `E:/fbw-build/obstacles/obstacles.bin` is 5.3 MB, 1,864 one-degree tiles.

Format `OBS1`, little-endian: 16-byte header (`OBS1`, version u16 = 1, reserved u16, tile count u32, record count u32); tile index sorted by key (`(lat_tile + 90) * 360 + lon_tile + 180`, first record u32, count u32); 8-byte records (lat and lon within the tile in 1/65536 degree, about 1.7 m; AGL ft u16; AMSL ft i16, -32768 unknown).

`crates/gpws/src/obstacles.rs` reads it: `ObstacleDb::load/from_bytes` validates the header and index; `within(lat, lon, radius_nm, out)` returns records inside the circle, across the antimeridian and near the poles. `ObstacleDb` implements `taws::ObstacleSource` with only the obstacles whose AMSL is known; `WithTerrain { db, terrain }` also fills AGL-only obstacles from a `TerrainSampler` (base + AGL). Tests: `crates/gpws/tests/obstacles.rs` on `tests/fixtures/obstacles` (synthetic FAA and OSM rows built by the tool with `--offline`).

Still to do: worldwide OSM fetch in boxes (Overpass rate limits, run overnight), loading `obstacles.bin` in the plugin and passing `WithTerrain` with the terrain probe to `Taws::update`, an EFB/installer "update obstacle data" step, ODbL notice in THIRD_PARTY_NOTICES.

## 3. Audio path audit

- Steam Audio is present: `src/binaural.rs` loads `steamaudio/phonon.dll` + `phonon_fmod.dll` (Apache-2.0) into X-Plane's own FMOD (`XPLMGetFMODStudio`), and for every FMOD Studio event of the sound pack that has an FMOD Pan DSP it bypasses the pan and inserts the Steam Audio spatializer on the event's channel group, feeding the event's world position. Enabled by the `binauralAudio` setting (default on). It only touches sound-pack (FMOD Studio) events.
- FlyByWire's own sounds (Wwise media decoded by the plugin) and the voice pack play through `XPLMPlayPCMOnBus`, which returns an FMOD Core channel:
  - sound.xml `SimVarSounds` with a `NodeName` go through `src/sound/voices.rs`: position from `sound_nodes.txt` turned to world space with the aircraft pose, `XPLMSetAudioPosition` + fade distance 2-60 m on the interior bus. That is FMOD's own 3D panning, not Steam Audio.
  - `AvionicSounds` (requested by name) play flat on the interior bus (`Sound::play_pcm`). Every warning aural is one of these: the native GPWS/TAWS (`native_shadow.rs` `play_sound` -> `play_instrument_sound`), native TCAS (`crates/tcas` `SoundCall` names such as `traffic_traffic`, `climb_climb`), FBW's JS FWS sound manager and chimes (`js_bridge.rs` -> `play_instrument_sound`).
- Speaker position available today: `sound_nodes.txt` has `SPEAKER_OHVD_LP` at (-0.119, 3.673, -31.808) m in aircraft coordinates, the node FlyByWire's sound.xml uses for flight-deck electrical sounds. No separate left/right loudspeaker nodes exist in the model.

## 4. Plan: positional warning aurals through Steam Audio

1. Placement table in `src/sound/voices.rs` or a new `src/sound/warnings.rs`: aural name -> speaker. Captain loudspeaker at `SPEAKER_OHVD_LP` + (-0.55, -0.9, 0) m and F/O at + (+0.55, -0.9, 0) m (GENERIC until a cockpit-model node is found; check the converted cockpit OBJ for speaker grilles first). Warning aurals play from both speakers at once, as two channels, each at -3 dB (GENERIC).
2. Route by name: in `Sound::update`, when an instrument-queue name is a warning aural (GPWS/TAWS `aural_*` and `co_*`, TCAS names from `tcas::consts`, FWS chime and voice names that FBW's FWS sound manager requests), start it as a `voices::Voice` with `position = Some(speaker)`, `exterior = false`, `view = InsideOnly`, instead of the flat `play_or_queue`. The existing `voices` update keeps the world position current each frame.
3. Steam Audio on Core channels: extend `binaural.rs` with `FMOD_Channel_AddDSP`/`FMOD_Channel_RemoveDSP` and `FMOD_Channel_GetSystemObject` (same symbol-loading macro). For each warning channel create one spatializer DSP (`create_dsp` with the plugin handle already found), set its source position parameter each frame from the voice's world position, and disable FMOD's own 3D pan on that channel (`FMOD_Channel_SetMode` to 2D) so the sound is panned once. Release the DSP when the channel ends. Falls back to the current `XPLMSetAudioPosition` path when Steam Audio did not start or `binauralAudio` is off.
4. Priority and ducking stay where they are: `gpws::sound::SoundManager` already serialises GPWS aurals, `crates/tcas` its callouts; positional playback changes only where the sound comes from.
5. Tests: a unit test that every warning name maps to a speaker; a mocked DSP-call test for attach/release order; in sim, a log line `sound: warning <name> at <speaker>` and a headphone check that the captain's side is louder in the left ear with the head turned right.

Order of work: 1-2 (no new FFI, uses XPLM 3D already in place), then 3 behind the existing `binauralAudio` setting.

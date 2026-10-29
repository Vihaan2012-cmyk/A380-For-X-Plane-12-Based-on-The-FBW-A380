# DATA half: implementation specification

This is the complete specification for `xplane-js/data`. Implement it from this
file, `spec/data-api.d.ts` (the exact public declarations) and
`spec/API-v1.md` (the overview). Nothing else is needed.

## 0. Deliverables

All under `data/`:

| file | what |
|---|---|
| `data/src/*.ts` | the implementation, TypeScript, no dependencies |
| `data/src/runtime.ts` | entry of the page runtime: builds every export once and stores them on `globalThis.xplaneJsData` (an object whose keys are the export names of `spec/data-api.d.ts`) |
| `data/src/index.ts` | the module that instruments import as `xplane-js/data`: re-exports every name of `spec/data-api.d.ts` from `globalThis.xplaneJsData` (so each page has exactly one runtime, however many bundles it loads) |
| `data/tsconfig.json` | strict, target ES2017, lib ES2020 + DOM |
| `data/test/*.ts` | unit tests runnable with `node --test` against a fake host (section 2) |

The page runtime is bundled to one IIFE file by the build (not your job). It
must not touch the page before `DOMContentLoaded` except to install
`globalThis.xplaneJsData`.

Style: no comments except licence header lines; no `any` in public
signatures beyond those in `data-api.d.ts`.

## 1. Words used here

- **host**: the native object `globalThis.__xphfbw` installed before the page
  runs (section 2). If it is missing, every read returns 0 / `''`, writes do
  nothing, calls never resolve, and everything else still works.
- **frame**: one `requestAnimationFrame` callback of the page.
- **variable name**: any string. The host resolves it; this library never
  parses a name except where a section below says so.
- **unit**: a string such as `'number'`, `'bool'`, `'feet'`, `'string'`.

## 2. The host object

```ts
interface Host {
  readonly view: number;
  readonly screen: string;
  readonly aircraftDir: string;
  snapshot(): void;                                   // freeze this frame's values
  getVar(name: string, unit: string): number;
  setVar(name: string, unit: string, value: number): void;
  getString(name: string): string;                    // last known text value, '' if none
  setString(name: string, value: string): void;
  sendEvent(name: string, values: number[]): void;
  call(name: string, argsJson: string): Promise<string>; // resolves with JSON text, rejects with an error text
  poll(): Array<[kind: 'h' | 'provider' | 'game' | 'string', name: string, payload: string]>;
  gameString(name: string): string;                   // synchronous; '' when unknown
  storedData(op: 'get' | 'set' | 'delete' | 'search', key: string, value: string): string;
  readFile(path: string): string | null;
  log(level: 0 | 1 | 2, text: string): void;          // 0 info, 1 warning, 2 error
  magVar(lat: number, lon: number): number;            // degrees, east positive
  loaded(ok: boolean, text: string): void;
}
```

Rules:

1. Call `snapshot()` once at the start of every frame, before anything else
   the runtime does in that frame.
2. `poll()` must be called exactly once per frame (after `snapshot()`), and
   every entry it returns must be handled in order (section 6).
3. `call` resolution text is JSON; parse it (`'null'` → `null`). If parsing
   fails, resolve with the raw text.

## 3. `Vars`

`Vars.read(name, unit)`:

| unit (case-insensitive) | result |
|---|---|
| `'string'` | `host.getString(name)` |
| `'latlonalt'` | `new GeoPos3(host.getVar(name + '_LAT', 'degrees'), host.getVar(name + '_LON', 'degrees'), host.getVar(name + '_ALT', 'feet'))` |
| `'xyz'` | `{ x: host.getVar(name + '_X', 'number'), y: host.getVar(name + '_Y', 'number'), z: host.getVar(name + '_Z', 'number') }` as a `Vec3Data` |
| anything else | `host.getVar(name, unit)` (a number; NaN passes through) |

`Vars.write(name, unit, value)` returns a promise resolved (with `undefined`)
after the write was handed to the host:

| case | action |
|---|---|
| name starts with `K:` or `H:` | `host.sendEvent(name, [Number(value) || 0])` |
| unit `'string'` | `host.setString(name, String(value))` |
| any other | `host.setVar(name, unit, typeof value === 'boolean' ? (value ? 1 : 0) : Number(value))` |

- `Vars.readNow(name, unit)` is `read`.
- `Vars.readEnv(name, unit)` is `read`.
- `Vars.readHost(name, unit, p1?, p2?)`:
  - `'string'` → `host.gameString(name)`;
  - `'xyz'` → as `read` with `'xyz'`;
  - otherwise → `host.getVar(name, unit)`. Parameters are ignored.
- `Vars.register(name, unit)` returns a small integer id, the same id for the
  same `(name, unit)` pair, starting at 0.
  - `readById(id)` is `read(name, unit)` of that pair.
  - `readTextById(id)` is `host.getString(name)`.
  - An unknown id reads 0 / `''`.
- `Vars.VarBatch(countName, idName)` stores both names; `add(name, unit)`
  appends a column.
  - `Vars.readBatch(batch, callback)` calls `callback([])` asynchronously,
    on the next frame. There is no batch source in X-Plane.
- `Vars.watch(name, unit, listener)`: from the next frame on, read
  `(name, unit)` once per frame (after the snapshot) and call `listener(value)`
  the first time and whenever the value is not `===` the previous one (NaN
  equals NaN). It returns `{ dispose() }`.

## 4. `Datarefs`, `Commands`, `Units`

- `Datarefs.get(path, index?)` = `host.getVar(index === undefined ? path : path + '[' + index + ']', 'raw')`.
- `Datarefs.set(path, value, index?)` = `host.setVar(<same name>, 'raw', value)`.
- `Datarefs.getText(path)` = `host.getString(path)`.
- `Datarefs.watch(path, listener, index?)` works like `Vars.watch` on that
  name with unit `'raw'`.
- `Commands.once(path)` = `host.sendEvent('CMD:' + path, [0])`, `begin` → `[1]`, `end` → `[2]`.
- `Units.convert(value, from, to)`: a table of linear units by dimension.
  - Names are matched case-insensitively, after trimming, with plural
    `s`/`es` forms accepted. The table:

| dimension | units (factor to the base unit, base first) |
|---|---|
| length | meter/meters/m 1, foot/feet/ft 0.3048, nautical mile/nm 1852, kilometer/km 1000, statute mile/mile 1609.344, inch/in 0.0254, centimeter/cm 0.01, millimeter/mm 0.001 |
| speed | meter per second/m/s 1, knot/knots/kt 0.514444..., kilometer per hour/km/h 1/3.6, feet per second 0.3048, feet per minute/fpm 0.00508, mach is not linear: excluded |
| angle | radian 1, degree/degrees/deg π/180, degree latitude, degree longitude (π/180) |
| mass | kilogram/kg 1, pound/lbs/lb 0.45359237, tonne 1000, slug 14.5939029 |
| pressure | pascal 1, hectopascal/hpa 100, millibar/mbar 100, inches of mercury/inhg 3386.389, psi 6894.757 |
| time | second/seconds/sec 1, minute 60, hour 3600, millisecond 0.001 |
| temperature | celsius (offset): kelvin = c + 273.15; fahrenheit = c × 1.8 + 32; rankine = (c + 273.15) × 1.8 |
| ratio | number 1, percent 0.01, percent over 100 1, part 1 |
| volume | liter 1, gallon/gallons 3.785411784, cubic meter 1000 |
| flow | gallons per hour/gph 1, pounds per hour 1 (separate dimensions; each converts only to itself) |

  - Unknown units or a dimension mismatch give `NaN`.
  - `Units.dimension(unit)` returns the dimension name or `undefined`.

## 5. Sessions, settings, text, geometry, small types

- `sessionPhase()` always returns `SessionPhase.flying` (3).
- `loadStored(key)` = `host.storedData('get', key, '')`, with `''` mapped to `null`.
- `saveStored(key, value)` = `host.storedData('set', key, String(value))`; returns `undefined`.
- `Text.localize(key)` returns `key` unchanged (`null` stays `null`).
- `Text.newGuid()` returns a random RFC 4122 version 4 UUID string, lower case.
- `Declination.at(lat, lon)` = `host.magVar(lat, lon)` (0 without a host).
- `Noop.Void` is `() => {}`; `Noop.Boolean` is `(_b: boolean) => {}`.
- `SoundId`: `constructor(value = '')` sets `str = value` and `value = 0`.
- `GeoPosData(lat = 0, long = 0)` stores both. `GeoPos(latOrObject, long)`: when the first argument is an object, copy its `lat`/`long`.
- `GeoPos3(latOrObject, long, alt = 0)`: same; `alt` from the object when present, else 0. `toLatLong()` returns a `GeoPos` with the same `lat`/`long`.
- `Vec3Data`: plain `x`, `y`, `z` fields, all 0.
- `AircraftModel`, `SessionPhase`, `KeyCodes`: enums with exactly the values in `data-api.d.ts` (`KeyCodes` values are the standard DOM `keyCode` numbers).
- `InputButtons.MENU_A` = `'MENU_VALID'`.
- `MapElement` is a custom element class (`HTMLElement`) whose methods do nothing; `configId` 0, `configs` `[]`.
- `MapConfig.load(path, cb)` calls `cb` asynchronously; `applyTo` does nothing.

`NavMath.Geo`: spherical earth, radius 3440.065 nautical miles; angles in
degrees; latitudes/longitudes as `lat`/`long` fields.

| member | definition |
|---|---|
| `DEG_TO_RAD` / `RAD_TO_DEG` | π/180 and 180/π |
| `wrapDegrees(a)` | `a` reduced into [0, 360) |
| `angleDelta(a, b)` | `b - a` reduced into (-180, 180] |
| `lerpDegrees(from, to, t)` | `wrapDegrees(from + angleDelta(from, to) * t)` |
| `distance(p, q)` | great-circle distance by the haversine formula, nautical miles |
| `bearing(p, q)` | initial great-circle course from `p` to `q`, true, [0, 360) |
| `destination(bearing, distance, lat, long)` | the point reached by following the great circle from (`lat`, `long`) on initial course `bearing` for `distance` nautical miles, as `GeoPos3` with `alt` 0, longitude normalised into [-180, 180] |
| `adfBcd32(f)` | `f` in kHz as BCD: each decimal digit of `round(f * 10)` in its own nibble, least significant digit in the lowest nibble (so 345.0 → `0x3450`) |

## 6. Events: `Bridge`, channels, relay

Every entry from `host.poll()`, in order:

| kind | handling |
|---|---|
| `'h'` | panel event `name`: every live instrument's `onPanelEvent([name])` (section 7), then `Bridge` listeners of `'panel.event'` with `(name)` |
| `'provider'` | `args = JSON.parse(payload)` (on failure `[payload]`); if `args` is not an array, `args = [args]`. Dispatch to `Bridge` listeners registered for `name`, each called with `...args`; then the channel rules below |
| `'game'` | nothing more (the host already cached the text) |
| `'string'` | nothing more |

- `Bridge.call(name, ...args)` = `host.call(name, JSON.stringify(args))`, parsed as in section 2 rule 3, except two names that refer to `Vars.register` ids and are handled here, never sent to the host:
  - `'var.set'` with `(id, value)`: `Vars.write(<name>, <unit>, value)` for the pair registered under `id` (nothing for an unknown id); the promise resolves with `undefined`.
  - `'var.set.text'` with `(id, text)`: `host.setString(<name>, String(text))` for that id; resolves with `undefined`.
  `Bridge.signal` with these names does the same.
- `Bridge.on(name, fn)` registers a listener and returns `{ clear() }`, which removes exactly that registration.
- `Bridge.off(name, fn)` removes every registration of `fn` for `name`.
- `Bridge.signal(name, ...args)` = `host.call(name, JSON.stringify(args))`; the result is ignored, and rejections are swallowed.

### Channels

`openChannel(name, onReady?)` returns a `Channel` and calls `onReady` once,
asynchronously (a resolved promise).

On a provider event named `'channel.event'` whose parsed array is
`[channelName, event, ...rest]`:

- if `event === 'channel.broadcasted'`, the real event name is `rest[0]` and
  its arguments are `rest.slice(1)`;
- otherwise the event is `event` with arguments `rest`.

Then every open channel named `channelName` calls its listeners for that
event with those arguments.

Channel methods:

- `on(event, fn)` / `off(event, fn?)`: `off` without `fn` removes every
  listener of that event.
- `call(event, ...args)` = `Bridge.call(event, ...args)`.
- `signal(event, ...args)` = `Bridge.signal(event, ...args)`.
- `broadcast(event, ...args)` = `Bridge.signal('channel.broadcast', <channel name>, event, ...args)`.
- `close()` stops dispatching to this channel object.

### Relay

`openRelay(onReady?)` returns `{ send(key, data), receive(key, fn) }` and
calls `onReady` once, asynchronously.

- `send(key, data)` = `Bridge.signal('relay.send', key, typeof data === 'string' ? data : JSON.stringify(data))`.
- `receive(key, fn)`: on a `'channel.event'` provider event whose parsed
  array is `['relay', key, data]`, call `fn(value)`, where `value` is
  `JSON.parse(data)` when `data` is a string that parses, and `data`
  unchanged otherwise.
- The sender never receives its own sends; the host already excludes them.

- `sendFlowEvent(name, ...args)` = `Bridge.signal(name, ...args)`.

## 7. The instrument host

### 7.1 Page format

The build writes each instrument's page as:

```html
<!doctype html>
<html>
<head>
<meta charset="utf-8">
<link rel="stylesheet" href="NAME.css">
</head>
<body>
<template id="TEMPLATE_ID">...instrument markup...</template>
<script src="NAME.js"></script>
</body>
</html>
```

The page runtime (this half) is already loaded when `NAME.js` runs: the
viewer injects it as the first script of `<head>`.

### 7.2 `defineInstrument(tag, type)`

1. Register `type` as the custom element `tag` (`customElements.define`;
   ignore an already-defined tag).
2. When the document is ready (immediately if `document.readyState !== 'loading'`,
   else on `DOMContentLoaded`), create one element of `tag` and append it to
   `document.body`.

### 7.3 `PanelInstrument` (extends `HTMLElement`)

The browser's `connectedCallback` of the element calls `this.onAttach()`,
inside a try/catch:

- on success: `host.loaded(true, '')`, then
  `Bridge.signal('instrument.ready', <instance guid>, <tag>)`, then start the
  frame loop;
- on an exception: `host.log(2, <message and stack>)` and
  `host.loaded(false, <message>)`.

Base implementations (subclasses call them through `super`):

- `onAttach()`:
  1. find `<template id=this.templateId>` and append a deep clone of its
     content to the element (once only);
  2. call `this.readPanelConfig()`;
  3. call `this.onPrepare()`.
- `onPrepare()`: nothing.
- `readPanelConfig()`: `this.panelConfig` = the result of
  `DOMParser.parseFromString(host.readFile('/VFS/panel/panel.xml') ?? '<PanelInfo/>', 'text/xml')`.
- `templateId`: `''` in the base (subclasses override the getter).
- `interactive`: `true`.
- `screenIndex`: the integer value of the `Index` parameter of
  `location.search` (case-insensitive key), default 1.
- `frameDelta`: see 7.4.
- `onFrame()`, `onPanelEvent(args)`, `onSessionChange(from, to)`,
  `onFlightBegin()`, `onSoundDone(id)`, `onPowered()`, `onShutdown()`: do
  nothing.
- `sessionState()`: `sessionPhase()`.

### 7.4 Frame callbacks

Two kinds of `requestAnimationFrame` chains, each of which requests its next
frame as the first thing it does in every callback:

1. **The page chain**, started when the runtime is installed (before any
   instrument script runs). Each frame it:
   - calls `host.snapshot()`;
   - handles `host.poll()` (section 6);
   - updates every watch (section 3).
2. **One chain per instrument**, requested in `connectedCallback` immediately
   before `onAttach()` is called. Chains requested while `onAttach()` runs
   (by the instrument's own code) therefore run after it within each frame.
   Each callback, once the instrument has attached successfully:
   - sets its `frameDelta`: milliseconds since this chain's previous
     callback, or, the first time, since the chain was requested (the
     clock read with `performance.now()` at the request);
   - on its first callback only, calls
     `onSessionChange(SessionPhase.briefing, SessionPhase.flying)` and then
     `onFlightBegin()`;
   - calls `onFrame()`.

   An exception in an instrument is logged with `host.log(2, ...)`, and its
   chain keeps running. If `onAttach()` throws, the chain stops.

Watches, `Bridge.on`, channels and relays need no loop of their own: the page
chain always runs.

## 7.5 `PlaneState`

Each getter reads through `Vars.read` with these names and units:

| getter | reads | result |
|---|---|---|
| `altitude()` | `fbw/INDICATED_ALTITUDE`, `feet` | the number |
| `heightAboveGround()` | `fbw/PLANE_ALT_ABOVE_GROUND_MINUS_CG`, `feet` | the number |
| `outsideAirTemp()` | `sim/weather/aircraft/temperature_ambient_deg_c`, `celsius` | the number |
| `groundSpeed()` | `fbw/GPS_GROUND_SPEED`, `knots` | the number |
| `indicatedSpeed()` | `sim/flightmodel/position/indicated_airspeed`, `knots` | the number |
| `verticalSpeed()` | `fbw/VERTICAL_SPEED`, `feet per minute` | the number |
| `onGround()` | `fbw/SIM_ON_GROUND`, `bool` | value `!== 0` |
| `baroSetting(units = 'inches of mercury')` | `fbw/KOHLSMAN_SETTING_HG_1`, `units` | the number |
| `baroMode(_aircraft)` | `fbw/KOHLSMAN_SETTING_STD_1`, `bool` | `'STD'` when `!== 0`, else `'QNH'` |
| `baroUnits()` | `L:XMLVAR_Baro_Selector_HPA_1`, `bool` | `'millibar'` when `!== 0`, else `'inches of mercury'` |
| `apSpeedTarget()` | `fbw/AUTOPILOT_AIRSPEED_HOLD_VAR`, `knots` | the number |
| `apSpeedSelected()` | `fbw/AUTOPILOT_SPEED_SLOT_INDEX`, `number` | value `=== 1` |
| `apSpeedManaged()` | `fbw/AUTOPILOT_SPEED_SLOT_INDEX`, `number` | value `=== 2` |
| `apMachTarget()` | `fbw/AUTOPILOT_MACH_HOLD_VAR`, `number` | the number |
| `apMachMode()` | `fbw/AUTOPILOT_MANAGED_SPEED_IN_MACH`, `bool` | value `!== 0` |
| `apHeadingTarget(radians = false)` | `fbw/AUTOPILOT_HEADING_LOCK_DIR`, `radians` or `degrees` | the number |
| `apAltitudeTarget(units = 'feet')` | `fbw/AUTOPILOT_ALTITUDE_LOCK_VAR_3`, `units` | the number |
| `apAltitudeManaged()` | `fbw/AUTOPILOT_ALTITUDE_SLOT_INDEX`, `number` | value `=== 2` |
| `greenDotSpeed()` | `L:A32NX_SPEEDS_GD`, `number` | the number |

## 8. Errors

`window.addEventListener('error')` and `('unhandledrejection')`: forward
the message (and stack when present) to `host.log(2, text)`.

## 9. Tests

`node --test` must cover, with a fake host that records every call:

- every table row of sections 3–6 and 7.4;
- the channel and relay rules;
- the unit table, including the temperature offsets and NaN cases;
- the geometry rows against values computed independently in the test from
  the formulas above.

# JS Library for X-Plane, API v1

A JavaScript library for glass-cockpit instruments that run in a browser view
inside X-Plane 12 (for example Chromium hosted by a plugin). Any aircraft can
use it. It has two halves:

| module | half | what it is |
|---|---|---|
| `xplane-js/data` | DATA | Access to the simulator: X-Plane datarefs and commands, the variables a plugin holds, key events, named calls into the plugin, persistent settings, units, and the instrument host (page lifecycle). |
| `xplane-js/ui` | UI | Reactive values, an event bus, components with a JSX renderer, an instrument board, and small maths, formatting and navigation helpers. |

`xplane-js/ui` depends on `xplane-js/data`; `xplane-js/data` depends on
nothing but the page and the host bridge.

## 1. DATA half (`xplane-js/data`)

### 1.1 The model

X-Plane exposes state as **datarefs** (typed values named by a path such as
`sim/flightmodel/position/latitude`) and actions as **commands** (named by a
path, with begin/end phases). A plugin can publish its own datarefs and
commands and can keep variables of its own. The library is built on these
three ideas:

- **Variable names** are what the plugin knows a value by:
  - an X-Plane dataref path: `sim/...`;
  - a dataref the plugin publishes: `<prefix>/NAME` (the A380 plugin uses `fbw/NAME`);
  - an aircraft variable the plugin keeps: `L:NAME`.
  - An optional one-letter category in front (`A:` or `E:`) is accepted and ignored by the plugin; it only records where a name came from.
  - Names ending in `_<n>` address element `n` of an indexed family.
- **Units are always explicit.** Every read and write names the unit it wants
  (`'feet'`, `'knots'`, `'bool'`, `'number'`, ...). The host converts between
  the unit a variable is stored in and the unit asked for. Pure conversions
  are also available in JavaScript (`Units`).
- **Change subscriptions** are frame based: the host samples every watched
  value once per frame and calls the listener when it changed.

### 1.2 Variables: `Vars`

```ts
namespace Vars {
  read(name: string, unit: UnitName): any;                 // number, string, or GeoPos3 for 'latlonalt'
  write(name: string, unit: UnitName, value: any): Promise<void>;
  readNow(name: string, unit: UnitName): any;              // same value as read, no caching layer
  register(name: string, unit: UnitName): number;          // a numeric id for repeated reads
  readById(id: number): any;
  readTextById(id: number): string;
  readHost(name: string, unit: UnitName, param?: any): any; // values the simulator host computes (database dates, camera, speed conversions)
  readEnv(name: string, unit: UnitName): any;               // environment values (clock, simulation rate)
  readBatch(batch: VarBatch, callback: (rows: any[]) => void): void;
  class VarBatch { constructor(countName: string, idName: string); add(name: string, unit: UnitName): void; }
  type UnitName = string;
  watch(name: string, unit: UnitName, listener: (value: any) => void): { dispose(): void };
}
```

`write` with a name starting `K:` fires a key event (see 1.5). A `string`
unit reads or writes text.

### 1.3 Datarefs and commands

```ts
namespace Datarefs {
  get(path: string, index?: number): number;
  getText(path: string): string;
  set(path: string, value: number, index?: number): void;
  watch(path: string, listener: (value: number) => void, index?: number): { dispose(): void };
}
namespace Commands {
  once(path: string): void;
  begin(path: string): void;
  end(path: string): void;
}
```

Datarefs read through this namespace are raw: the value in the dataref's own
units. Convert explicitly with `Units`.

### 1.4 Units

```ts
namespace Units {
  convert(value: number, from: UnitName, to: UnitName): number; // NaN when the units are not of one dimension
  dimension(unit: UnitName): string | undefined;
}
```

### 1.5 Calls, events and channels: `Bridge`, `openChannel`

```ts
namespace Bridge {
  call(name: string, ...args: any[]): Promise<any>;        // named request answered by the plugin
  on(name: string, listener: (...args: any[]) => void): { clear(): void };
  off(name: string, listener: (...args: any[]) => void): void;
  signal(name: string, ...args: any[]): void;              // one-way notice to the host
}
function openChannel(name: string, onReady?: () => void): Channels.Channel;
namespace Channels {
  class Channel {
    on(name: string, listener: (...args: any[]) => void): void;
    off(name: string, listener?: (...args: any[]) => void): void;
    call(name: string, ...args: any[]): Promise<any>;
    signal(name: string, ...args: any[]): void;
    broadcast(name: string, ...args: any[]): void;          // to every view that opened the channel
    close(): void;
  }
}
function sendFlowEvent(name: string, ...args: any[]): void;
```

Named calls and channel events are the plugin's protocol; the library only
carries them.

### 1.6 Settings and text

```ts
function loadStored(key: string): string | null;
function saveStored(key: string, value: string): void;
namespace Text { localize(text: string): string; newGuid(): string; }
```

### 1.7 Aircraft state helpers: `PlaneState`

Convenience readers over `Vars` for values instruments commonly need:
`altitude()`, `heightAboveGround()`, `outsideAirTemp()`, `groundSpeed()`,
`indicatedSpeed()`, `verticalSpeed()`, `onGround()`, `baroSetting(unit)`,
`baroMode()`, `baroUnits()`, `greenDotSpeed()`, and autopilot targets
`apSpeedTarget()`, `apSpeedSelected()`, `apSpeedManaged()`, `apMachTarget()`,
`apMachMode()`, `apHeadingTarget()`, `apAltitudeTarget()`,
`apAltitudeManaged()`. Each is one or two `Vars.read` calls; the exact names
are in the DATA specification.

### 1.8 Geometry: `NavMath.Geo`, `Declination`, positions

```ts
namespace NavMath.Geo {
  DEG_TO_RAD: number; RAD_TO_DEG: number;
  bearing(from: GeoPosData, to: GeoPosData): number;          // initial great-circle bearing, degrees true
  distance(from: GeoPosData, to: GeoPosData): number;         // great-circle distance, nautical miles
  destination(from: GeoPosData, bearing: number, distance: number): GeoPos;
  angleDelta(a: number, b: number): number;
  lerpDegrees(a: number, b: number, t: number): number;
  wrapDegrees(a: number): number;
  adfBcd32(frequency: number): number;
}
namespace Declination { at(lat: number, lon: number): number; }
class GeoPos { lat: number; long: number; }
class GeoPos3 extends GeoPos { alt: number; }
interface GeoPosData { lat: number; long: number; }
interface Vec3Data { x: number; y: number; z: number; }
```

### 1.9 The instrument host: `PanelInstrument`, `defineInstrument`

An instrument is a class registered under a tag. The host creates one
instance per view, then drives it:

```ts
abstract class PanelInstrument implements PanelTemplate {
  readonly templateId: string;          // id of the page's template element
  readonly interactive: boolean;
  readonly screenIndex: number;         // from the view URL (?Index=n), 1-based, default 1
  readonly frameDelta: number;          // milliseconds since the previous frame
  readonly panelConfig: Document | null;
  onAttach(): void;                     // once, after the instance is in the page
  onPrepare(): void;
  onFrame(): void;                      // every frame
  onPanelEvent(args: string[]): void;   // cockpit interaction (button presses)
  onSessionChange(from: SessionPhase, to: SessionPhase): void;
  onFlightBegin(): void;
  onSoundDone(id: SoundId): void;
  onPowered(): void;
  onShutdown(): void;
  sessionState(): SessionPhase;
  readPanelConfig(): void;
}
function defineInstrument(tag: string, type: new () => PanelInstrument): void;
enum SessionPhase { loading, briefing, flying, ... }
```

### 1.10 Small types

`Noop` (shared no-op callbacks), `SoundId`, `KeyCodes`, `InputButtons`,
`AircraftModel`, `AirportRecord` / `FacilityRecord` / `RunwayRecord` (records
returned by the plugin's navigation calls), `MapElement` / `MapConfig` (a map
element contract; the X-Plane host renders no map of its own).

## 2. UI half (`xplane-js/ui`)

Layout: `values/` (reactive values), `collections/`, `bus/`, `components/`
(`view.ts`, `instrument.ts`), `maths/`, `services/`.

### 2.1 Reactive values

| name | role |
|---|---|
| `Gettable<T>` / `Settable<T>` | `get()` / `set(v)` |
| `Readable<T>` | `get()`, `watch(listener, notifyNow?, held?) -> Bond`, `derive(fn, equals?) -> DerivedReadable`, `forward(target, fn?, held?) -> Bond` |
| `Writable<T>` | `Readable` + `Settable` |
| `Bond` | `isLive`, `isHeld`, `hold()`, `release(notifyNow?)`, `dispose()` |
| `Lifespan` | `adopt(bond)`: disposes adopted links together |
| `ReadableBase<T>` | base class for readables; `emit()` notifies watchers |
| `Signal<T>` | writable value: `Signal.of(v, equals?)`, `get`, `set`, `emit` |
| `Derived<I, T>` | computed from inputs: `Derived.from(fn, ...inputs)`, `Derived.fromInputs(...)`, `scopedTo(lifespan)` |
| `BusSignal<T>` / `BusValue<T>` | a value fed by a bus `Stream`: `from(stream, initial)`, `rebind(stream)` |
| `Logic` | `not()`, `and()`, `or()`, `nor()` mapping functions |
| `Readables` | `isReadable`, `isWritable`, `toReadable` |
| `Emitter<S, E>` | `listen(handler)`, `emit(sender, data)` |
| `Holder<T>` | plain holder: `Holder.of(v)` |

Collections: `SignalList` (`of`, `snapshot`, `all`, `insert`, `removeAt`,
`set`, `clear`, `watch`; change kinds `ListChange`), `SignalSet`, `SignalMap`
(`lookup`, `assign`; `MapChange`), `SignalRecord`, `QuantitySignal`
(`inUnit`).

### 2.2 Event bus

| name | role |
|---|---|
| `Bus` | `inbox<E>()`, `outbox<E>()`, `listen(topic, fn)`, `listenAll(fn)`, `post(topic, data, sync?, cached?)` |
| `Inbox<E>` | `topic(name) -> Stream` |
| `Outbox<E>` | `post(name, data, sync?, cached?)` |
| `Stream<T>` | `listen(fn, held?) -> Bond`, `distinct()`, `distinctBy(amount)`, `rounded(precision)`, `limit(hz, immediate?)`, `after(ms)`, `sampled(n)` |
| `StreamSource<T>` | a `Stream` built from a subscribe function |
| `TopicFeeder<E>` | publisher base: `begin()`, `tick()`, `postTopic(...)` |
| `VarPublisher<E>` | publishes variables (`VarSpec`, `VarTopic` with `indexed`, `defaultIndex`, `transform`) per frame to watched topics |
| `PanelEventPublisher` | publishes cockpit interaction events (`dispatchPanelEvent`) |
| `KeyControl` | `connect(bus)`, `intercept(key, passThrough)`, `fire(key, bypass, v0?, v1?, v2?)` |
| `SimClock`, `SimClockFeeder`, `ClockTopics` | per-frame clock topics |
| `SessionState` | `get()`: readable session phase |
| `Pacer`, `IndexedTopic`, `TopicsByIndex`, `BoardFeeder` | typing and pacing helpers |

Cross-view sync: a `post` with `sync` set reaches every view through the
host's channel relay, batched once per frame.

### 2.3 Components and JSX

JSX factory: `UI.node`, fragment `UI.Group`.

| name | role |
|---|---|
| `UI` | `node`, `Group`, `mount(node, element)`, `ref()`, `remove(el)`, `splitClasses(s)`, `childNodes(...)` |
| `View<P>` | component base: `props`, `render()`, `afterRender(node)`, `afterMount(node)`, `dispose()` |
| `ScopedView<P>` | a `View` with a `scope` lifespan disposed with it |
| `Ref<T>` | `target`, `peek()` |
| `ViewNode`, `ViewProps`, `ViewChildren` | node and props types |

### 2.4 Board (instrument wiring)

`Board` (`attachFeeder`, `attachDevice`, `setup`, `tick`), `BoardDevice`
(`setup`, `tick`), `HostedInstrument<T>` (a `PanelInstrument` that forwards
the host lifecycle to a `HostedDevice` created by `createDevice()`).

### 2.5 Maths, formatting, services

`Maths` (`round`, `angleDiff`), `Bits` (`any`, `all`), `Arrays`, `Isa`
(`isaTemp`), `Smoother` (`push`, `latest`), `LookupTable` (`at`),
`Transforms`/`Translate3d` (`css`), `Vec2D` (`make`, `set`, `fromPolar`,
`angle`, `magnitude`), `GeoPoint`, `GeoPointLong`, `Dimension`, `Measure`,
`LinearMeasure`, `Quantity` (`amount`, `inUnit`), `UnitList`; `NumFormat`,
`TimeFormat`, `AngleFormat`, `CycleFormat` (all `make(...)`), `NavCycle`;
`Waits` (`delay`, `frames`, `until`, `forValue`), `Debouncer` (`arm`,
`cancel`, `waiting`), `Ids` (`random`), `SharedObject`/`SharedHandle`
(`isOwner`, `value`, `detached`), `Pool` (`take`, `give`), `IcaoCodes`,
`NavDatabase` (`cycles`), `AirportKinds`, charts (`Charts`, `ChartPanel`,
`ChartOrigin`, `ChartError`, `FaaChart`, `LidoChart`, `ChartInfo`,
`ChartList`, `ChartPageSet`, `ChartPicture`), `RunwayId`, `ComSlot`,
`MetarReport`.

### 2.6 Second naming style

Some code uses shorter names; both styles are exported and are the same
objects: `State` (= `Signal`), `ComputedState`, `StateBase`, `StateList`,
`BusState`, `Handle`, `MutableState`, `Responder`, `Lifetime`, `Pipe`,
`PipeSource`, `Exchange`, `TickTopics`, `Part`, `PartProps`, `PartNode`,
`Parts`, and the factories `makeState`, `makeComputed`, `makeStateList`,
`makeBusState`.

## 3. Using it from a FlyByWire build

FlyByWire's instrument sources are moved onto this API once, by
`tools/convert.ts`. The name mapping it uses is an internal file and is not
part of the library.

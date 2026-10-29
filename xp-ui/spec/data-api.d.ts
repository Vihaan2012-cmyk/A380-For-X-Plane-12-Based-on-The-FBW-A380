export type UnitName = string;
export type Listener = (...args: any[]) => void;

export declare namespace Vars {
  type UnitName = string;
  function read(name: string, unit: UnitName, source?: string): any;
  function write(name: string, unit: UnitName, value: any, source?: string): Promise<any>;
  function readHost(name: string, unit: UnitName, param1?: number, param2?: number): any;
  function readEnv(name: string, unit: UnitName): any;
  function register(name: any, unit: any, source?: any): number;
  function readNow(name: string, unit: UnitName, source?: string): any;
  function readById(id: number): any;
  function readTextById(id: number): any;
  function readBatch(batch: VarBatch, callback: (rows: any) => void, source?: string): void;
  function watch(name: string, unit: UnitName, listener: (value: any) => void): { dispose(): void };
  class VarBatch {
    constructor(countName: string, idName: string);
    add(name: string, unit: UnitName, type?: string): void;
  }
}

export declare namespace Datarefs {
  function get(path: string, index?: number): number;
  function getText(path: string): string;
  function set(path: string, value: number, index?: number): void;
  function watch(path: string, listener: (value: number) => void, index?: number): { dispose(): void };
}

export declare namespace Commands {
  function once(path: string): void;
  function begin(path: string): void;
  function end(path: string): void;
}

export declare namespace Units {
  function convert(value: number, from: UnitName, to: UnitName): number;
  function dimension(unit: UnitName): string | undefined;
}

export interface EventHandle {
  clear(): void;
}

export declare namespace Bridge {
  function call(name: string, ...args: any[]): Promise<any>;
  function on(name: string, listener: Listener, context?: any): EventHandle;
  function off(name: string, listener: Listener, context?: any): void;
  function signal(name: string, ...args: any[]): void;
}

export declare namespace Channels {
  class Channel {
    on(name: string, listener: Listener): void;
    off(name: string, listener?: Listener): void;
    call(name: string, ...args: any[]): Promise<any>;
    signal(name: string, ...args: any[]): void;
    broadcast(event: any, ...args: any[]): void;
    close: () => void;
  }
}

export declare function openChannel(name: string, onReady?: () => void, single?: boolean): Channels.Channel;

export interface Relay {
  send(key: string, data: any): void;
  receive(key: string, listener: (data: any) => void): void;
}
export declare function openRelay(onReady?: () => void): Relay;

export declare function sendFlowEvent(name: string, ...args: any[]): void;
export declare function loadStored(key: string): any;
export declare function saveStored(key: string, value: any): any;

export declare namespace Text {
  function localize(key: string): string | null;
  function newGuid(): string;
}

export declare namespace Declination {
  function at(lat: number, lon: number): number;
}

export declare enum SessionPhase {
  'mainmenu' = 0,
  'loading' = 1,
  'briefing' = 2,
  'flying' = 3,
}
export declare function sessionPhase(): SessionPhase;

export declare class SoundId {
  constructor(value?: string);
  str: string;
  value: number;
}

export declare class Noop {
  static Void: () => void;
  static Boolean: (result: boolean) => void;
}

export declare class GeoPosData {
  lat: number;
  long: number;
  constructor(lat?: number, long?: number);
}
export declare class GeoPos extends GeoPosData {
  constructor(lat?: number | { lat: number; long: number }, long?: number);
}
export declare class GeoPos3 implements GeoPos {
  lat: number;
  long: number;
  alt: number;
  constructor(lat?: number | { lat: number; long: number; alt?: number }, long?: number, alt?: number);
  toLatLong(): GeoPos;
}
export declare class Vec3Data {
  x: number;
  y: number;
  z: number;
}

export declare namespace NavMath {
  class Geo {
    static DEG_TO_RAD: number;
    static RAD_TO_DEG: number;
    static destination(bearing: number, distance: number, lat: number, long: number): GeoPos3;
    static wrapDegrees(a: number): number;
    static distance(from: GeoPosData, to: GeoPosData): number;
    static bearing(from: GeoPosData, to: GeoPosData): number;
    static angleDelta(a: number, b: number): number;
    static lerpDegrees(from: number, to: number, t: number): number;
    static adfBcd32(frequency: number): number;
  }
}

export declare enum AircraftModel {
  CJ4,
  A320,
  B747_8,
  AS01B,
  AS02A,
}

export declare namespace PlaneState {
  function altitude(): number;
  function heightAboveGround(forceRead?: boolean): number;
  function outsideAirTemp(): number | null;
  function apSpeedTarget(): number | null;
  function apSpeedManaged(): boolean;
  function apSpeedSelected(): boolean;
  function apAltitudeManaged(): boolean;
  function apAltitudeTarget(units?: string): number;
  function apMachTarget(): number | null;
  function apMachMode(): boolean | null;
  function apHeadingTarget(radians?: boolean): number | null;
  function greenDotSpeed(): number | null;
  function groundSpeed(): number | null;
  function indicatedSpeed(): number;
  function onGround(): boolean;
  function baroMode(aircraft: AircraftModel): 'QFE' | 'QNH' | 'STD' | '';
  function baroUnits(): 'inches of mercury' | 'millibar';
  function baroSetting(units?: string): number;
  function verticalSpeed(): number;
}

export interface PanelTemplate {
  readonly templateId: string;
}

export declare class PanelInstrument extends HTMLElement implements PanelTemplate {
  get templateId(): string;
  get interactive(): boolean;
  get screenIndex(): number;
  get frameDelta(): number;
  panelConfig: Document;
  onAttach(): void;
  protected onPrepare(): void;
  protected onFrame(): void;
  onPanelEvent(args: Array<string>): void;
  protected onSessionChange(from: SessionPhase, to: SessionPhase): void;
  protected onFlightBegin(): void;
  onSoundDone(id: SoundId): void;
  onPowered(): void;
  onShutdown(): void;
  sessionState(): SessionPhase;
  protected readPanelConfig(): void;
}

export declare function defineInstrument(tag: string, type: CustomElementConstructor): void;

export declare enum KeyCodes {
  KEY_CANCEL = 3,
  KEY_HELP = 6,
  KEY_BACK_SPACE = 8,
  KEY_TAB = 9,
  KEY_CLEAR = 12,
  KEY_RETURN = 13,
  KEY_ENTER = 13,
  KEY_SHIFT = 16,
  KEY_CONTROL = 17,
  KEY_ALT = 18,
  KEY_PAUSE = 19,
  KEY_CAPS_LOCK = 20,
  KEY_ESCAPE = 27,
  KEY_SPACE = 32,
  KEY_PAGE_UP = 33,
  KEY_PAGE_DOWN = 34,
  KEY_END = 35,
  KEY_HOME = 36,
  KEY_LEFT = 37,
  KEY_UP = 38,
  KEY_RIGHT = 39,
  KEY_DOWN = 40,
  KEY_DELETE = 46,
  KEY_0 = 48,
  KEY_9 = 57,
  KEY_A = 65,
  KEY_P = 80,
  KEY_Z = 90,
}

export declare namespace InputButtons {
  const MENU_A: string;
}

export interface FacilityRecord {
  icao: string;
  name: string;
  lat: number;
  lon: number;
  region: string;
  city: string;
}
export interface RunwayRecord {
  latitude: number;
  longitude: number;
  elevation: number;
  direction: number;
  designation: string;
  length: number;
  width: number;
  surface: number;
  lighting: number;
  designatorCharPrimary: number;
  designatorCharSecondary: number;
  primaryILSFrequency: any;
  secondaryILSFrequency: any;
  primaryElevation: number;
  primaryThresholdLength: number;
  secondaryElevation: number;
  secondaryThresholdLength: number;
}
export interface AirportRecord extends FacilityRecord {
  altitude: number;
  runways: RunwayRecord[];
}

export declare class MapElement extends HTMLElement {
  setMapId(id: any): void;
  setConfig(id: number): void;
  setParams(params: { lla: GeoPos3 | GeoPosData; radius: number }): void;
  setVisible(visible: boolean): void;
  configId: number;
  configs: any[];
}
export declare class MapConfig {
  load(path: string, callback: () => void): void;
  applyTo(map: any): void;
}

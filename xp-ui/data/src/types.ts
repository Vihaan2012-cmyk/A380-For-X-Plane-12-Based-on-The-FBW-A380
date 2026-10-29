export type UnitName = string;
export type Listener = (...args: any[]) => void;

export interface EventHandle {
  clear(): void;
}

export interface Relay {
  send(key: string, data: any): void;
  receive(key: string, listener: (data: any) => void): void;
}

export interface PanelTemplate {
  readonly templateId: string;
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

export type EnumObject<T extends Record<string, number>> = { -readonly [K in keyof T]: T[K] } & { [value: number]: string };

export function makeEnum<T extends Record<string, number>>(values: T): EnumObject<T> {
  const out: Record<string, string | number> = {};
  for (const key of Object.keys(values)) {
    const value = values[key];
    out[key] = value;
    out[String(value)] = key;
  }
  return out as unknown as EnumObject<T>;
}

export const SessionPhase = makeEnum({
  mainmenu: 0,
  loading: 1,
  briefing: 2,
  flying: 3,
} as const);
export type SessionPhase = 0 | 1 | 2 | 3;

export function sessionPhase(): SessionPhase {
  return SessionPhase.flying;
}

export const AircraftModel = makeEnum({
  CJ4: 0,
  A320: 1,
  B747_8: 2,
  AS01B: 3,
  AS02A: 4,
} as const);
export type AircraftModel = 0 | 1 | 2 | 3 | 4;

export const KeyCodes = makeEnum({
  KEY_CANCEL: 3,
  KEY_HELP: 6,
  KEY_BACK_SPACE: 8,
  KEY_TAB: 9,
  KEY_CLEAR: 12,
  KEY_RETURN: 13,
  KEY_ENTER: 13,
  KEY_SHIFT: 16,
  KEY_CONTROL: 17,
  KEY_ALT: 18,
  KEY_PAUSE: 19,
  KEY_CAPS_LOCK: 20,
  KEY_ESCAPE: 27,
  KEY_SPACE: 32,
  KEY_PAGE_UP: 33,
  KEY_PAGE_DOWN: 34,
  KEY_END: 35,
  KEY_HOME: 36,
  KEY_LEFT: 37,
  KEY_UP: 38,
  KEY_RIGHT: 39,
  KEY_DOWN: 40,
  KEY_DELETE: 46,
  KEY_0: 48,
  KEY_9: 57,
  KEY_A: 65,
  KEY_P: 80,
  KEY_Z: 90,
} as const);
export type KeyCodes = number;

export const InputButtons: { readonly MENU_A: string } = { MENU_A: 'MENU_VALID' };

export class SoundId {
  str: string;
  value: number;
  constructor(value: string = '') {
    this.str = value;
    this.value = 0;
  }
}

export class Noop {
  static Void: () => void = () => {};
  static Boolean: (result: boolean) => void = (_b: boolean) => {};
}

export class GeoPosData {
  lat: number;
  long: number;
  constructor(lat: number = 0, long: number = 0) {
    this.lat = lat;
    this.long = long;
  }
}

export class GeoPos extends GeoPosData {
  constructor(lat?: number | { lat: number; long: number }, long?: number) {
    if (lat !== null && typeof lat === 'object') {
      super(lat.lat, lat.long);
    } else {
      super(lat, long);
    }
  }
}

export class GeoPos3 implements GeoPos {
  lat: number;
  long: number;
  alt: number;
  constructor(lat?: number | { lat: number; long: number; alt?: number }, long?: number, alt: number = 0) {
    if (lat !== null && typeof lat === 'object') {
      this.lat = lat.lat;
      this.long = lat.long;
      this.alt = lat.alt !== undefined ? lat.alt : 0;
    } else {
      this.lat = lat === undefined ? 0 : lat;
      this.long = long === undefined ? 0 : long;
      this.alt = alt;
    }
  }
  toLatLong(): GeoPos {
    return new GeoPos(this.lat, this.long);
  }
}

export class Vec3Data {
  x: number = 0;
  y: number = 0;
  z: number = 0;
}

export class MapConfig {
  load(_path: string, callback: () => void): void {
    Promise.resolve().then(() => callback());
  }
  applyTo(_map: any): void {}
}

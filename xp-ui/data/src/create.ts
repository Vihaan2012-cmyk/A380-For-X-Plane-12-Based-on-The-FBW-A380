import type { Context, Host, HostSource, LogLevel } from './host.ts';
import { globalHost, requestFrame } from './host.ts';
import { createVars } from './vars.ts';
import { createBridge } from './bridge.ts';
import { createInstrumentHost } from './panelHost.ts';
import { createPlaneState } from './planestate.ts';
import { Units } from './units.ts';
import { NavMath } from './geo.ts';
import { Text } from './text.ts';
import {
  AircraftModel,
  GeoPos,
  GeoPos3,
  GeoPosData,
  InputButtons,
  KeyCodes,
  MapConfig,
  Noop,
  SessionPhase,
  SoundId,
  Vec3Data,
  sessionPhase,
} from './types.ts';

export interface CreateOptions {
  host?: HostSource;
}

export function createData(options: CreateOptions = {}) {
  const host: HostSource = options.host ? options.host : globalHost;
  let frameIndex = 0;

  const ctx: Context = {
    host,
    frameIndex: () => frameIndex,
    log(level: LogLevel, text: string): void {
      const h = host();
      if (h) {
        h.log(level, text);
      }
    },
  };

  const vars = createVars(ctx);
  const bridge = createBridge(ctx, (name: string, args: unknown[]): boolean => {
    if (name === 'var.set') {
      const pair = vars.pairOf(args[0]);
      if (pair) {
        vars.Vars.write(pair[0], pair[1], args[1]);
      }
      return true;
    }
    if (name === 'var.set.text') {
      const pair = vars.pairOf(args[0]);
      const h = host();
      if (pair && h) {
        h.setString(pair[0], String(args[1]));
      }
      return true;
    }
    return false;
  });
  const instruments = createInstrumentHost(ctx, bridge.Bridge.signal);

  function pageFrame(_time: number): void {
    requestFrame(pageFrame);
    frameIndex++;
    const h: Host | undefined = host();
    if (h) {
      h.snapshot();
    }
    const entries = h ? h.poll() : [];
    bridge.handlePoll(Array.isArray(entries) ? entries : [], instruments.panelEvent);
    vars.tick();
  }

  requestFrame(pageFrame);

  const Vars = vars.Vars;

  function refName(path: string, index?: number): string {
    return index === undefined ? path : path + '[' + index + ']';
  }

  const Datarefs = {
    get(path: string, index?: number): number {
      const h = host();
      return h ? h.getVar(refName(path, index), 'raw') : 0;
    },
    getText(path: string): string {
      const h = host();
      return h ? h.getString(path) : '';
    },
    set(path: string, value: number, index?: number): void {
      const h = host();
      if (h) {
        h.setVar(refName(path, index), 'raw', value);
      }
    },
    watch(path: string, listener: (value: number) => void, index?: number): { dispose(): void } {
      return Vars.watch(refName(path, index), 'raw', listener);
    },
  };

  function command(path: string, phase: number): void {
    const h = host();
    if (h) {
      h.sendEvent('CMD:' + path, [phase]);
    }
  }

  const Commands = {
    once(path: string): void {
      command(path, 0);
    },
    begin(path: string): void {
      command(path, 1);
    },
    end(path: string): void {
      command(path, 2);
    },
  };

  function loadStored(key: string): any {
    const h = host();
    const value = h ? h.storedData('get', key, '') : '';
    return value === '' || value === undefined || value === null ? null : value;
  }

  function saveStored(key: string, value: any): any {
    const h = host();
    if (h) {
      h.storedData('set', key, String(value));
    }
    return undefined;
  }

  const Declination = {
    at(lat: number, lon: number): number {
      const h = host();
      return h ? h.magVar(lat, lon) : 0;
    },
  };

  const PlaneState = createPlaneState(Vars.read);

  return {
    Vars,
    Datarefs,
    Commands,
    Units,
    Bridge: bridge.Bridge,
    Channels: bridge.Channels,
    openChannel: bridge.openChannel,
    openRelay: bridge.openRelay,
    sendFlowEvent: bridge.sendFlowEvent,
    loadStored,
    saveStored,
    Text,
    Declination,
    SessionPhase,
    sessionPhase,
    SoundId,
    Noop,
    GeoPosData,
    GeoPos,
    GeoPos3,
    Vec3Data,
    NavMath,
    AircraftModel,
    PlaneState,
    PanelInstrument: instruments.PanelInstrument,
    defineInstrument: instruments.defineInstrument,
    KeyCodes,
    InputButtons,
    MapElement: instruments.MapElement,
    MapConfig,
  };
}

export type XplaneJsData = ReturnType<typeof createData>;

export const exportNames: ReadonlyArray<keyof XplaneJsData> = [
  'Vars',
  'Datarefs',
  'Commands',
  'Units',
  'Bridge',
  'Channels',
  'openChannel',
  'openRelay',
  'sendFlowEvent',
  'loadStored',
  'saveStored',
  'Text',
  'Declination',
  'SessionPhase',
  'sessionPhase',
  'SoundId',
  'Noop',
  'GeoPosData',
  'GeoPos',
  'GeoPos3',
  'Vec3Data',
  'NavMath',
  'AircraftModel',
  'PlaneState',
  'PanelInstrument',
  'defineInstrument',
  'KeyCodes',
  'InputButtons',
  'MapElement',
  'MapConfig',
];

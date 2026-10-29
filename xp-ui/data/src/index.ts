import type { XplaneJsData } from './create.ts';
import type {
  AircraftModel as AircraftModelValue,
  SessionPhase as SessionPhaseValue,
} from './types.ts';

export type {
  AirportRecord,
  EventHandle,
  FacilityRecord,
  Listener,
  PanelTemplate,
  Relay,
  RunwayRecord,
  UnitName,
} from './types.ts';

const data = (globalThis as unknown as { xplaneJsData: XplaneJsData }).xplaneJsData;

export const Vars = data.Vars;
export const Datarefs = data.Datarefs;
export const Commands = data.Commands;
export const Units = data.Units;
export const Bridge = data.Bridge;
export const Channels = data.Channels;
export const openChannel = data.openChannel;
export const openRelay = data.openRelay;
export const sendFlowEvent = data.sendFlowEvent;
export const loadStored = data.loadStored;
export const saveStored = data.saveStored;
export const Text = data.Text;
export const Declination = data.Declination;
export const SessionPhase = data.SessionPhase;
export const sessionPhase = data.sessionPhase;
export const SoundId = data.SoundId;
export const Noop = data.Noop;
export const GeoPosData = data.GeoPosData;
export const GeoPos = data.GeoPos;
export const GeoPos3 = data.GeoPos3;
export const Vec3Data = data.Vec3Data;
export const NavMath = data.NavMath;
export const AircraftModel = data.AircraftModel;
export const PlaneState = data.PlaneState;
export const PanelInstrument = data.PanelInstrument;
export const defineInstrument = data.defineInstrument;
export const KeyCodes = data.KeyCodes;
export const InputButtons = data.InputButtons;
export const MapElement = data.MapElement;
export const MapConfig = data.MapConfig;

export type SessionPhase = SessionPhaseValue;
export type AircraftModel = AircraftModelValue;
export type KeyCodes = number;
export type SoundId = InstanceType<XplaneJsData['SoundId']>;
export type Noop = InstanceType<XplaneJsData['Noop']>;
export type GeoPosData = InstanceType<XplaneJsData['GeoPosData']>;
export type GeoPos = InstanceType<XplaneJsData['GeoPos']>;
export type GeoPos3 = InstanceType<XplaneJsData['GeoPos3']>;
export type Vec3Data = InstanceType<XplaneJsData['Vec3Data']>;
export type PanelInstrument = InstanceType<XplaneJsData['PanelInstrument']>;
export type MapElement = InstanceType<XplaneJsData['MapElement']>;
export type MapConfig = InstanceType<XplaneJsData['MapConfig']>;

export declare namespace Vars {
  type UnitName = string;
  type VarBatch = InstanceType<XplaneJsData['Vars']['VarBatch']>;
}

export declare namespace Channels {
  type Channel = InstanceType<XplaneJsData['Channels']['Channel']>;
}

export declare namespace NavMath {
  type Geo = InstanceType<XplaneJsData['NavMath']['Geo']>;
}

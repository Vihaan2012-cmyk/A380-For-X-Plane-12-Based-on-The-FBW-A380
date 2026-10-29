// @ts-strict-ignore
import { VerticalMode } from '@shared/autopilot';
import { PlaneState, Vars } from 'xplane-js/data';

export enum FmgcFlightPhase {
  Preflight,
  Takeoff,
  Climb,
  Cruise,
  Descent,
  Approach,
  GoAround,
  Done,
}

export function isReady(): boolean {
  return Vars.read('L:A32NX_IS_READY', 'number') === 1;
}

export function isSlewActive(): boolean {
  return Vars.read('fbw/IS_SLEW_ACTIVE', 'bool');
}

export function isOnGround(): boolean {
  return (
    Vars.read('L:A32NX_LGCIU_1_NOSE_GEAR_COMPRESSED', 'bool') ||
    Vars.read('L:A32NX_LGCIU_2_NOSE_GEAR_COMPRESSED', 'bool')
  );
}

function isEngineOn(index: number): boolean {
  return Vars.read(`L:A32NX_ENGINE_N2:${index}`, 'percent') > 20;
}

function isEngineOnTakeOffThrust(index: number): boolean {
  return Vars.read(`L:A32NX_ENGINE_N1:${index}`, 'percent') >= 70;
}

export function isAnEngineOn(): boolean {
  return isEngineOn(2) || isEngineOn(3);
}

export function isAllEngineOn(): boolean {
  return isEngineOn(2) && isEngineOn(3);
}

export function getAutopilotVerticalMode(): VerticalMode {
  return Vars.read('L:A32NX_FMA_VERTICAL_MODE', 'Enum');
}

export function conditionTakeOff(): boolean {
  return (
    (getAutopilotVerticalMode() === VerticalMode.SRS && isEngineOnTakeOffThrust(1) && isEngineOnTakeOffThrust(2)) ||
    Math.abs(PlaneState.groundSpeed()) > 90
  );
}

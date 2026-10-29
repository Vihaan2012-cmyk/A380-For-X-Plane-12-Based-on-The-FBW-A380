// @ts-strict-ignore
// Copyright (c) 2021-2024 FlyByWire Simulations
//
// SPDX-License-Identifier: GPL-3.0

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
  return Vars.read(`L:A32NX_ENGINE_N2:${index}`, 'number') > 55;
}

function isEngineOnTakeOffThrust(index: number): boolean {
  return Vars.read(`L:A32NX_ENGINE_N1:${index}`, 'number') >= 70;
}

export function isAnEngineOn(): boolean {
  return isEngineOn(1) || isEngineOn(2);
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

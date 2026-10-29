// Copyright (c) 2021-2023 FlyByWire Simulations
//
// SPDX-License-Identifier: GPL-3.0

import { Bus, VarSpec, VarUnit, VarPublisher } from 'xplane-js/ui';

export interface ClockSimvars {
  ltsTest: number;
  dcEssIsPowered: boolean;
  dcHot1IsPowered: boolean;
  absTime: number;
  timeOfDay: number;
  currentUTC: number;
  dayOfMonth: number;
  monthOfYear: number;
  year: number;
  elapsedKnobPos: number;
  dc2IsPowered: boolean;
}

export enum ClockVars {
  ltsTest = 'L:A32NX_OVHD_INTLT_ANN',
  dcEssIsPowered = 'L:A32NX_ELEC_DC_ESS_BUS_IS_POWERED',
  dcHot1IsPowered = 'L:A32NX_ELEC_DC_HOT_1_BUS_IS_POWERED',
  absTime = 'E:fbw/ABSOLUTE_TIME',
  timeOfDay = 'E:fbw/TIME_OF_DAY',
  currentUTC = 'E:fbw/ZULU_TIME',
  dayOfMonth = 'E:fbw/ZULU_DAY_OF_MONTH',
  monthOfYear = 'E:fbw/ZULU_MONTH_OF_YEAR',
  year = 'E:fbw/ZULU_YEAR',
  elapsedKnobPos = 'L:A32NX_CHRONO_ET_SWITCH_POS',
  dc2IsPowered = 'L:A32NX_ELEC_DC_2_BUS_IS_POWERED',
}

export class ClockSimvarPublisher extends VarPublisher<ClockSimvars> {
  private static simvars = new Map<keyof ClockSimvars, VarSpec>([
    ['ltsTest', { name: ClockVars.ltsTest, type: VarUnit.Number }],
    ['dcEssIsPowered', { name: ClockVars.dcEssIsPowered, type: VarUnit.Bool }],
    ['dcHot1IsPowered', { name: ClockVars.dcHot1IsPowered, type: VarUnit.Bool }],
    ['absTime', { name: ClockVars.absTime, type: VarUnit.Number }],
    ['timeOfDay', { name: ClockVars.timeOfDay, type: VarUnit.Enum }],
    ['currentUTC', { name: ClockVars.currentUTC, type: VarUnit.Number }],
    ['dayOfMonth', { name: ClockVars.dayOfMonth, type: VarUnit.Number }],
    ['monthOfYear', { name: ClockVars.monthOfYear, type: VarUnit.Number }],
    ['year', { name: ClockVars.year, type: VarUnit.Number }],
    ['elapsedKnobPos', { name: ClockVars.elapsedKnobPos, type: VarUnit.Number }],
    ['dc2IsPowered', { name: ClockVars.dc2IsPowered, type: VarUnit.Bool }],
  ]);

  public constructor(bus: Bus) {
    super(ClockSimvarPublisher.simvars, bus);
  }
}

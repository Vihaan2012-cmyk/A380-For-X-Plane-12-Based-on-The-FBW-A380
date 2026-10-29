//  Copyright (c) 2025 FlyByWire Simulations
//  SPDX-License-Identifier: GPL-3.0

import { EfisSide } from '@flybywiresim/fbw-sdk';
import { Bus, VarSpec, VarPublisher, VarUnit } from 'xplane-js/ui';

export type OitSimvars = {
  coldDark: number;
  elec: boolean;
  elecFo: boolean;
  potentiometerCaptain: number;
  potentiometerFo: number;
  oisDomainSwitchCapt: boolean;
  oisDomainSwitchFo: boolean;
  nssAnsu1Healthy: boolean;
  nssAnsu2Healthy: boolean;
  fltOpsAnsu1Healthy: boolean;
  laptopCaptHealthy: boolean;
  laptopFoHealthy: boolean;
  laptopCaptPowered: boolean;
  laptopFoPowered: boolean;
  nssMasterOff: boolean;
  nssDataToAvncsOff: boolean;
  parkBrakeSet: boolean;
  cabinDoorOpen: number;
  hydGreenPressurized: boolean;
  hydYellowPressurized: boolean;
};

export type InternalKbdKeyEvent = {
  kbdKeyEvent: [EfisSide, string];
};

/** A publisher to poll and publish nav/com simvars. */
export class OitSimvarPublisher extends VarPublisher<OitSimvars> {
  private static simvars = new Map<keyof OitSimvars, VarSpec>([
    ['coldDark', { name: 'L:A32NX_COLD_AND_DARK_SPAWN', type: VarUnit.Number }],
    ['elec', { name: 'L:A32NX_ELEC_AC_2_BUS_IS_POWERED', type: VarUnit.Bool }],
    ['elecFo', { name: 'L:A32NX_ELEC_AC_ESS_BUS_IS_POWERED', type: VarUnit.Bool }],
    ['potentiometerCaptain', { name: 'fbw/LIGHT_POTENTIOMETER_78', type: VarUnit.Number }],
    ['potentiometerFo', { name: 'fbw/LIGHT_POTENTIOMETER_79', type: VarUnit.Number }],
    ['oisDomainSwitchCapt', { name: 'L:A380X_SWITCH_OIT_SIDE_LEFT', type: VarUnit.Bool }],
    ['oisDomainSwitchFo', { name: 'L:A380X_SWITCH_OIT_SIDE_RIGHT', type: VarUnit.Bool }],
    ['nssAnsu1Healthy', { name: 'L:A32NX_NSS_ANSU_1_IS_HEALTHY', type: VarUnit.Bool }],
    ['nssAnsu2Healthy', { name: 'L:A32NX_NSS_ANSU_2_IS_HEALTHY', type: VarUnit.Bool }],
    ['fltOpsAnsu1Healthy', { name: 'L:A32NX_FLTOPS_ANSU_1_IS_HEALTHY', type: VarUnit.Bool }],
    ['laptopCaptHealthy', { name: 'L:A32NX_FLTOPS_LAPTOP_1_IS_HEALTHY', type: VarUnit.Bool }],
    ['laptopFoHealthy', { name: 'L:A32NX_FLTOPS_LAPTOP_2_IS_HEALTHY', type: VarUnit.Bool }],
    ['laptopCaptPowered', { name: 'L:A380X_SWITCH_LAPTOP_POWER_LEFT', type: VarUnit.Bool }],
    ['laptopFoPowered', { name: 'L:A380X_SWITCH_LAPTOP_POWER_RIGHT', type: VarUnit.Bool }],
    ['nssMasterOff', { name: 'L:A32NX_NSS_MASTER_OFF', type: VarUnit.Bool }],
    ['nssDataToAvncsOff', { name: 'L:A32NX_OVHD_NSS_DATA_TO_AVNCS_TOGGLE', type: VarUnit.Bool }],
    ['parkBrakeSet', { name: 'L:A32NX_PARK_BRAKE_LEVER_POS', type: VarUnit.Bool }],
    ['cabinDoorOpen', { name: 'fbw/INTERACTIVE_POINT_OPEN_0', type: VarUnit.PercentOver100 }],
    ['hydGreenPressurized', { name: 'L:A32NX_HYD_GREEN_SYSTEM_1_SECTION_PRESSURE_SWITCH', type: VarUnit.Bool }],
    [
      'hydYellowPressurized',
      { name: 'L:A32NX_HYD_YELLOW_SYSTEM_1_SECTION_PRESSURE_SWITCH', type: VarUnit.Bool },
    ],
  ]);

  public constructor(bus: Bus) {
    super(OitSimvarPublisher.simvars, bus);
  }
}

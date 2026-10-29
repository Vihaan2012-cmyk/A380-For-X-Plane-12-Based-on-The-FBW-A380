// Copyright (c) 2024 FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0

import { Bus, VarPublisher, VarUnit } from 'xplane-js/ui';
import { EfisNdMode, EfisOption, EfisSide, NavAidMode } from '@flybywiresim/fbw-sdk';

export interface FcuSimVars {
  ndRangeSetting: number;
  ndMode: EfisNdMode;
  option: EfisOption;
  navaidMode1: NavAidMode;
  navaidMode2: NavAidMode;
  oansRange: number;
  /** State of the LS pushbutton on the EFIS control panel. */
  efisLsActive: boolean;
  /** A380X only: Active overlay (0 = none, 1 = WXR, 2 = TERR) */
  a380x_efis_cp_active_overlay: number;
}

export class FcuBusPublisher extends VarPublisher<FcuSimVars> {
  constructor(bus: Bus, efisSide: EfisSide) {
    super(
      new Map([
        ['ndRangeSetting', { name: `L:A32NX_EFIS_${efisSide}_ND_RANGE`, type: VarUnit.Enum }],
        ['ndMode', { name: `L:A32NX_EFIS_${efisSide}_ND_MODE`, type: VarUnit.Enum }],
        ['option', { name: `L:A32NX_EFIS_${efisSide}_OPTION`, type: VarUnit.Enum }],
        ['navaidMode1', { name: `L:A32NX_EFIS_${efisSide}_NAVAID_1_MODE`, type: VarUnit.Enum }],
        ['navaidMode2', { name: `L:A32NX_EFIS_${efisSide}_NAVAID_2_MODE`, type: VarUnit.Enum }],
        ['efisLsActive', { name: `L:A380X_EFIS_${efisSide}_LS_BUTTON_IS_ON`, type: VarUnit.Bool }],
        ['oansRange', { name: `L:A32NX_EFIS_${efisSide}_OANS_RANGE`, type: VarUnit.Number }],
        [
          'a380x_efis_cp_active_overlay',
          { name: `L:A380X_EFIS_${efisSide}_ACTIVE_OVERLAY`, type: VarUnit.Number },
        ],
      ]),
      bus,
    );
  }
}

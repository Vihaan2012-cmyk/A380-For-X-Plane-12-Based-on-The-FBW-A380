// Copyright (c) 2024 FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0

import { Bus, VarPublisher, VarTopic, VarUnit } from 'xplane-js/ui';

export enum AnnLightTestState {
  Test = 0,
  Bright = 1,
  Dim = 2,
}

export interface OverheadEvents {
  ovhd_ann_lt_test_switch: AnnLightTestState;
  ovhd_ann_lt_test_active: boolean;
}

export class OverheadPublisher extends VarPublisher<OverheadEvents> {
  constructor(bus: Bus) {
    super(
      new Map<keyof OverheadEvents, VarTopic<any>>([
        ['ovhd_ann_lt_test_switch', { name: 'L:A32NX_OVHD_INTLT_ANN', type: VarUnit.Enum }],
        [
          'ovhd_ann_lt_test_active',
          { name: 'L:A32NX_OVHD_INTLT_ANN', type: VarUnit.Enum, transform: (v) => v === AnnLightTestState.Test },
        ],
      ]),
      bus,
    );
  }
}

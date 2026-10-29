// Copyright (c) 2025 FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0
import { VarPublisher, Bus, VarUnit } from 'xplane-js/ui';

export interface FmsMessageVars {
  tdReached: boolean;
  destEfobBelowMin: boolean;
}

export class FmsMessagePublisher extends VarPublisher<FmsMessageVars> {
  constructor(bus: Bus) {
    super(
      new Map([
        ['tdReached', { name: 'L:A32NX_PFD_MSG_TD_REACHED', type: VarUnit.Bool }],
        ['destEfobBelowMin', { name: 'L:A380X_FMS_DEST_EFOB_BELOW_MIN', type: VarUnit.Bool }],
      ]),
      bus,
    );
  }
}

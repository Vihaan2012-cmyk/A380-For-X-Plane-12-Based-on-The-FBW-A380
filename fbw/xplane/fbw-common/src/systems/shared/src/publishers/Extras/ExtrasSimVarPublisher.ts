// @ts-strict-ignore
// Copyright (c) 2023 FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0

import {
  Bus,
  IndexedTopic,
  Pacer,
  VarPublisher,
  VarTopic,
  VarUnit,
} from 'xplane-js/ui';

/* eslint-disable camelcase */

export interface BaseExtrasSimVarEvents {
  /** ECP TO CONF pushbutton state, A380X only! */
  ecp_to_config_pushbutton: boolean;
  /** FWC flight phase from 1 - 10 */
  fwc_flight_phase: number;
}

type IndexedTopics = null;

type ExtrasIndexedSimVarEvents = {
  [P in keyof Pick<BaseExtrasSimVarEvents, IndexedTopics> as IndexedTopic<P>]: BaseExtrasSimVarEvents[P];
};
export interface ExtrasSimVarEvents extends BaseExtrasSimVarEvents, ExtrasIndexedSimVarEvents {}

// FIXME remove this publisher and use the publishers oriented around the system providing the data to avoid duplication.
export class ExtrasSimVarPublisher extends VarPublisher<ExtrasSimVarEvents> {
  constructor(bus: Bus, pacer?: Pacer<ExtrasSimVarEvents>) {
    const simVars: [keyof ExtrasSimVarEvents, VarTopic<any>][] = [
      ['ecp_to_config_pushbutton', { name: 'L:A32NX_BTN_TOCONFIG', type: VarUnit.Bool, transform: (v) => !!v }],
      ['fwc_flight_phase', { name: 'L:A32NX_FWC_FLIGHT_PHASE', type: VarUnit.Number }],
    ];

    super(new Map(simVars), bus, pacer);
  }
}

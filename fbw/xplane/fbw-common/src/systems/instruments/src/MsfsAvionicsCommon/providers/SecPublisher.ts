// Copyright (c) 2024 FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0

/* eslint-disable camelcase */

import {
  Bus,
  IndexedTopic,
  Pacer,
  VarPublisher,
  VarTopic,
  VarUnit,
} from 'xplane-js/ui';

interface BaseSecEvents {
  sec_rudder_status_word: number;
  sec_rudder_trim_actual_position: number;
}

/**
 * Indexed events related to air data computer information.
 */
type SecIndexedEvents = {
  [P in keyof BaseSecEvents as IndexedTopic<P>]: BaseSecEvents[P];
};

export interface SecDataEvents extends BaseSecEvents, SecIndexedEvents {}

export class SecPublisher extends VarPublisher<SecDataEvents> {
  /**
   * Creates an AdcPublisher.
   * @param bus The event bus to which to publish.
   * @param pacer An optional pacer to use to control the rate of publishing.
   */
  public constructor(bus: Bus, pacer?: Pacer<SecDataEvents>) {
    const simvars = new Map<keyof SecDataEvents, VarTopic<any>>([
      [
        'sec_rudder_status_word',
        { name: 'L:A32NX_SEC_#index#_RUDDER_STATUS_WORD', type: VarUnit.Number, indexed: true },
      ],
      [
        'sec_rudder_trim_actual_position',
        { name: 'L:A32NX_SEC_#index#_RUDDER_ACTUAL_POSITION', type: VarUnit.Number, indexed: true },
      ],
    ]);
    super(simvars, bus, pacer);
  }
}

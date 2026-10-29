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

interface BaseFwcEvents {
  fwc_discrete_word_126: number;
}

/**
 * Indexed events related to air data computer information.
 */
type FwcIndexedEvents = {
  [P in keyof BaseFwcEvents as IndexedTopic<P>]: BaseFwcEvents[P];
};

export interface FwcDataEvents extends BaseFwcEvents, FwcIndexedEvents {}

export class FwcPublisher extends VarPublisher<FwcDataEvents> {
  /**
   * Creates an AdcPublisher.
   * @param bus The event bus to which to publish.
   * @param pacer An optional pacer to use to control the rate of publishing.
   */
  public constructor(bus: Bus, pacer?: Pacer<FwcDataEvents>) {
    const simvars = new Map<keyof FwcDataEvents, VarTopic<any>>([
      [
        'fwc_discrete_word_126',
        { name: 'L:A32NX_FWC_#index#_DISCRETE_WORD_126', type: VarUnit.Number, indexed: true },
      ],
    ]);
    super(simvars, bus, pacer);
  }
}

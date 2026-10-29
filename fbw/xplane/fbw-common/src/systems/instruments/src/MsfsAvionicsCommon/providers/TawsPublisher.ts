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

interface BaseTawsEvents {
  egpws_alert_discrete_word_1: number;
  egpws_alert_discrete_word_2: number;
}

/**
 * Indexed events related to air data computer information.
 */
type TawsIndexedEvents = {
  [P in keyof BaseTawsEvents as IndexedTopic<P>]: BaseTawsEvents[P];
};

export interface TawsDataEvents extends BaseTawsEvents, TawsIndexedEvents {}

export class TawsPublisher extends VarPublisher<TawsDataEvents> {
  /**
   * Creates an AdcPublisher.
   * @param bus The event bus to which to publish.
   * @param pacer An optional pacer to use to control the rate of publishing.
   */
  public constructor(bus: Bus, pacer?: Pacer<TawsDataEvents>) {
    const simvars = new Map<keyof TawsDataEvents, VarTopic<any>>([
      [
        'egpws_alert_discrete_word_1',
        { name: 'L:A32NX_EGPWS_ALERT_#index#_DISCRETE_WORD_1', type: VarUnit.Number, indexed: true },
      ],
      [
        'egpws_alert_discrete_word_2',
        { name: 'L:A32NX_EGPWS_ALERT_#index#_DISCRETE_WORD_2', type: VarUnit.Number, indexed: true },
      ],
    ]);
    super(simvars, bus, pacer);
  }
}

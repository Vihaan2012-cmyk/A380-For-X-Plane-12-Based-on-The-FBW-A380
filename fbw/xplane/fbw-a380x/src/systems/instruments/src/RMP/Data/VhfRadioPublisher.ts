// Copyright (c) 2024 FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0

import {
  Bus,
  IndexedTopic,
  Pacer,
  VarPublisher,
  VarTopic,
  VarUnit,
} from 'xplane-js/ui';

interface BaseVhfRadioEvents {
  /** The VHF radio frequeny ARINC429 word. */
  vhf_radio_frequency: number;
}

type IndexedTopics = 'vhf_radio_frequency';

type VhfRadioIndexedEvents = {
  [P in keyof Pick<BaseVhfRadioEvents, IndexedTopics> as IndexedTopic<P>]: BaseVhfRadioEvents[P];
};

export interface VhfRadioEvents extends BaseVhfRadioEvents, VhfRadioIndexedEvents {}

export class VhfRadioPublisher extends VarPublisher<VhfRadioEvents> {
  /**
   * Create a VHF radio publisher.
   * @param bus The EventBus to publish to
   * @param pacer An optional pacer to use to control the rate of publishing
   */
  public constructor(bus: Bus, pacer?: Pacer<VhfRadioEvents>) {
    const simvars = new Map<keyof VhfRadioEvents, VarTopic<any>>([
      ['vhf_radio_frequency', { name: `L:FBW_VHF#index#_FREQUENCY`, type: VarUnit.Number, indexed: true }],
    ]);

    super(simvars, bus, pacer);
  }
}

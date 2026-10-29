// Copyright (c) 2024 FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0

import { BusSignal, Bus, BoardDevice } from 'xplane-js/ui';
import { AudioControlLocalVarEvents } from '../Data/AudioControlPublisher';
import { RmpMessageControlEvents } from './RmpMessageManager';

/** Monitors the transmit state of each external COM means and posts a message if receive is not selected. */
export class TransmitMessageMonitor implements BoardDevice {
  private readonly sub = this.bus.inbox<AudioControlLocalVarEvents>();
  private pub = this.bus.outbox<RmpMessageControlEvents>();

  private static readonly monitorDefinitions = [
    {
      type: 'VHF',
      index: 1,
    },
    {
      type: 'VHF',
      index: 2,
    },
    {
      type: 'VHF',
      index: 3,
    },
    {
      type: 'HF',
      index: 1,
    },
    {
      type: 'HF',
      index: 2,
    },
    {
      type: 'TEL',
      index: 1,
    },
    {
      type: 'TEL',
      index: 2,
    },
  ];

  private readonly monitorSubs = TransmitMessageMonitor.monitorDefinitions.map((def) => {
    const receiveState = BusSignal.from(
      this.sub.topic(`${def.type.toLowerCase()}_receive_${def.index}` as keyof AudioControlLocalVarEvents),
      false,
    );
    const transmitState = BusSignal.from(
      this.sub.topic(`${def.type.toLowerCase()}_transmit_${def.index}` as keyof AudioControlLocalVarEvents),
      false,
    );
    return transmitState.watch(
      (tx) =>
        tx && !receiveState.get() && this.pub.post('rmp_message_set', `${def.type}${def.index} RECEPTION NOT SELECTED`),
      false,
      true,
    );
  });

  constructor(private readonly bus: Bus) {}

  setup(): void {
    for (const sub of this.monitorSubs) {
      // no initialNotify as we don't care about boot up state...
      sub.release();
    }
  }
  tick(): void {
    // noop
  }
}

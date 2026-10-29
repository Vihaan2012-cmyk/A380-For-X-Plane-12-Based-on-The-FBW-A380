// Copyright (c) 2024 FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0

import { ClockTopics, Bus, Signal } from 'xplane-js/ui';

export interface FlasherEvents {
  flash_1hz: boolean;
}

export class Flasher {
  constructor(bus: Bus) {
    const sub = bus.inbox<ClockTopics>();

    const flash1Hz = Signal.of(false);

    sub
      .topic('simTime')
      .limit(2)
      .listen(() => flash1Hz.set(!flash1Hz.get()));

    const pub = bus.outbox<FlasherEvents>();

    flash1Hz.watch((v) => pub.post('flash_1hz', v));
  }
}

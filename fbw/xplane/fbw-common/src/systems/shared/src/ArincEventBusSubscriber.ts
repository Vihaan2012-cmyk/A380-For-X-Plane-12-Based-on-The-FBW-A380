//  Copyright (c) 2026 FlyByWire Simulations
//  SPDX-License-Identifier: GPL-3.0

import { Exchange as Bus } from 'xplane-js/ui';
import { ArincConsumer, BasicArincConsumer } from './ArincConsumer';

export class ArincEventSubscriber<E> {
  constructor(private readonly bus: Bus) {}

  public on<K extends keyof E & string>(topic: K): ArincConsumer<E[K]> {
    return new BasicArincConsumer<E[K]>((handler, paused) => this.bus.listen(topic, handler, paused));
  }
}

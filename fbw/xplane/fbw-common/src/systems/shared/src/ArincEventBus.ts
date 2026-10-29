//  Copyright (c) 2026 FlyByWire Simulations
//  SPDX-License-Identifier: GPL-3.0

import { Exchange as Bus } from 'xplane-js/ui';
import { ArincEventSubscriber } from './ArincEventBusSubscriber';

export class ArincEventBus extends Bus {
  public getArincSubscriber<E>(): ArincEventSubscriber<E> {
    return new ArincEventSubscriber<E>(this);
  }
}

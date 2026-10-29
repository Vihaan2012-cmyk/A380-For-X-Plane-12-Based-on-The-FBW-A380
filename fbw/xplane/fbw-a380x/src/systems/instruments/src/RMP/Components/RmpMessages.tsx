// Copyright (c) 2024 FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0

import { ViewProps, BusSignal, View, Bus, UI, ViewNode } from 'xplane-js/ui';
import { RmpMessageDataEvents } from '../Systems/RmpMessageManager';

export interface RmpMessagesProps extends ViewProps {
  bus: Bus;
}

export class RmpMessages extends View<RmpMessagesProps> {
  private readonly message = BusSignal.from(
    this.props.bus.inbox<RmpMessageDataEvents>().topic('rmp_message'),
    '',
  );

  /** @inheritdoc */
  render(): ViewNode {
    return <div class="rmp-messages font-10">{this.message}</div>;
  }
}

// @ts-strict-ignore
// Copyright (c) 2024 FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0

import {
  UI,
  View,
  ViewNode,
  Readable,
  Derived,
  ViewProps,
  Bus,
  BusSignal,
  Signal,
  Readables,
} from 'xplane-js/ui';
import { ExtendedClockEvents } from './providers/ExtendedClockProvider';
import { NXLogicTriggeredMonostableNode } from '@flybywiresim/fbw-sdk';

export interface FlashProps extends ViewProps {
  bus: Bus;
  className1?: Readable<string> | string;
  className2?: Readable<string> | string;
  visible?: Readable<boolean>;
  flashing?: Readable<boolean>;
  flashDuration: number;
}

export class FlashOneHertz extends View<FlashProps> {
  private readonly oneHertzClock = BusSignal.from(null, false);

  private readonly flashingMtrig = new NXLogicTriggeredMonostableNode(this.props.flashDuration, true);

  private readonly flashingMtrigResult = Signal.of(false);

  private readonly visible = Derived.from(
    ([visible, oneHertzClock, flashingMtrig]) => visible && !(flashingMtrig && oneHertzClock),
    this.props.visible ?? Signal.of(true),
    this.oneHertzClock,
    this.flashingMtrigResult,
  );

  private readonly class = Derived.from(
    ([visible, class1, class2]) => (visible ? class1 : class2),
    this.visible,
    Readables.toReadable(this.props.className1 ?? '', true),
    Readables.toReadable(this.props.className2 ?? 'HiddenElement', true),
  );

  private prevClass = '';

  afterRender(node: ViewNode): void {
    super.afterRender(node);

    const sub = this.props.bus.inbox<ExtendedClockEvents>();

    this.oneHertzClock.rebind(sub.topic('ext_clock_one_hertz'));

    sub.topic('ext_clock_delta_time').listen((dt) => {
      const visible = this.props.visible?.get() ?? true;
      const shouldFlash = this.props.flashing?.get() ?? true;
      this.flashingMtrigResult.set(
        this.props.flashDuration === Infinity
          ? visible && shouldFlash
          : this.flashingMtrig.write(visible && shouldFlash, dt),
      );
    });

    this.class.watch((value) => {
      this.props.children.forEach((child) => {
        const classList = ((child as ViewNode).target as HTMLElement).classList;

        classList.remove(...UI.splitClasses(this.prevClass));
        classList.add(...UI.splitClasses(value));
      });

      this.prevClass = value;
    }, true);
  }

  render(): ViewNode {
    return <>{this.props.children}</>;
  }
}

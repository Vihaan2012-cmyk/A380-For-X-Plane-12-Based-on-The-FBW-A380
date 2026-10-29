// @ts-strict-ignore
// Copyright (c) 2021-2023 FlyByWire Simulations
//
// SPDX-License-Identifier: GPL-3.0

import {
  BusSignal,
  View,
  Bus,
  UI,
  Derived,
  Signal,
  Readable,
  ViewNode,
} from 'xplane-js/ui';
import { EfisNdMode, Arinc429ConsumerSubject, MathUtils } from '@flybywiresim/fbw-sdk';

import { NDSimvars } from '../NDSimvarPublisher';
import { GenericDisplayManagementEvents } from '../types/GenericDisplayManagementEvents';
import { GenericFcuEvents } from '../types/GenericFcuEvents';

export interface TrackBugProps {
  bus: Bus;
  isUsingTrackUpMode: Readable<boolean>;
  ndMode: Readable<EfisNdMode>;
}

export class TrackBug extends View<TrackBugProps> {
  private readonly ndMode = BusSignal.from(null, EfisNdMode.ARC);

  private readonly headingWord = Arinc429ConsumerSubject.create(null);

  private readonly trackWord = Arinc429ConsumerSubject.create(null);

  private readonly diffSubject = Signal.of(0);

  private readonly bugShown = Signal.of(false);

  private readonly transformSubject = Derived.from(
    ([diff, ndMode]) => {
      return `rotate(${diff} 384 ${ndMode === EfisNdMode.ARC ? 620 : 384})`;
    },
    this.diffSubject,
    this.props.ndMode,
  );

  afterRender(node: ViewNode) {
    super.afterRender(node);

    const sub = this.props.bus.inbox<GenericDisplayManagementEvents & NDSimvars & GenericFcuEvents>();

    this.ndMode.rebind(sub.topic('ndMode').distinct());
    this.headingWord.setConsumer(sub.topic('heading'));
    this.trackWord.setConsumer(sub.topic('track'));

    this.ndMode.watch(() => this.handleDisplay(), true);
    this.headingWord.watch(() => this.handleDisplay(), true);
    this.trackWord.watch(() => this.handleDisplay(), true);
  }

  private handleDisplay() {
    const wrongNdMode = this.ndMode.get() === EfisNdMode.PLAN;

    const headingInvalid = !this.headingWord.get().isNormalOperation();
    const trackInvalid = !this.trackWord.get().isNormalOperation();

    if (wrongNdMode || headingInvalid || trackInvalid) {
      this.bugShown.set(false);
    } else {
      let diff;
      if (this.props.isUsingTrackUpMode.get()) {
        diff = 0;
      } else {
        diff = MathUtils.getSmallestAngle(this.trackWord.get().value, this.headingWord.get().value);
      }

      this.bugShown.set(diff <= 40);
      this.diffSubject.set(diff);
    }
  }

  render(): ViewNode | null {
    return (
      <g visibility={this.bugShown.derive((v) => (v ? 'inherit' : 'hidden'))} transform={this.transformSubject}>
        <path
          d={this.ndMode.derive((ndMode) =>
            ndMode !== EfisNdMode.ARC
              ? 'M384,134 L379,143 L384,152 L389,143 L384,134'
              : 'M384,128 L378,138 L384,148 L390,138 L384,128',
          )}
          class="rounded shadow"
          stroke-width={4.5}
        />
        <path
          d={this.ndMode.derive((ndMode) =>
            ndMode !== EfisNdMode.ARC
              ? 'M384,134 L379,143 L384,152 L389,143 L384,134'
              : 'M384,128 L378,138 L384,148 L390,138 L384,128',
          )}
          class="Green rounded"
          stroke-width={3}
        />
      </g>
    );
  }
}

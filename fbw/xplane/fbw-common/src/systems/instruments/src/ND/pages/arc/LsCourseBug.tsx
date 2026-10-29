// @ts-strict-ignore
// Copyright (c) 2021-2023 FlyByWire Simulations
//
// SPDX-License-Identifier: GPL-3.0

import {
  UI,
  View,
  Derived,
  Signal,
  Readable,
  ViewNode,
  BusSignal,
} from 'xplane-js/ui';
import { ArincEventBus, EfisNdMode, Arinc429ConsumerSubject, MathUtils } from '@flybywiresim/fbw-sdk';

import { NDSimvars } from '../../NDSimvarPublisher';
import { GenericDisplayManagementEvents } from '../../types/GenericDisplayManagementEvents';
import { GenericFcuEvents } from '../../types/GenericFcuEvents';

export interface LsCourseBugProps {
  bus: ArincEventBus;
  rotationOffset: Readable<number>;
  mode: Readable<EfisNdMode>;
}

export class LsCourseBug extends View<LsCourseBugProps> {
  private readonly diffSubject = Signal.of(0);

  private readonly headingWord = Arinc429ConsumerSubject.create(null).hold();

  private readonly lsCourse = BusSignal.from(null, 0).hold();

  private readonly efisLsActive = BusSignal.from(null, false);

  private readonly bugShown = Derived.from(
    ([headingWord, lsCourse, diff, efisLsActive]) =>
      efisLsActive && headingWord.isNormalOperation() && lsCourse >= 0 && Math.abs(diff) <= 48,
    this.headingWord,
    this.lsCourse,
    this.diffSubject,
    this.efisLsActive,
  );

  private readonly transformSubject = Derived.from(
    ([diff, ndMode]) => {
      return `rotate(${diff} 384 ${ndMode === EfisNdMode.ARC ? 620 : 384})`;
    },
    this.diffSubject,
    this.props.mode,
  );

  afterRender(node: ViewNode) {
    super.afterRender(node);

    const sub = this.props.bus.getArincSubscriber<GenericDisplayManagementEvents & GenericFcuEvents & NDSimvars>();

    this.headingWord.setConsumer(sub.on('heading').withArinc429Precision(2));

    this.headingWord.watch((_h) => this.handleDisplay());

    this.lsCourse.watch(this.handleDisplay.bind(this));

    this.lsCourse.rebind(sub.on('ilsCourse'));

    this.efisLsActive.watch((ls) => {
      if (ls) {
        this.headingWord.release();
        this.lsCourse.release();
        this.handleDisplay();
      } else {
        this.headingWord.hold();
        this.lsCourse.hold();
      }
    });

    this.efisLsActive.rebind(sub.on('efisLsActive'));
  }

  private handleDisplay() {
    const headingValid = this.headingWord.get().isNormalOperation();

    if (headingValid) {
      const diff = MathUtils.getSmallestAngle(this.lsCourse.get(), this.headingWord.get().value);

      this.diffSubject.set(diff + this.props.rotationOffset.get());
    }
  }

  render(): ViewNode | null {
    return (
      <>
        <g visibility={this.bugShown.derive((v) => (v ? '' : 'hidden'))} transform={this.transformSubject}>
          <line x1={376} y1={114} x2={392} y2={114} class="rounded shadow" stroke-width={2.5} />
          <line x1={384} y1={122} x2={384} y2={74} class="rounded shadow" stroke-width={2.5} />

          <line x1={376} y1={114} x2={392} y2={114} class="Magenta rounded" stroke-width={2.5} />
          <line x1={384} y1={122} x2={384} y2={74} class="Magenta rounded" stroke-width={2.5} />
        </g>
      </>
    );
  }
}

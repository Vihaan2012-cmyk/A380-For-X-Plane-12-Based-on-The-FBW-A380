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

export interface SelectedHeadingBugProps {
  bus: ArincEventBus;
  rotationOffset: Readable<number>;
  mode: Readable<EfisNdMode>;
}

export class SelectedHeadingBug extends View<SelectedHeadingBugProps> {
  private readonly diffSubject = Signal.of(0);

  private readonly headingWord = Arinc429ConsumerSubject.create(null);

  private readonly selected = Signal.of(0);

  private readonly isSelectedHeadingShown = BusSignal.from(null, false);

  // eslint-disable-next-line
    private readonly bugShown = Derived.from(([headingWord, isShown, diff, mode]) => {
      if (!isShown) {
        return false;
      }
      if (!headingWord.isNormalOperation()) {
        return false;
      }

      if (mode === EfisNdMode.ROSE_ILS || mode === EfisNdMode.ROSE_NAV || mode === EfisNdMode.ROSE_VOR) {
        return true;
      }

      if (mode === EfisNdMode.PLAN) {
        return false;
      }

      return diff <= 48;
    },
    this.headingWord,
    this.isSelectedHeadingShown,
    this.diffSubject,
    this.props.mode,
  );

  // eslint-disable-next-line
    private readonly textShown = Derived.from(([headingWord, isShown, diff, mode]) => {
      if (!isShown) {
        return false;
      }
      if (!headingWord.isNormalOperation()) {
        return false;
      }

      if (mode === EfisNdMode.ROSE_ILS || mode === EfisNdMode.ROSE_NAV || mode === EfisNdMode.ROSE_VOR) {
        return false;
      }

      if (mode === EfisNdMode.PLAN) {
        return false;
      }

      return Math.abs(diff) > 48;
    },
    this.headingWord,
    this.isSelectedHeadingShown,
    this.diffSubject,
    this.props.mode,
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

    const sub = this.props.bus.getArincSubscriber<GenericDisplayManagementEvents & NDSimvars>();

    sub
      .on('selectedHeading')
      .distinct()
      .listen((v) => {
        this.selected.set(v);
        this.handleDisplay();
      });

    this.headingWord.setConsumer(sub.on('heading').withArinc429Precision(2));

    this.isSelectedHeadingShown.rebind(sub.on('showSelectedHeading').distinct());

    this.headingWord.watch((_v) => this.handleDisplay(), true);
  }

  private handleDisplay() {
    const headingValid = this.headingWord.get().isNormalOperation();

    if (headingValid) {
      const diff = MathUtils.getSmallestAngle(this.selected.get(), this.headingWord.get().value);

      this.diffSubject.set(diff + this.props.rotationOffset.get());
    }
  }

  render(): ViewNode | null {
    return (
      <>
        <g visibility={this.bugShown.derive((v) => (v ? 'inherit' : 'hidden'))} transform={this.transformSubject}>
          <path d="M382,126 L370,99 L398,99 L386,126" class="rounded shadow" stroke-width={3.5} />
          <path d="M382,126 L370,99 L398,99 L386,126" class="Cyan rounded" stroke-width={3} />
        </g>

        <text
          visibility={this.textShown.derive((v) => (v ? 'inherit' : 'hidden'))}
          x={384}
          y={60}
          text-anchor="middle"
          transform={this.diffSubject.derive((diff) => `rotate(${diff < 0 ? -38 : 38} 384 620)`)}
          class="Cyan shadow"
          font-size={22}
        >
          {this.selected.derive((selected) => `${Math.round(selected).toString().padStart(3, '0')}`)}
        </text>
      </>
    );
  }
}

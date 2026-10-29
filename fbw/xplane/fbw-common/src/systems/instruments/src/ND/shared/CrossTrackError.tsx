// Copyright (c) 2021-2023 FlyByWire Simulations
//
// SPDX-License-Identifier: GPL-3.0

import {
  UI,
  View,
  Signal,
  Readable,
  ViewNode,
  Bus,
  Derived,
} from 'xplane-js/ui';
import { EfisNdMode } from '@flybywiresim/fbw-sdk';

import { GenericFmsEvents } from '../types/GenericFmsEvents';

export interface CrossTrackErrorProps {
  bus: Bus;
  currentPageMode: Readable<EfisNdMode>;
  isNormalOperation: Readable<boolean>; // TODO replace with ARINC429 word
}

export class CrossTrackError extends View<CrossTrackErrorProps> {
  private readonly crossTrackText = Signal.of('');

  private readonly crossTrackX = Signal.of(0);

  private readonly xValueInitial = this.props.currentPageMode.derive((p) => {
    return p === EfisNdMode.PLAN ? 44 : 390;
  });

  private readonly yValueInitial = this.props.currentPageMode.derive((p) => {
    switch (p) {
      case EfisNdMode.ARC:
        return 646;
      case EfisNdMode.PLAN:
        return 690;
      case EfisNdMode.ROSE_NAV:
        return 407;
      default:
        return 0;
    }
  });

  private readonly crossTrackAnchor = Signal.of('');

  private rnp = 0;

  /* eslint-disable max-len */
  private readonly crossTrackVisibility = Derived.from(
    ([isNormalOperation, currentPageMode]) =>
      isNormalOperation &&
      (currentPageMode === EfisNdMode.ARC ||
        currentPageMode === EfisNdMode.PLAN ||
        currentPageMode === EfisNdMode.ROSE_NAV)
        ? 'inherit'
        : 'hidden',
    this.props.isNormalOperation,
    this.props.currentPageMode,
  );

  afterRender(node: ViewNode) {
    super.afterRender(node);

    const sub = this.props.bus.inbox<GenericFmsEvents>();

    sub
      .topic('crossTrackError')
      .limit(2)
      .listen((crossTrackError) => {
        const x = this.xValueInitial.get();

        let crossTrackText = '';
        let crossTrackAnchor = 'start';
        let crossTrackX = x;
        const crossTrackAbs = Math.min(99.9, Math.abs(crossTrackError));

        if (
          this.rnp > 0 &&
          this.rnp <= 0.3 + Number.EPSILON &&
          crossTrackAbs >= 0.02 - Number.EPSILON &&
          crossTrackAbs < 0.3 + Number.EPSILON
        ) {
          crossTrackText = crossTrackAbs.toFixed(2);
        } else if (crossTrackAbs >= 0.1) {
          crossTrackText = crossTrackAbs.toFixed(1);
        }

        if (crossTrackText.length > 0) {
          if (crossTrackError < 0) {
            crossTrackText += 'R';
            crossTrackAnchor = 'start';
            crossTrackX = x + 34;
          } else {
            crossTrackText += 'L';
            crossTrackAnchor = 'end';
            crossTrackX = x - 38;
          }
        }

        this.crossTrackText.set(crossTrackText);
        this.crossTrackAnchor.set(crossTrackAnchor);
        this.crossTrackX.set(crossTrackX);
      });

    sub
      .topic('rnp')
      .distinct()
      .listen((rnp) => {
        this.rnp = rnp;
      });
  }

  render(): ViewNode | null {
    return (
      <text
        x={Derived.from(
          ([currentPageMode, crossTrackX, xValueInitial]) =>
            currentPageMode === EfisNdMode.PLAN ? xValueInitial : crossTrackX,
          this.props.currentPageMode,
          this.crossTrackX,
          this.xValueInitial,
        )}
        y={this.yValueInitial}
        text-anchor={Derived.from(
          ([currentPageMode, crossTrackAnchor]) => (currentPageMode === EfisNdMode.PLAN ? 'start' : crossTrackAnchor),
          this.props.currentPageMode,
          this.crossTrackAnchor,
        )}
        class="Green FontSmall shadow"
        visibility={this.crossTrackVisibility}
      >
        {this.crossTrackText}
      </text>
    );
  }
}

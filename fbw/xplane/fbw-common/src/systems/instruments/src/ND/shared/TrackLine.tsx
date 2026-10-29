// @ts-strict-ignore
// Copyright (c) 2021-2026 FlyByWire Simulations
//
// SPDX-License-Identifier: GPL-3.0

import {
  UI,
  View,
  Bus,
  ViewNode,
  Derived,
  Readable,
  BusSignal,
  Signal,
} from 'xplane-js/ui';
import { MathUtils, EfisNdMode, Arinc429ConsumerSubject } from '@flybywiresim/fbw-sdk';

import { NDSimvars } from '../NDSimvarPublisher';
import { GenericDisplayManagementEvents } from '../types/GenericDisplayManagementEvents';
import { GenericFcuEvents } from '../types/GenericFcuEvents';
import { GenericFlightGuidanceEvents } from '../types/GenericFlightGuidanceEvents';
import { FmsSymbolsData } from '../FmsSymbolsPublisher';

export interface TrackLineProps {
  bus: Bus;
  isUsingTrackUpMode: Readable<boolean>;
}

const TRACK_LINE_Y_POSITION = {
  [EfisNdMode.ROSE_NAV]: 384,
  [EfisNdMode.ARC]: 620,
};

export class TrackLine extends View<TrackLineProps> {
  private readonly lineRef = UI.ref<SVGLineElement>();

  private readonly sub = this.props.bus.inbox<
    GenericDisplayManagementEvents & GenericFlightGuidanceEvents & NDSimvars & GenericFcuEvents & FmsSymbolsData
  >();

  private readonly ndMode = BusSignal.from(this.sub.topic('ndMode').distinct(), EfisNdMode.ARC);

  private readonly headingWord = Arinc429ConsumerSubject.create(null);

  private readonly trackWord = Arinc429ConsumerSubject.create(null);

  private readonly visibility = Signal.of('hidden');

  private readonly rotate = Derived.from(
    ([heading, track]) => {
      if (this.props.isUsingTrackUpMode.get()) {
        return 0;
      }

      if (heading.isNormalOperation() && track.isNormalOperation()) {
        return MathUtils.diffAngle(heading.value, track.value);
      }

      return 0;
    },
    this.headingWord,
    this.trackWord,
  );

  private readonly y = this.ndMode.derive((mode) => TRACK_LINE_Y_POSITION[mode] ?? 0);

  private readonly transform = Derived.from(
    ([rotation, y]) => {
      return `rotate(${rotation} 384 ${y})`;
    },
    this.rotate,
    this.y,
  );

  private readonly areActiveVectorsTransmitted = BusSignal.from(this.sub.topic('vectorsActive'), []).derive(
    (vectors) => vectors !== undefined && vectors !== null,
  );

  afterRender(node: ViewNode) {
    super.afterRender(node);

    this.headingWord.setConsumer(this.sub.topic('heading'));
    this.trackWord.setConsumer(this.sub.topic('track'));

    this.headingWord.watch(() => this.handleLineVisibility(), true);
    this.trackWord.watch(() => this.handleLineVisibility(), true);
    this.ndMode.watch(() => this.handleLineVisibility(), true);
    this.areActiveVectorsTransmitted.watch(() => this.handleLineVisibility(), true);
  }

  private handleLineVisibility() {
    const wrongNDMode = TRACK_LINE_Y_POSITION[this.ndMode.get()] === undefined;

    const headingInvalid = !this.headingWord.get().isNormalOperation();
    const trackInvalid = !this.trackWord.get().isNormalOperation();

    const areActiveVectorsTransmitted = this.areActiveVectorsTransmitted.get();

    const shouldShowLine = !areActiveVectorsTransmitted;

    if (wrongNDMode || headingInvalid || trackInvalid || !shouldShowLine) {
      this.visibility.set('hidden');
    } else {
      this.visibility.set('inherit');
    }
  }

  render(): ViewNode | null {
    return (
      <g ref={this.lineRef} transform={this.transform} visibility={this.visibility}>
        <line x1={384} y1={149} x2={384} y2={this.y} class="rounded shadow" stroke-width={3.0} />
        <line x1={384} y1={149} x2={384} y2={this.y} class="Green rounded" stroke-width={2.5} />
      </g>
    );
  }
}

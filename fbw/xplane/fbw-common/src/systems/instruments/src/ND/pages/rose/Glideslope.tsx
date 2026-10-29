// @ts-strict-ignore
// Copyright (c) 2024 FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0

import {
  UI,
  ViewProps,
  View,
  ViewNode,
  Signal,
  Bus,
  Derived,
  Readable,
  BusSignal,
} from 'xplane-js/ui';
import { GenericVorEvents } from '../../types/GenericVorEvents';
import { Layer } from '../../../MsfsAvionicsCommon/Layer';

export interface GlideSlopeProps extends ViewProps {
  bus: Bus;
  backbeam?: Readable<boolean>;
}

export class GlideSlope extends View<GlideSlopeProps> {
  private readonly sub = this.props.bus.inbox<GenericVorEvents>();

  private readonly backbeam = this.props.backbeam ?? Signal.of(false);

  // FIXME hook up when MMR ready
  private readonly mixLocVnav = Signal.of(false);

  private readonly isHidden = Derived.from(
    ([backbeam, mixLocVnav]) => backbeam && !mixLocVnav,
    this.backbeam,
    this.mixLocVnav,
  );

  private readonly glideSlopeValid = BusSignal.from(this.sub.topic('glideSlopeValid'), false);

  private readonly gsAvailable = Derived.from(
    ([isHidden, glideSlopeValid]) => !isHidden && glideSlopeValid,
    this.isHidden,
    this.glideSlopeValid,
  );

  private readonly gsDeviation = BusSignal.from(this.sub.topic('glideSlopeDeviation'), 0);

  private readonly deviationPxSub = this.gsDeviation.derive((deviation) => {
    return (deviation / 0.8) * 128;
  });

  private readonly deviationUpperVisibleSub = Derived.from(
    ([available, deviationPx]) => {
      return available && deviationPx < 128;
    },
    this.gsAvailable,
    this.deviationPxSub,
  );

  private readonly deviationLowerVisibleSub = Derived.from(
    ([available, deviationPx]) => {
      return available && deviationPx > -128;
    },
    this.gsAvailable,
    this.deviationPxSub,
  );

  private readonly visibilityFn = (v) => (v ? 'inherit' : 'hidden');

  render(): ViewNode | null {
    return (
      <g
        id="glideslope-deviation"
        class={{
          hidden: this.isHidden,
        }}
      >
        <Layer x={750} y={384}>
          <circle cx={0} cy={-128} r={4} stroke-width={2.5} class="White" />
          <circle cx={0} cy={-64} r={4} stroke-width={2.5} class="White" />
          <line x1={-12} x2={12} y1={0} y2={0} stroke-width={5} class="Yellow" />
          <circle cx={0} cy={64} r={4} stroke-width={2.5} class="White" />
          <circle cx={0} cy={128} r={4} stroke-width={2.5} class="White" />
        </Layer>

        <Layer x={750} y={384}>
          <path
            d="M10,0 L0,-16 L-10,0"
            transform={this.deviationPxSub.derive((deviationPx) => `translate(0 ${Math.max(-128, deviationPx)})`)}
            class="Magenta rounded"
            stroke-width={2.5}
            visibility={this.deviationUpperVisibleSub.derive(this.visibilityFn)}
          />
          <path
            d="M-10,0 L0,16 L10,0"
            transform={this.deviationPxSub.derive((deviationPx) => `translate(0 ${Math.min(128, deviationPx)})`)}
            class="Magenta rounded"
            stroke-width={2.5}
            visibility={this.deviationLowerVisibleSub.derive(this.visibilityFn)}
          />
        </Layer>
      </g>
    );
  }
}

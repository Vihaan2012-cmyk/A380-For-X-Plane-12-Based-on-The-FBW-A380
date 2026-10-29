// Copyright (c) 2024 FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0

import { BusSignal, View, Bus, UI, ViewNode } from 'xplane-js/ui';
import { Arinc429Word, FmsOansData } from '@flybywiresim/fbw-sdk';
import { RopRowOansSimVars } from '../../MsfsAvionicsCommon/providers';

export interface RwyAheadAdvisoryProps {
  bus: Bus;
}

export class RwyAheadAdvisory extends View<RwyAheadAdvisoryProps> {
  private readonly sub = this.props.bus.inbox<RopRowOansSimVars & FmsOansData>();

  private readonly ndRwyAheadQfu = BusSignal.from(this.sub.topic('ndRwyAheadQfu').distinct(), '');

  private readonly oansWord1Raw = BusSignal.from(this.sub.topic('oansWord1Raw').distinct(), 0);

  private readonly flagDisplay = this.oansWord1Raw.derive((word) => {
    const w = new Arinc429Word(word);
    return w.bitValueOr(11, false) ? 'block' : 'none';
  });

  afterRender(node: ViewNode) {
    super.afterRender(node);
  }

  render(): ViewNode | null {
    return (
      <g style={{ display: this.flagDisplay }}>
        <rect
          x="273"
          y="209"
          width="210"
          height="40"
          stroke="yellow"
          stroke-width="3"
          fill="black"
          style="RwyAheadAnimation"
        />
        <text
          x={378}
          y={241}
          class="FontLarge Yellow MiddleAlign RwyAheadAnimation TextOutline"
          style="stroke-width: 0.25mm;"
        >
          {this.ndRwyAheadQfu}
        </text>
      </g>
    );
  }
}

// @ts-strict-ignore
// Copyright (c) 2021-2023 FlyByWire Simulations
//
// SPDX-License-Identifier: GPL-3.0

import { UI, View, ViewNode, Bus, Readable } from 'xplane-js/ui';
import { EfisNdMode } from '@flybywiresim/fbw-sdk';

export interface LubberLineProps {
  bus: Bus;

  visible: Readable<boolean>;
  ndMode: Readable<EfisNdMode>;
  rotation: Readable<number>;
  colorClass?: 'Yellow' | 'Magenta';
}

export class LubberLine extends View<LubberLineProps> {
  private readonly transform = this.props.rotation.derive((it) => `rotate(${it} 384 626)`);

  render(): ViewNode | null {
    return (
      <g visibility={this.props.visible.derive((it) => (it ? 'inherit' : 'hidden'))} transform={this.transform}>
        <line
          x1={384}
          y1={this.props.ndMode.derive((mode) => (mode === EfisNdMode.ARC ? 108 : 116))}
          x2={384}
          y2={this.props.ndMode.derive((mode) => (mode === EfisNdMode.ARC ? 148 : 152))}
          class="shadow"
          stroke-width={5.5}
          stroke-linejoin="round"
          stroke-linecap="round"
        />
        <line
          x1={384}
          y1={this.props.ndMode.derive((mode) => (mode === EfisNdMode.ARC ? 108 : 116))}
          x2={384}
          y2={this.props.ndMode.derive((mode) => (mode === EfisNdMode.ARC ? 148 : 152))}
          class={this.props.colorClass ?? 'Yellow'}
          stroke-width={5}
          stroke-linejoin="round"
          stroke-linecap="round"
        />
      </g>
    );
  }
}

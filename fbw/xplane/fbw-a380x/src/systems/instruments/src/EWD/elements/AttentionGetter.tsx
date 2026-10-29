// Copyright (c) 2024-2025 FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0
import { View, Bus, UI, Derived, Readable } from 'xplane-js/ui';

interface AttentionGetterProps {
  bus: Bus;
  x: number;
  y: number;
  engine: number;
  active: Readable<boolean>;
  normal: Readable<boolean>;
  abnormal: Readable<boolean>;
}

export class AttentionGetter extends View<AttentionGetterProps> {
  private readonly visible = Derived.from(
    ([active, normal, abnormal]) => active && (normal || abnormal),
    this.props.active,
    this.props.normal,
    this.props.abnormal,
  );

  render() {
    return (
      <g
        id={`attention-getter-${this.props.engine}`}
        visibility={this.visible.derive((it) => (it ? 'inherit' : 'hidden'))}
      >
        <path
          class={{
            WhiteLine: this.props.normal,
            AmberLine: this.props.abnormal,
          }}
          d={`m ${this.props.x - 74} ${this.props.y - 13} l 0,-72 l 162,0 l 0,72`}
        />
        <path
          class={{
            WhiteLine: this.props.normal,
            AmberLine: this.props.abnormal,
          }}
          d={`m ${this.props.x - 74} ${this.props.y + 168} l 0,72 l 162,0 l 0,-72`}
        />
      </g>
    );
  }
}

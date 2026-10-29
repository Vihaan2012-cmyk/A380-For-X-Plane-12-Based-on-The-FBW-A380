// Copyright (c) 2024-2025 FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0
import { View, Readable, ViewNode, UI, Bus } from 'xplane-js/ui';
import { AttentionGetter } from './AttentionGetter';
import { EGT } from './EGT';
import { N1 } from './N1';
import { ThrustGauge } from './ThrustGauge';

interface EngineGaugeProps {
  bus: Bus;
  x: number;
  y: number;
  engine: number;
  active: Readable<boolean>;
  n1Degraded: Readable<boolean>;
  engineNormalAttentionGettingBox: Readable<boolean>;
  engineAbnormalAttentionGettingBox: Readable<boolean>;
}

export class EngineGauge extends View<EngineGaugeProps> {
  public afterRender(node: ViewNode): void {
    super.afterRender(node);
  }

  render() {
    return (
      <g id={`Engine-Gauge-${this.props.engine}`}>
        <AttentionGetter
          bus={this.props.bus}
          x={this.props.x}
          y={this.props.y}
          engine={this.props.engine}
          active={this.props.active}
          normal={this.props.engineNormalAttentionGettingBox}
          abnormal={this.props.engineAbnormalAttentionGettingBox}
        />
        <ThrustGauge
          bus={this.props.bus}
          x={this.props.x + 2}
          y={this.props.y - 3}
          engine={this.props.engine}
          active={this.props.active}
          n1Degraded={this.props.n1Degraded}
        />
        <N1
          bus={this.props.bus}
          x={this.props.x}
          y={this.props.y + 65}
          engine={this.props.engine}
          active={this.props.active}
          n1Degraded={this.props.n1Degraded}
        />
        <EGT
          bus={this.props.bus}
          x={this.props.x + 4}
          y={this.props.y + 212}
          engine={this.props.engine}
          active={this.props.active}
        />
      </g>
    );
  }
}

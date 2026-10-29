//  Copyright (c) 2024-2026 FlyByWire Simulations
//  SPDX-License-Identifier: GPL-3.0

import {
  ViewProps,
  View,
  UI,
  Readable,
  ReadableSet,
  ClassToggles,
  ViewNode,
} from 'xplane-js/ui';

interface ShapeProps extends ViewProps {
  class?: string | Readable<string> | ReadableSet<string> | ClassToggles;
  color?: string | Readable<string>;
}

export class TriangleDown extends View<ShapeProps> {
  render(): ViewNode {
    return (
      <svg class={this.props.class} height="15" width="15">
        <polygon points="0,0 15,0 7.5,15" style={{ fill: this.props.color ?? 'white' }} />
      </svg>
    );
  }
}

export class TriangleUp extends View<ShapeProps> {
  render(): ViewNode {
    return (
      <svg class={this.props.class} height="15" width="15">
        <polygon points="7.5,0 15,15 0,15" style={{ fill: this.props.color ?? 'white' }} />
      </svg>
    );
  }
}

export class TriangleLeft extends View<ShapeProps> {
  render(): ViewNode {
    return (
      <svg height="15" width="15">
        <polygon points="0,7.5 15,15 15,0" style="fill:white" />
      </svg>
    );
  }
}

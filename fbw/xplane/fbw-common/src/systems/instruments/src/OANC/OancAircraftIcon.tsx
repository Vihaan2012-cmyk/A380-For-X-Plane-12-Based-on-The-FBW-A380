// Copyright (c) 2023-2024 FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0

import {
  View,
  UI,
  Derived,
  DerivedReadable,
  Readable,
  Bond,
  ViewNode,
} from 'xplane-js/ui';

export interface OancAircraftIconProps {
  isVisible: Readable<boolean>;
  x: Readable<number>;
  y: Readable<number>;
  rotation: Readable<number>;
}

export class OancAircraftIcon extends View<OancAircraftIconProps> {
  private svgRef = UI.ref<SVGSVGElement>();

  private readonly subscriptions: (Bond | DerivedReadable<any>)[] = [];

  afterRender() {
    this.subscriptions.push(
      this.props.isVisible.watch((isVisible) => {
        this.svgRef.target.style.visibility = isVisible ? 'inherit' : 'hidden';
      }, true),
      Derived.from(this.props.x, this.props.y, this.props.rotation).watch(([x, y, rotation]) => {
        this.svgRef.target.style.transform = `translate(${x - 45}px, ${y - 39.625}px) rotate(${rotation}deg)`;
      }),
    );
  }

  destroy() {
    for (const subscription of this.subscriptions) {
      subscription.destroy();
    }
  }

  render(): ViewNode | null {
    return (
      <svg
        ref={this.svgRef}
        class="oanc-svg"
        viewBox="0 0 90 70.25"
        style="position: absolute; width: 90px; height: 79.25px;"
      >
        <path
          class="oanc-airplane-shadow"
          stroke-width={8}
          stroke-linecap="round"
          d="M 4 29.5 H 86 m -41 -29.5 v 70.25 m -11.5 -9.75 h 23.5"
        />
        <path
          class="oanc-airplane"
          stroke-width={5}
          stroke-linecap="round"
          d="M 4 29.5 H 86 m -41 -29.5 v 70.25 m -11.5 -9.75 h 23.5"
        />
      </svg>
    );
  }
}

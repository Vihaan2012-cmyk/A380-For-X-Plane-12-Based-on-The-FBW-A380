// @ts-strict-ignore
//  Copyright (c) 2024-2025 FlyByWire Simulations
//  SPDX-License-Identifier: GPL-3.0

import {
  ViewProps,
  View,
  UI,
  Signal,
  Readable,
  Readables,
  Bond,
  ViewNode,
} from 'xplane-js/ui';

export enum MouseCursorColor {
  Yellow,
  Magenta,
}

interface MouseCursorProps extends ViewProps {
  side: Readable<'CAPT' | 'FO'>;
  isDoubleScreenMfd?: boolean;
  visible?: Signal<boolean>;
  color?: Readable<MouseCursorColor> | MouseCursorColor;
}

export class MouseCursor extends View<MouseCursorProps> {
  private readonly subs: Bond[] = [];

  private readonly divRef = UI.ref<HTMLSpanElement>();

  private readonly color: Readable<MouseCursorColor> = Readables.toReadable(
    this.props.color ?? MouseCursorColor.Yellow,
    true,
  );

  private readonly fillColor = this.color.derive((c) => (c === MouseCursorColor.Magenta ? '#ff94ff' : '#ffff00'));

  private readonly rotation = this.props.side.derive((side) => `rotate(${side === 'FO' ? 90 : 0},40,40)`);

  private hideTimer: ReturnType<typeof setTimeout> | undefined = undefined;

  updatePosition(x: number, y: number) {
    const xOffset = this.props.side.get() === 'FO' && this.props.isDoubleScreenMfd ? x - 40 - 878 : x - 40;
    this.divRef.target.style.left = `${xOffset}px`;
    this.divRef.target.style.top = `${y - 40}px`;

    if (this.hideTimer) {
      clearTimeout(this.hideTimer);
    }
    this.hideTimer = setTimeout(() => this.hide(), 5000);
  }

  show() {
    this.hide();
  }

  hide() {
    this.divRef.target.style.display = 'none';

    if (this.hideTimer) {
      clearTimeout(this.hideTimer);
    }
  }

  afterRender(node: ViewNode): void {
    super.afterRender(node);

    if (this.props.visible) {
      this.subs.push(this.props.visible.watch((vis) => (vis ? this.show() : this.hide()), true));
    }

    this.subs.push(this.fillColor, this.rotation);
  }

  render(): ViewNode {
    return (
      <div ref={this.divRef} class="mfd-mouse-cursor" style="display: none">
        <svg width="80" height="80" xmlns="http://www.w3.org/2000/svg">
          <g transform={this.rotation}>
            <polyline points="0,0 40,35 80,0" style={{ fill: 'none', stroke: this.fillColor, 'stroke-width': '3' }} />
            <line x1="40" y1="39" x2="40" y2="41" style={{ stroke: this.fillColor, 'stroke-width': '2' }} />
            <line x1="39" y1="40" x2="41" y2="40" style={{ stroke: this.fillColor, 'stroke-width': '2' }} />
            <polyline points="0,80 40,45 80,80" style={{ fill: 'none', stroke: this.fillColor, 'stroke-width': '3' }} />
          </g>
        </svg>
      </div>
    );
  }

  destroy(): void {
    for (const s of this.subs) {
      s.destroy();
    }

    super.destroy();
  }
}

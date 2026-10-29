// Copyright (c) 2026 FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0

import { ViewProps, View, UI, Readable, ViewNode } from 'xplane-js/ui';
import { Vars } from 'xplane-js/data';

export function fcuKey(event: string, value = 0) {
  Vars.write(`K:${event}`, 'number', value);
}

interface FcuBkupLightProps extends ViewProps {
  label: string;
  on: Readable<boolean>;
  onClick: () => void;
  labelBelow?: boolean;
}

export class FcuBkupLight extends View<FcuBkupLightProps> {
  private readonly ref = UI.ref<HTMLDivElement>();

  public afterRender(node: ViewNode): void {
    super.afterRender(node);
    this.ref.target.addEventListener('click', () => this.props.onClick());
  }

  render(): ViewNode {
    return (
      <div class={{ 'mfd-fcubkup-light-group': true, 'label-below': !!this.props.labelBelow }}>
        <div ref={this.ref} class={{ 'mfd-fcubkup-light': true, wide: !!this.props.labelBelow }}>
          <div class={{ 'mfd-fcubkup-bars': true, on: this.props.on }}>
            <div />
            <div />
            <div />
          </div>
          <div class="mfd-fcubkup-light-label" style={this.props.labelBelow ? 'display: none' : ''}>
            {this.props.label}
          </div>
        </div>
        <div class="mfd-fcubkup-light-caption" style={this.props.labelBelow ? '' : 'display: none'}>
          {this.props.label}
        </div>
      </div>
    );
  }
}

interface FcuBkupWindowProps extends ViewProps {
  left: number;
  top?: number;
  width?: number;
  text: Readable<string>;
  unit: Readable<string>;
  inc: string;
  dec: string;
}

export class FcuBkupWindow extends View<FcuBkupWindowProps> {
  private readonly ref = UI.ref<HTMLDivElement>();

  private readonly onWheel = (e: WheelEvent) => {
    e.preventDefault();
    const clicks = Math.max(1, Math.round(Math.abs(e.deltaY) / 100));
    for (let i = 0; i < clicks; i++) {
      fcuKey(e.deltaY < 0 ? this.props.inc : this.props.dec);
    }
  };

  public afterRender(node: ViewNode): void {
    super.afterRender(node);
    this.ref.target.addEventListener('wheel', this.onWheel);
  }

  render(): ViewNode {
    return (
      <div
        ref={this.ref}
        class="mfd-fcubkup-window"
        style={`left: ${this.props.left}px; top: ${this.props.top ?? 305}px; width: ${this.props.width ?? 150}px;`}
      >
        <span class="mfd-fcubkup-window-value">{this.props.text}</span>
        <span class="mfd-fcubkup-window-unit">{this.props.unit}</span>
      </div>
    );
  }
}

//  Copyright (c) 2024-2025 FlyByWire Simulations
//  SPDX-License-Identifier: GPL-3.0

import { ViewProps, View, UI, Readable, Bond, ViewNode } from 'xplane-js/ui';

export interface SurvStatusItemProps extends ViewProps {
  label: string;
  active: Readable<boolean>;
  failed: Readable<boolean>;
  sys: string;
  style?: string;
  onChanged?(val: boolean): void;
}

export class SurvStatusItem extends View<SurvStatusItemProps> {
  // Make sure to collect all subscriptions here, otherwise page navigation doesn't work.
  private readonly subs = [] as Bond[];

  private readonly label = this.props.active.derive((active) => `${this.props.label} ${active ? this.props.sys : 'OFF'}`);

  public afterRender(node: ViewNode): void {
    super.afterRender(node);

    this.subs.push(this.label);
  }

  public destroy(): void {
    // Destroy all subscriptions to remove all references to this instance.
    this.subs.forEach((x) => x.destroy());

    super.destroy();
  }

  public render(): ViewNode {
    return (
      <div
        style={this.props.style}
        class={{
          'mfd-surv-status-item': true,
          active: this.props.active,
          failed: this.props.failed,
        }}
      >
        {this.label}
      </div>
    );
  }
}

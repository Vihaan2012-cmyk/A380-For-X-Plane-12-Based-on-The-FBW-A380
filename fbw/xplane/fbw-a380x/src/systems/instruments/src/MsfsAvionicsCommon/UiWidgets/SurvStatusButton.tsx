//  Copyright (c) 2024-2025 FlyByWire Simulations
//  SPDX-License-Identifier: GPL-3.0

import { ViewProps, View, UI, Readable, Bond, ViewNode } from 'xplane-js/ui';

export interface SurvStatusButtonProps extends ViewProps {
  label: string;
  active: Readable<boolean>;
  onClick?: () => void;
}

/*
 * Button for MFD pages. If menuItems is set, a dropdown menu will be displayed when button is clicked
 */
export class SurvStatusButton extends View<SurvStatusButtonProps> {
  // Make sure to collect all subscriptions here, otherwise page navigation doesn't work.
  private readonly subs = [] as Bond[];

  private readonly buttonRef = UI.ref<HTMLSpanElement>();

  private clickHandler(): void {
    if (this.props.onClick) {
      this.props.onClick();
    }
  }

  public afterRender(node: ViewNode): void {
    super.afterRender(node);

    this.buttonRef.target.addEventListener('click', this.clickHandler.bind(this));
  }

  public destroy(): void {
    // Destroy all subscriptions to remove all references to this instance.
    this.subs.forEach((x) => x.destroy());

    this.buttonRef.target.removeEventListener('click', () => this.clickHandler.bind(this));

    super.destroy();
  }

  public render(): ViewNode {
    return (
      <div ref={this.buttonRef} class="mfd-surv-status-button">
        <div style="padding-left: 7px; padding-right: 7px; height: 50%; background-color: black; display: flex; flex-direction: column; justify-content: space-evenly;">
          <div class={{ 'mfd-surv-status-indicator': true, active: this.props.active }}></div>
          <div class={{ 'mfd-surv-status-indicator': true, active: this.props.active }}></div>
          <div class={{ 'mfd-surv-status-indicator': true, active: this.props.active }}></div>
        </div>
        <p class="mfd-surv-status-button-label">{this.props.label}</p>
      </div>
    );
  }
}

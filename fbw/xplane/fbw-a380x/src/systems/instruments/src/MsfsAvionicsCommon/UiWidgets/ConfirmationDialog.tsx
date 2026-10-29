//  Copyright (c) 2024-2025 FlyByWire Simulations
//  SPDX-License-Identifier: GPL-3.0

import { ViewProps, View, UI, Readable, Bond, ViewNode } from 'xplane-js/ui';
import { Button } from './Button';

interface ConfirmationDialogProps extends ViewProps {
  visible: Readable<boolean>;
  cancelAction: () => void;
  confirmAction: () => void;
  contentContainerStyle?: string;
  amberLabel?: boolean;
}

/*
 * Popup dialog for actions needing confirmation (e.g. DERATED TO)
 */
export class ConfirmationDialog extends View<ConfirmationDialogProps> {
  // Make sure to collect all subscriptions here, otherwise page navigation doesn't work.
  private readonly subs = [] as Bond[];

  private readonly topRef = UI.ref<HTMLDivElement>();

  afterRender(node: ViewNode): void {
    super.afterRender(node);

    this.subs.push(
      this.props.visible.watch((val) => {
        if (this.topRef.peek()) {
          this.topRef.target.style.display = val ? 'block' : 'none';
        }
      }, true),
    );
  }

  public destroy(): void {
    // Destroy all subscriptions to remove all references to this instance.
    this.subs.forEach((x) => x.destroy());

    super.destroy();
  }

  render(): ViewNode {
    return (
      <div ref={this.topRef} style="position: relative;">
        <div class="mfd-dialog" style={`${this.props.contentContainerStyle ?? ''}`}>
          <div class="mfd-dialog-title">
            <span class={`mfd-label ${this.props.amberLabel ? 'amber' : ''}`}>{this.props.children}</span>
          </div>
          <div class="mfd-dialog-buttons">
            <Button label="CANCEL" onClick={() => this.props.cancelAction()} />
            <Button label="CONFIRM *" onClick={() => this.props.confirmAction()} buttonStyle="padding-right: 6px;" />
          </div>
        </div>
      </div>
    );
  }
}

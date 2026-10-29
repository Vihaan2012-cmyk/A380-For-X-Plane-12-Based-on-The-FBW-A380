// Copyright (c) 2025-2026 FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0

import { View, UI, Bond, Signal, ViewNode } from 'xplane-js/ui';
import { AtccomMfdPageProps } from '../../MFD';
import { Button } from '../../../MsfsAvionicsCommon/UiWidgets/Button';

export class AtccomFooter extends View<AtccomMfdPageProps> {
  // Make sure to collect all subscriptions here, otherwise page navigation doesn't work.
  protected readonly subs = [] as Bond[];

  private readonly buttonRef = UI.ref<Button>();

  private readonly messageRef = UI.ref<HTMLSpanElement>();

  private readonly messageToBeCleared = Signal.of<boolean>(false);

  public afterRender(node: ViewNode): void {
    super.afterRender(node);

    if (this.props.atcService) {
      this.subs.push(
        this.props.atcService.atcErrors.watch((_, __, ___, arr) => {
          const ind = arr.findIndex((el) => !el.cleared);

          if (ind > -1 && this.messageRef.peek()) {
            this.messageToBeCleared.set(true);
            this.messageRef.target.textContent = arr[ind].messageText;

            if (arr[ind].backgroundColor === 'white') {
              this.messageRef.target.style.backgroundColor = '#ffffff';
            } else if (arr[ind].backgroundColor === 'cyan') {
              this.messageRef.target.style.backgroundColor = '#00ffff';
            } else if (arr[ind].backgroundColor === 'amber') {
              this.messageRef.target.style.backgroundColor = '#e68000';
            }
          } else {
            this.messageToBeCleared.set(false);
            this.messageRef.target.textContent = '';
            this.messageRef.target.style.backgroundColor = 'none';
          }
        }, true),
      );
    }
  }

  public destroy(): void {
    // Destroy all subscriptions to remove all references to this instance.
    this.subs.forEach((x) => x.destroy());

    super.destroy();
  }

  render(): ViewNode {
    return (
      <div class="mfd-footer">
        <Button
          ref={this.buttonRef}
          buttonStyle="width: 100px;"
          label={
            <span>
              CLEAR
              <br />
              INFO
            </span>
          }
          onClick={() => {
            if (this.messageToBeCleared.get()) {
              this.props.atcService.clearLatestAtcErrorMessage();
            }
          }}
        />
        <div class="mfd-footer-message-area">
          <span ref={this.messageRef} />
        </div>
      </div>
    );
  }
}

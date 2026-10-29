// Copyright (c) 2026 FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0

import { Bus, Outbox, Signal, Readable, Bond } from 'xplane-js/ui';
import { PopupControlEvents } from './PopupControlEvents';
import { PopupDefinition } from './PopupTypes';
import { PopupEvents } from './PopupEvents';

export class PopupTransporter {
  private readonly publisher: Outbox<PopupControlEvents>;
  private readonly isRequested = Signal.of(false);

  private isVisible = false;

  private sub?: Bond;

  public constructor(
    bus: Bus,
    private readonly definition: PopupDefinition,
  ) {
    this.publisher = bus.outbox();

    this.isRequested.watch(this.onRequestChanged.bind(this));

    bus
      .inbox<PopupEvents>()
      .topic('popup_visible_uuid_changed')
      .listen((uuid) => {
        if (uuid === this.definition.uuid) {
          this.isVisible = true;
        } else if (this.isVisible) {
          this.isVisible = false;
          // reset the request so it will notify next time
          this.isRequested.set(false);
        }
      });
  }

  private onRequestChanged(isVisible: boolean): void {
    if (isVisible) {
      this.publisher.post('popup_enqueue_popup', this.definition, true, false);
    } else {
      this.publisher.post('popup_dequeue_popup', this.definition.uuid, true, false);
    }
  }

  public set(isVisible: boolean): void {
    this.isRequested.set(isVisible);
  }

  public bind(isVisible: Readable<boolean>): void {
    this.sub?.destroy();

    this.sub = isVisible.forward(this.isRequested);
  }

  public destroy(): void {
    this.sub?.destroy();
  }
}

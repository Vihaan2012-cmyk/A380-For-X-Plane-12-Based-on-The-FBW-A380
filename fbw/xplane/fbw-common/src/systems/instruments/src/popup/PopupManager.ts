// Copyright (c) 2026 FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0

import { ClockTopics, Bus, Outbox, Signal, Readable } from 'xplane-js/ui';
import { PopupControlEvents } from '../../../shared/src/popup/PopupControlEvents';
import { PopupDefinition } from '../../../shared/src/popup/PopupTypes';
import { PopupEvents } from '../../../shared/src/popup/PopupEvents';

export class PopupManager {
  private readonly publisher: Outbox<PopupEvents>;

  private readonly queue: PopupDefinition[] = [];

  private readonly _visiblePopup = Signal.of<PopupDefinition | undefined>(undefined);
  public readonly visiblePopup: Readable<PopupDefinition | undefined> = this._visiblePopup;

  private readonly _visiblePopupTimeRemaining = Signal.of<number | undefined>(undefined);
  public readonly visiblePopupTimeRemaining: Readable<number | undefined> = this._visiblePopupTimeRemaining;

  private lastSimDuration = 0;

  public constructor(bus: Bus) {
    this.publisher = bus.outbox();

    this._visiblePopup.watch(this.onVisiblePopupChanged.bind(this));

    const sub = bus.inbox<ClockTopics & PopupControlEvents>();

    sub.topic('popup_dequeue_popup').listen(this.onDequeue.bind(this));
    sub.topic('popup_enqueue_popup').listen(this.onEnqueue.bind(this));

    sub.topic('activeSimDuration').listen(this.onUpdate.bind(this));
  }

  private onVisiblePopupChanged(definition: PopupDefinition | undefined): void {
    this._visiblePopupTimeRemaining.set(definition?.timeout);

    this.publisher.post('popup_visible_uuid_changed', definition?.uuid, true, true);
  }

  private onDequeue(uuid: string): void {
    const index = this.queue.findIndex((p) => p.uuid === uuid);

    if (index >= 0) {
      this.queue.splice(index, 1);
    }

    this._visiblePopup.set(this.queue[0]);
  }

  private onEnqueue(definition: PopupDefinition): void {
    const index = this.queue.findIndex((p) => p.uuid === definition.uuid);

    if (index >= 0) {
      this.queue[index] = definition;
    } else {
      this.queue.push(definition);
    }

    this._visiblePopup.set(this.queue[0]);
  }

  private onUpdate(simDuration: number): void {
    const dt = this.lastSimDuration > 0 ? simDuration - this.lastSimDuration : 0;
    this.lastSimDuration = simDuration;

    let remaining = this._visiblePopupTimeRemaining.get();
    if (remaining !== undefined && remaining > 0) {
      remaining = Math.max(0, remaining - dt);

      if (remaining > 0) {
        this._visiblePopupTimeRemaining.set(remaining);
      } else {
        this.queue.shift();

        this._visiblePopup.set(this.queue[0]);
      }
    }
  }
}

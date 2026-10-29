// Copyright (c) 2026 FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0

import { Bus, Outbox, Readable } from 'xplane-js/ui';
import { PopupControlEvents } from '../../../shared/src/popup/PopupControlEvents';
import { PopupDefinition, PopupUuid } from '../../../shared/src/popup/PopupTypes';
import { Bridge, Channels, KeyCodes, openChannel } from 'xplane-js/data';

export class TodPauseManager {
  private readonly listener: Channels.Channel;

  private readonly publisher: Outbox<PopupControlEvents>;

  private readonly isTodPauseVisible: Readable<boolean>;

  public constructor(bus: Bus, currentPopup: Readable<PopupDefinition | undefined>) {
    this.publisher = bus.outbox();

    this.listener = openChannel('toolbar', this.onListenerReady.bind(this));

    this.isTodPauseVisible = currentPopup.derive((v) => v?.uuid === PopupUuid.TodPause);
  }

  private onListenerReady(): void {
    // OR USE A:SIM DISABLED but that will not make the toolbar work
    // OR Use the regular K:PAUSE_SET event and only intercept the key
    this.listener.on('SetActivePauseEnabled', (isPaused: boolean) => {
      if (!isPaused && this.isTodPauseVisible.get()) {
        this.hideTodPause();
      }
    });

    this.isTodPauseVisible.watch((isVisible) => {
      if (isVisible) {
        window.document.addEventListener('keydown', this.handleKeyDown);
        Bridge.call('sim.pause', true);
      } else {
        window.document.removeEventListener('keydown', this.handleKeyDown);
        Bridge.call('sim.pause', false);
      }
    });
  }

  private hideTodPause(): void {
    this.publisher.post('popup_dequeue_popup', PopupUuid.TodPause);
  }

  private handleKeyDown = (e: KeyboardEvent): void => {
    if (e.keyCode === KeyCodes.KEY_P && this.isTodPauseVisible.get()) {
      this.hideTodPause();
    }
  };
}

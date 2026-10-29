// Copyright (c) 2026 FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0

import { SimClock, Bus, UI, HostedDevice } from 'xplane-js/ui';
import { Popup } from './Popup';
import { PopupManager } from './PopupManager';
import { TodPauseManager } from './TodPauseManager';
import { PanelInstrument, SessionPhase, SoundId } from 'xplane-js/data';

export class PopupFsInstrument implements HostedDevice {
  private readonly bus = new Bus();

  private readonly popupManager = new PopupManager(this.bus);

  private readonly todPauseManager = new TodPauseManager(this.bus, this.popupManager.visiblePopup);

  private readonly clock = new SimClock(this.bus);

  constructor(public readonly instrument: PanelInstrument) {
    this.doInit();
  }

  private doInit(): void {
    // Remove "instrument didn't load" text
    this.instrument.innerHTML = '';

    UI.mount(
      <Popup
        currentPopup={this.popupManager.visiblePopup}
        timeRemaining={this.popupManager.visiblePopupTimeRemaining}
      />,
      this.instrument,
    );

    this.clock.setup();
  }

  /**
   * A callback called when the instrument gets a frame update.
   */
  public onFrame(): void {
    this.clock.tick();
  }

  public onPanelEvent(_args: string[]): void {}

  onSessionChange(_oldState: SessionPhase, _newState: SessionPhase) {
    // noop
  }

  onFlightBegin() {
    // noop
  }

  onSoundDone(_soundEventId: SoundId) {
    // noop
  }
}

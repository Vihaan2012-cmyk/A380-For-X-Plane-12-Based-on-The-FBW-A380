// Copyright (c) 2023 FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0

import { BusSignal, Debouncer, Bus, Derived } from 'xplane-js/ui';
import { BaseExtrasSimVarEvents, NotificationManager, NotificationTheme } from '@flybywiresim/fbw-sdk';

/**
 * Monitors cockpit pushbuttons that may be written externally to ensure they are not "stuck" because
 * the external writer failed to set them back to 0.
 */
export class PushbuttonCheck {
  /** Maximum time in ms that TO CONF can be pressed before it is considered "stuck" */
  private static readonly TO_CONFIG_MAX_PRESS_TIME = 30000;

  private readonly sub = this.bus.inbox<BaseExtrasSimVarEvents>();

  private readonly fwcFlightPhase = BusSignal.from(null, 1);

  private readonly toConfButton = BusSignal.from(null, false);

  private readonly toConfTimer = new Debouncer();

  private readonly toConfButtonInCruise = Derived.from(
    ([toConf, phase]) => toConf && phase === 8,
    this.toConfButton,
    this.fwcFlightPhase,
  );

  private toConfMessageShown = false;

  constructor(
    private readonly bus: Bus,
    private readonly notification: NotificationManager,
  ) {}

  public connectedCallback(): void {
    this.toConfButtonInCruise.watch(this.onToConfigPushbutton.bind(this));

    this.fwcFlightPhase.rebind(this.sub.topic('fwc_flight_phase'));
    this.toConfButton.rebind(this.sub.topic('ecp_to_config_pushbutton'));
  }

  private onToConfigPushbutton(pressed: boolean): void {
    if (pressed && !this.toConfTimer.waiting() && !this.toConfMessageShown) {
      this.toConfTimer.arm(() => {
        this.toConfMessageShown = true;
        this.notification.showNotification({
          title: 'ECP Pushbutton Held',
          // eslint-disable-next-line max-len
          message:
            'The TO CONF pushbutton has been held for a long time!\n\nIf you have external hardware or software controlling this variable (L:A32NX_BTN_TOCONFIG), please check that it is setup to write the variable to 0 when the button is released.',
          theme: NotificationTheme.Tips,
        });
      }, PushbuttonCheck.TO_CONFIG_MAX_PRESS_TIME);
    } else if (!pressed) {
      this.toConfTimer.cancel();
    }
  }
}

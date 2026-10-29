// Copyright (c) 2024 FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0

import { BusSignal, Bus, PanelEventTopics, Derived, Signal } from 'xplane-js/ui';
import { KeypadEvents, SystemKeys } from './KeypadController';
import { TransponderEvents } from '../Data/TransponderPublisher';

export interface RmpMessageControlEvents {
  rmp_message_set: string;
  rmp_message_clear_message: string;
  rmp_message_set_squawk_message: boolean;
}

export interface RmpMessageDataEvents {
  rmp_message: string;
}

export class RmpMessageManager {
  private readonly sub = this.bus.inbox<PanelEventTopics & KeypadEvents & RmpMessageControlEvents & TransponderEvents>();
  private readonly pub = this.bus.outbox<RmpMessageDataEvents>();

  private readonly squawkMessageEnabled = BusSignal.from(this.sub.topic('rmp_message_set_squawk_message'), false);
  private readonly squawkCode = BusSignal.from(this.sub.topic('transponder_code_1'), 0);
  private readonly squawkAuto = BusSignal.from(this.sub.topic('transponder_auto_1'), false);
  private readonly squawkMessage = Derived.from(
    ([code, auto, enabled]) =>
      enabled ? `SQUAWK : ${code.toString(16).padStart(4, '0')}${auto ? '' : ' - STBY'}` : '',
    this.squawkCode,
    this.squawkAuto,
    this.squawkMessageEnabled,
  );

  private readonly displayedMessage = Signal.of('');
  private isBackgroundDisplayed = true;

  private readonly clearEvent = `RMP_${this.rmpIndex}_MSG_CLR_PRESSED`;

  constructor(
    private readonly bus: Bus,
    private readonly rmpIndex: 1 | 2 | 3,
  ) {
    this.sub.topic('rmp_message_set').listen((msg) => {
      this.isBackgroundDisplayed = false;
      this.displayedMessage.set(msg);
    });
    this.sub.topic('rmp_message_clear_message').listen((msg) => {
      if (this.displayedMessage.get() === msg) {
        this.clearMessage();
      }
    });
    this.squawkMessage.watch((msg) => {
      if (this.isBackgroundDisplayed) {
        this.displayedMessage.set(msg);
      }
    });

    this.sub.topic('keypad_system_key_pressed').listen((k) => {
      if (k === SystemKeys.MessageClear) {
        this.clearMessage();
      }
    });

    this.displayedMessage.watch((v) => this.pub.post('rmp_message', v));

    this.squawkMessageEnabled.watch((v) => console.log('squawkMessageEnabled', v));
  }

  private clearMessage(): void {
    this.isBackgroundDisplayed = true;
    this.displayedMessage.set(this.squawkMessage.get());
  }
}

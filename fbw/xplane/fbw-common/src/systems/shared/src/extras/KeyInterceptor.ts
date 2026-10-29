// Copyright (c) 2022-2025 FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0

import {
  Bus,
  KeyTopics,
  KeyControl,
  KeyPress,
  VarUnit,
  BusSignal,
} from 'xplane-js/ui';
import { NotificationManager, PilotSeat, PilotSeatEvents } from '@flybywiresim/fbw-sdk';
import { Vars } from 'xplane-js/data';

interface KeyInterceptDefinition {
  /** A handler to be called when this key is inercepted. Defaults to no handler. */
  handler?: (data: KeyPress) => void;
  /** Whether the event should be passed through to other consumers in the sim. Defaults to false. */
  passThrough?: boolean;
  /** Whether the event should be logged to the console. Defaults to false. */
  log?: boolean;
}

export type KeyInterceptDefinitions = [string, KeyInterceptDefinition][];

/**
 * This class is used to intercept the key events.
 * Subclasses should define the keys to be intercepted and their handlers in the {@link keys} property.
 */
export abstract class KeyInterceptor {
  private readonly keys: Map<string, KeyInterceptDefinition>;

  protected keyInterceptManager?: KeyControl;

  protected dialogVisible = false;

  private readonly sub = this.bus.inbox<PilotSeatEvents>();
  protected readonly pilotSeat = BusSignal.from(this.sub.topic('pilot_seat'), PilotSeat.Left);

  constructor(
    protected readonly bus: Bus,
    protected readonly notification: NotificationManager,
  ) {
    this.keys = new Map([
      ['MASTER_CAUTION_ACKNOWLEDGE', { handler: this.onMasterCautionAck.bind(this) }],
      ['MASTER_WARNING_ACKNOWLEDGE', { handler: this.onMasterWarningAck.bind(this) }],
      ...this.getExtraIntercepts(),
    ]);

    KeyControl.connect(this.bus).then((manager) => {
      this.keyInterceptManager = manager;
      this.registerIntercepts();
    });
    console.log('KeyInterceptor: Created');
  }

  protected abstract getExtraIntercepts(): KeyInterceptDefinitions;

  public startPublish(): void {
    console.log('KeyInterceptor: startPublish()');
  }

  private registerIntercepts() {
    for (const [key, config] of this.keys.entries()) {
      // we can assert keyInterceptManager as this is called in it's callback
      this.keyInterceptManager!.intercept(key, !!config.passThrough);
    }

    const subscriber = this.bus.inbox<KeyTopics>();
    subscriber.topic('key_intercept').listen(this.onKeyIntercepted.bind(this));
  }

  private onKeyIntercepted(data: KeyPress): void {
    const config = this.keys.get(data.key);
    if (!config) {
      return;
    }

    if (config.log) {
      console.log(`KeyInterceptor: ${data.key}`);
    }

    config.handler?.(data);
  }

  private togglePilotLocalVar(name: string, highTime = 500): void {
    const localVar = `L:${name}_${this.pilotSeat.get() === PilotSeat.Right ? 'R' : 'L'}`;
    Vars.write(localVar, VarUnit.Bool, true);
    setTimeout(() => Vars.write(localVar, VarUnit.Bool, false), highTime);
  }

  private onMasterCautionAck(): void {
    this.togglePilotLocalVar('PUSH_AUTOPILOT_MASTERCAUT');
  }

  private onMasterWarningAck(): void {
    this.togglePilotLocalVar('PUSH_AUTOPILOT_MASTERAWARN');
  }
}

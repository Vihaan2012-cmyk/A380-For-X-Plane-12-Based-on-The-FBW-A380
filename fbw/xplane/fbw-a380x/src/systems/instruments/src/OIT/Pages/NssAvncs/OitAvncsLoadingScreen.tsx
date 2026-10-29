//  Copyright (c) 2025 FlyByWire Simulations
//  SPDX-License-Identifier: GPL-3.0

import { NXDataStore } from '@flybywiresim/fbw-sdk';
import {
  ClockTopics,
  BusSignal,
  View,
  Bus,
  UI,
  Derived,
  Signal,
  Bond,
  ViewNode,
} from 'xplane-js/ui';
import { OitSimvars } from '../../OitSimvarPublisher';
import { Vars } from 'xplane-js/data';

interface OitAvncsLoadingScreenProps {
  readonly bus: Bus;
  readonly captOrFo: 'CAPT' | 'FO';
}

export class OitAvncsLoadingScreen extends View<OitAvncsLoadingScreenProps> {
  // Make sure to collect all subscriptions here, otherwise page navigation doesn't work.
  private readonly subscriptions = [] as Bond[];

  private readonly sub = this.props.bus.inbox<OitSimvars & ClockTopics>();

  private readonly ansuPowered = BusSignal.from(this.sub.topic('nssAnsu1Healthy'), false);

  /** in seconds */
  private readonly remainingStartupTime = Signal.of(24);
  private readonly totalStartupTime = Signal.of(24);

  private readonly progressBarFillWidth = Derived.from(
    ([remaining, total]) => (1 - remaining / total) * 40,
    this.remainingStartupTime,
    this.totalStartupTime,
  ).derive((t) => `${t}%`);

  private readonly screenHidden = Derived.from(
    ([powered, remaining]) => !powered || remaining <= 0,
    this.ansuPowered,
    this.remainingStartupTime,
  );

  public afterRender(node: ViewNode): void {
    super.afterRender(node);

    this.subscriptions.push(
      this.sub
        .topic('realTime')
        .limit(4)
        .listen(() => {
          // Update loading progress bar
          if (this.ansuPowered.get() && this.remainingStartupTime.get() > 0) {
            this.remainingStartupTime.set(Math.max(0, this.remainingStartupTime.get() - 0.25));
          }
        }),
      this.ansuPowered.watch((powered) => {
        // Aircraft Presets set L:A32NX_AIRCRAFT_PRESET_QUICK_MODE while a preset is
        // being applied: skip this loading bar's wait too, same convention as
        // OitDisplayUnit's own self-test.
        const quickMode = Vars.read('L:A32NX_AIRCRAFT_PRESET_QUICK_MODE', 'Bool');
        this.totalStartupTime.set(
          powered && !quickMode ? parseInt(NXDataStore.getLegacy('CONFIG_SELF_TEST_TIME', '12')) * 1.5 : 0,
        );
        this.remainingStartupTime.set(this.totalStartupTime.get());
      }, true),
    );

    this.subscriptions.push(this.progressBarFillWidth);
  }

  public destroy(): void {
    // Destroy all subscriptions to remove all references to this instance.
    for (const s of this.subscriptions) {
      s.destroy();
    }

    super.destroy();
  }

  render(): ViewNode {
    return (
      <>
        <div class={{ 'oit-flt-ops-loading-screen-container': true, hidden: this.screenHidden }}>
          <div class="LoadingTextTopLeft">The system is loading, please wait...</div>
          <div class="FbwTail" />
          <div class="LoadingProgressBarBackground" />
          <div
            class="LoadingProgressBarFill"
            style={{
              width: this.progressBarFillWidth,
            }}
          />
        </div>
      </>
    );
  }
}

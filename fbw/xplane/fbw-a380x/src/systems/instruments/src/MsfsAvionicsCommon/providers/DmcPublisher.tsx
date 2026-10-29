// Copyright (c) 2021-2024 FlyByWire Simulations
//
// SPDX-License-Identifier: GPL-3.0

import { Bus, VarSpec, VarPublisher, VarUnit, Signal } from 'xplane-js/ui';
import { Arinc429WordData, Arinc429RegisterSubject } from '@flybywiresim/fbw-sdk';
import { AdirsSimVars } from '../SimVarTypes';

export interface DmcLogicEvents {
  trueRefActive: boolean;
  heading: Arinc429WordData;
  track: Arinc429WordData;
}

export interface DmcDiscreteInputEvents {
  trueRefPushButton: boolean;
}

export type DmcEvents = DmcLogicEvents & DmcDiscreteInputEvents;

export class DmcPublisher extends VarPublisher<DmcDiscreteInputEvents> {
  private readonly irMaintWord = Arinc429RegisterSubject.createEmpty();

  private readonly magHeading = Arinc429RegisterSubject.createEmpty();

  private readonly trueHeading = Arinc429RegisterSubject.createEmpty();

  private readonly magTrack = Arinc429RegisterSubject.createEmpty();

  private readonly trueTrack = Arinc429RegisterSubject.createEmpty();

  private readonly trueRefPb = Signal.of(false);

  private readonly trueRefActive = Signal.of(false);

  constructor(private eventBus: Bus) {
    const simVars = new Map<keyof DmcDiscreteInputEvents, VarSpec>([
      // FIXME, per-side
      ['trueRefPushButton', { name: 'L:A32NX_PUSH_TRUE_REF', type: VarUnit.Bool }],
    ]);
    super(simVars, eventBus);
  }

  /** @inheritdoc */
  public begin(): void {
    const pub = this.eventBus.outbox<DmcLogicEvents>();

    this.trueRefActive.watch((v) => {
      pub.post('trueRefActive', v);
      this.handleHeading();
    }, true);

    const sub = this.eventBus.inbox<AdirsSimVars & DmcDiscreteInputEvents>();

    this.irMaintWord.watch(this.handleTrueRef.bind(this));
    this.trueRefPb.watch(this.handleTrueRef.bind(this), true);

    this.magHeading.watch(this.handleHeading.bind(this));
    this.magTrack.watch(this.handleHeading.bind(this));
    this.trueHeading.watch(this.handleHeading.bind(this));
    this.trueTrack.watch(this.handleHeading.bind(this));

    sub.topic('irMaintWordRaw').listen((v) => this.irMaintWord.setWord(v));
    sub
      .topic('trueRefPushButton')
      .distinct()
      .listen((v) => this.trueRefPb.set(v));
    sub.topic('magHeadingRaw').listen((v) => this.magHeading.setWord(v));
    sub.topic('magTrackRaw').listen((v) => this.magTrack.setWord(v));
    sub.topic('trueHeadingRaw').listen((v) => this.trueHeading.setWord(v));
    sub.topic('trueTrackRaw').listen((v) => this.trueTrack.setWord(v));

    super.begin();
  }

  private handleTrueRef(): void {
    // true ref is active when the PB is pressed or the ADIRU is at an extreme latitude
    // and the ADIRU must not be in ATT reversion mode
    const trueRequested = this.irMaintWord.get().bitValueOr(15, false) || this.trueRefPb.get();
    this.trueRefActive.set(trueRequested && !this.irMaintWord.get().bitValueOr(2, false));
  }

  private handleHeading(): void {
    const pub = this.eventBus.outbox<DmcLogicEvents>();

    pub.post('heading', this.trueRefActive.get() ? this.trueHeading.get() : this.magHeading.get());
    pub.post('track', this.trueRefActive.get() ? this.trueTrack.get() : this.magTrack.get());
  }
}

// Copyright (c) 2021-2024 FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0

import { getDisplayIndex } from '../HUD';

import {
  Arinc429LocalVarConsumerSubject,
  Arinc429RegisterSubject,
  Arinc429Word,
  MathUtils,
} from '@flybywiresim/fbw-sdk';
import {
  ClockTopics,
  BusSignal,
  Bus,
  Smoother,
  BoardDevice,
  Outbox,
  Bond,
} from 'xplane-js/ui';

import { HUDSimvars } from './HUDSimvarPublisher';
import { FcuEfisCpBusEvents } from '@shared/publishers/EfisCpBusPublisher';

export interface Arinc429Values {
  pitchAr: Arinc429Word;
  rollAr: Arinc429Word;

  /** The "displayed altitude" in feet. It's baro corrected for QFE/QNH modes, otherwise pressure alt. */
  altitudeAr: Arinc429Word;

  groundTrackAr: Arinc429Word;
  headingAr: Arinc429Word;
  speedAr: Arinc429Word;
  machAr: Arinc429Word;
  vs: Arinc429Word;
  gs: Arinc429Word;
  chosenRa: Arinc429Word;
  fpa: Arinc429Word;
  da: Arinc429Word;
  landingElevation: Arinc429Word;
  staticPressure: Arinc429Word;
  fmEisDiscreteWord1Raw: number;
  fmEisDiscreteWord2Raw: number;
  fmMdaRaw: number;
  fmDhRaw: number;
  fmTransAltRaw: number;
  fmTransLvlRaw: number;
  lgciuDiscreteWord1: Arinc429Word;
}
export class ArincValueProvider implements BoardDevice {
  private readonly sub = this.bus.inbox<FcuEfisCpBusEvents & ClockTopics & HUDSimvars>();

  private roll = new Arinc429Word(0);

  private pitch = new Arinc429Word(0);

  private groundTrack = new Arinc429Word(0);

  private heading = new Arinc429Word(0);

  private speed = new Arinc429Word(0);

  /** Displayed altitude. */
  private readonly altitude = Arinc429RegisterSubject.createEmpty();

  private readonly unfilteredAltitude = Arinc429RegisterSubject.createEmpty();

  private readonly baroCorrectedAltitude = Arinc429LocalVarConsumerSubject.create(null);

  private readonly pressureAltitude = Arinc429LocalVarConsumerSubject.create(this.sub.topic('pressureAltitude'));

  private readonly altitudeFilter = new Smoother(0.3, 0, 1);
  private lastAltitudeFilterTime = -1;

  private readonly fcuEisDiscreteWord2 = Arinc429LocalVarConsumerSubject.create(null);

  private mach = new Arinc429Word(0);

  private vsInert = new Arinc429Word(0);

  private vsBaro = new Arinc429Word(0);

  private groundSpeed = new Arinc429Word(0);

  private radioAltitude1 = new Arinc429Word(0);

  private radioAltitude2 = new Arinc429Word(0);

  private radioAltitude3 = new Arinc429Word(0);

  private fpa = new Arinc429Word(0);

  private da = new Arinc429Word(0);

  private ownLandingElevation = new Arinc429Word(0);

  private oppLandingElevation = new Arinc429Word(0);

  private staticPressure = new Arinc429Word(0);

  private lgciuDiscreteWord1 = new Arinc429Word(0);

  private readonly fm1Healthy = BusSignal.from(null, 0);

  private readonly fm2Healthy = BusSignal.from(null, 0);

  private readonly fm1Subs: Bond[] = [];

  private readonly fm2Subs: Bond[] = [];

  // each alti source should have a pipe, and only one pipe should be unpaused at a time
  private readonly baroAltitudePipe = this.baroCorrectedAltitude.forward(this.unfilteredAltitude, true);
  private readonly pressureAltitudePipe = this.pressureAltitude.forward(this.unfilteredAltitude, true);

  constructor(private readonly bus: Bus) {}

  /** @inheritdoc */
  public setup(): void {
    const isFo = getDisplayIndex() === 2;

    this.fcuEisDiscreteWord2.setConsumer(
      this.sub.topic(isFo ? 'fcu_efis_r_discrete_word_2' : 'fcu_efis_l_discrete_word_2'),
    );

    this.baroCorrectedAltitude.setConsumer(this.sub.topic(isFo ? 'baroCorrectedAltitude2' : 'baroCorrectedAltitude1'));

    const publisher = this.bus.outbox<Arinc429Values>();
    const subscriber = this.bus.inbox<HUDSimvars>();

    subscriber.topic('pitch').listen((p) => {
      this.pitch = new Arinc429Word(p);
      publisher.post('pitchAr', this.pitch);
    });
    subscriber.topic('roll').listen((p) => {
      this.roll = new Arinc429Word(p);
      publisher.post('rollAr', this.roll);
    });
    subscriber.topic('groundTrack').listen((gt) => {
      this.groundTrack = new Arinc429Word(gt);
      publisher.post('groundTrackAr', this.groundTrack);
    });
    subscriber.topic('heading').listen((h) => {
      this.heading = new Arinc429Word(h);
      publisher.post('headingAr', this.heading);
    });

    subscriber.topic('speed').listen((s) => {
      this.speed = new Arinc429Word(s);
      publisher.post('speedAr', this.speed);
    });

    this.altitude.watch((v) => publisher.post('altitudeAr', v));

    this.fcuEisDiscreteWord2.watch((v) => {
      const isStd = v.bitValueOr(11, true);
      if (isStd) {
        this.baroAltitudePipe.hold();
        this.pressureAltitudePipe.release(true);
      } else {
        this.pressureAltitudePipe.hold();
        this.baroAltitudePipe.release(true);
      }
    }, true);

    subscriber.topic('mach').listen((m) => {
      this.mach = new Arinc429Word(m);
      publisher.post('machAr', this.mach);
    });

    subscriber.topic('vsInert').listen((ivs) => {
      this.vsInert = new Arinc429Word(ivs);

      if (this.vsInert.isNormalOperation()) {
        publisher.post('vs', this.vsInert);
      }
    });

    subscriber.topic('vsBaro').listen((vsb) => {
      this.vsBaro = new Arinc429Word(vsb);
      if (!this.vsInert.isNormalOperation()) {
        publisher.post('vs', this.vsBaro);
      }
    });

    subscriber.topic('groundSpeed').listen((gs) => {
      this.groundSpeed = new Arinc429Word(gs);
      publisher.post('gs', this.groundSpeed);
    });

    subscriber.topic('radioAltitude1').listen((ra) => {
      this.radioAltitude1 = new Arinc429Word(ra);
      this.determineAndPublishChosenRadioAltitude(publisher);
    });

    subscriber.topic('radioAltitude2').listen((ra) => {
      this.radioAltitude2 = new Arinc429Word(ra);
      this.determineAndPublishChosenRadioAltitude(publisher);
    });

    subscriber.topic('radioAltitude3').listen((ra) => {
      this.radioAltitude3 = new Arinc429Word(ra);
      this.determineAndPublishChosenRadioAltitude(publisher);
    });

    subscriber.topic('fpaRaw').listen((fpa) => {
      this.fpa = new Arinc429Word(fpa);
      publisher.post('fpa', this.fpa);
    });

    subscriber.topic('daRaw').listen((da) => {
      this.da = new Arinc429Word(da);
      publisher.post('da', this.da);
    });

    subscriber.topic('landingElevation1Raw').listen((elevation) => {
      if (getDisplayIndex() === 1) {
        this.ownLandingElevation = new Arinc429Word(elevation);
      } else {
        this.oppLandingElevation = new Arinc429Word(elevation);
      }
      this.determineAndPublishChosenLandingElevation(publisher);
    });

    subscriber.topic('staticPressureRaw').listen((sp) => {
      this.staticPressure = new Arinc429Word(sp);
      publisher.post('staticPressure', this.staticPressure);
    });

    subscriber.topic('lgciuDiscreteWord1Raw').listen((word) => {
      this.lgciuDiscreteWord1 = new Arinc429Word(word);
      publisher.post('lgciuDiscreteWord1', this.lgciuDiscreteWord1);
    });

    this.fm1Subs.push(
      subscriber.topic('fm1EisDiscrete2Raw').listen((raw) => publisher.post('fmEisDiscreteWord2Raw', raw), true),
    );
    this.fm2Subs.push(
      subscriber.topic('fm2EisDiscrete2Raw').listen((raw) => publisher.post('fmEisDiscreteWord2Raw', raw), true),
    );
    this.fm1Subs.push(subscriber.topic('fm1MdaRaw').listen((raw) => publisher.post('fmMdaRaw', raw), true));
    this.fm2Subs.push(subscriber.topic('fm2MdaRaw').listen((raw) => publisher.post('fmMdaRaw', raw), true));
    this.fm1Subs.push(subscriber.topic('fm1DhRaw').listen((raw) => publisher.post('fmDhRaw', raw), true));
    this.fm2Subs.push(subscriber.topic('fm2DhRaw').listen((raw) => publisher.post('fmDhRaw', raw), true));
    this.fm1Subs.push(subscriber.topic('fm1TransAltRaw').listen((raw) => publisher.post('fmTransAltRaw', raw), true));
    this.fm2Subs.push(subscriber.topic('fm2TransAltRaw').listen((raw) => publisher.post('fmTransAltRaw', raw), true));
    this.fm1Subs.push(subscriber.topic('fm1TransLvlRaw').listen((raw) => publisher.post('fmTransLvlRaw', raw), true));
    this.fm2Subs.push(subscriber.topic('fm2TransLvlRaw').listen((raw) => publisher.post('fmTransLvlRaw', raw), true));

    this.fm1Healthy.rebind(subscriber.topic('fm1HealthyDiscrete'));
    this.fm2Healthy.rebind(subscriber.topic('fm2HealthyDiscrete'));
    this.fm1Healthy.watch(this.determineFmToUse.bind(this));
    this.fm2Healthy.watch(this.determineFmToUse.bind(this), true);

    // Do the filter at hi freq for accuracy, but we don't want to publish the ARINC word at hi-freq
    // as hi-frequency SVG redraws would be very bad.
    this.sub.topic('simTimeHiFreq').listen((time) => {
      const deltaTime = MathUtils.clamp(time - this.lastAltitudeFilterTime, 0, 300);
      this.lastAltitudeFilterTime = time;

      this.altitudeFilter.push(this.unfilteredAltitude.get().value, deltaTime / 1000);
    });
  }

  /** @inheritdoc */
  public tick(): void {
    this.altitude.setValueSsm(this.altitudeFilter.latest() ?? 0, this.unfilteredAltitude.get().ssm);
  }

  private determineAndPublishChosenRadioAltitude(publisher: Outbox<Arinc429Values>) {
    const validRaMap = [this.radioAltitude1, this.radioAltitude2, this.radioAltitude3].map(
      (ra) => !ra.isFailureWarning() && !ra.isNoComputedData(),
    );
    const validCount = validRaMap.filter((x) => !!x).length;

    let chosenRas = [this.radioAltitude1, this.radioAltitude2]; // Default: 1 gets 1, 2 gets 2
    if (validCount === 3) {
      // pick the median
      const heights = [this.radioAltitude1, this.radioAltitude2, this.radioAltitude3].sort((a, b) => a.value - b.value);
      chosenRas = [heights[1], heights[1]];
    } else if (validCount === 2) {
      if (!validRaMap[0]) {
        // fail PFD 1 to RA 3
        chosenRas = [this.radioAltitude3, this.radioAltitude2];
      } else if (!validRaMap[1]) {
        // fail PFD 2 to RA 3
        chosenRas = [this.radioAltitude1, this.radioAltitude3];
      }
      // otherwise stick with the default (PFD 1 to 1, PFD 2 to 2)
    } else if (validCount === 1) {
      if (validRaMap[0]) {
        // both get RA 1
        chosenRas = [this.radioAltitude1, this.radioAltitude1];
      } else if (validRaMap[1]) {
        // both get RA 2
        chosenRas = [this.radioAltitude2, this.radioAltitude2];
      } else {
        // both get RA 3
        chosenRas = [this.radioAltitude3, this.radioAltitude3];
      }
    } else {
      // at this point all have either NCD or FW
      // try to fail back a bit more intelligently around FWs to prioritize NCDs
      const nonFailedMap = [this.radioAltitude1, this.radioAltitude2, this.radioAltitude3].map(
        (ra) => !ra.isFailureWarning(),
      );
      const nonFailedCount = nonFailedMap.filter((x) => !!x).length;
      if (nonFailedCount === 2) {
        if (!nonFailedMap[0]) {
          // fail PFD 1 to RA 3
          chosenRas = [this.radioAltitude3, this.radioAltitude2];
        } else if (!nonFailedMap[1]) {
          // fail PFD 2 to RA 3
          chosenRas = [this.radioAltitude1, this.radioAltitude3];
        }
      } else if (nonFailedCount === 1) {
        if (nonFailedMap[0]) {
          // both get RA 1
          chosenRas = [this.radioAltitude1, this.radioAltitude1];
        } else if (nonFailedMap[1]) {
          // both get RA 2
          chosenRas = [this.radioAltitude2, this.radioAltitude2];
        } else {
          // both get RA 3
          chosenRas = [this.radioAltitude3, this.radioAltitude3];
        }
      }
      // don't do anything in case of 3 FWs or 0 FWs and stick to the default
    }

    publisher.post('chosenRa', getDisplayIndex() === 1 ? chosenRas[0] : chosenRas[1]);
  }

  private determineAndPublishChosenLandingElevation(publisher: Outbox<Arinc429Values>) {
    const useOpposite =
      (this.ownLandingElevation.isFailureWarning() || this.ownLandingElevation.isNoComputedData()) &&
      !this.oppLandingElevation.isFailureWarning() &&
      !this.oppLandingElevation.isNoComputedData();

    if (useOpposite) {
      publisher.post('landingElevation', this.oppLandingElevation);
    } else {
      publisher.post('landingElevation', this.ownLandingElevation);
    }
  }

  private determineFmToUse(): void {
    const onSideIndex = MathUtils.clamp(getDisplayIndex(), 1, 2);

    const onlyFm1Healthy = this.fm1Healthy.get() && !this.fm2Healthy.get();
    const onlyFm2Healthy = this.fm2Healthy.get() && !this.fm1Healthy.get();

    if ((onSideIndex === 1 && !onlyFm2Healthy) || onlyFm1Healthy) {
      this.fm2Subs.forEach((sub) => sub.hold());
      this.fm1Subs.forEach((sub) => sub.release(true));
    } else if ((onSideIndex === 2 && !onlyFm1Healthy) || onlyFm2Healthy) {
      this.fm1Subs.forEach((sub) => sub.hold());
      this.fm2Subs.forEach((sub) => sub.release(true));
    }
  }
}

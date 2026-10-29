// Copyright (c) 2021-2023 FlyByWire Simulations
//
// SPDX-License-Identifier: GPL-3.0

import { Bus } from 'xplane-js/ui';
import { Arinc429Register, Arinc429RegisterSubject, Arinc429WordData } from '@flybywiresim/fbw-sdk';
import { EwdSimvars } from './EwdSimvarPublisher';

export interface Arinc429Values {
  sat: Arinc429Register;
  slatsFlapsStatus: Arinc429Register;
  slatsPosition: Arinc429Register;
  flapsPosition: Arinc429Register;
  cpiomBAgsDiscrete: Arinc429WordData;
}
export class ArincValueProvider {
  private sat = Arinc429Register.empty();

  private cpiomB1AgsDiscrete = Arinc429RegisterSubject.createEmpty();

  private cpiomB2AgsDiscrete = Arinc429RegisterSubject.createEmpty();

  private cpiomB3AgsDiscrete = Arinc429RegisterSubject.createEmpty();

  private cpiomB4AgsDiscrete = Arinc429RegisterSubject.createEmpty();

  private cpiomBToUse = 1;

  constructor(private readonly bus: Bus) {}

  public init() {
    const publisher = this.bus.outbox<Arinc429Values>();
    const subscriber = this.bus.inbox<EwdSimvars>();

    subscriber.topic('satRaw').listen((p) => {
      this.sat.set(p);
      publisher.post('sat', this.sat);
    });

    subscriber.topic('cpiomB1AgsDiscreteRaw').listen((discreteWord) => {
      this.cpiomB1AgsDiscrete.setWord(discreteWord);
      this.cpiomBToUse = this.determineCpiomBToUse();
      if (this.cpiomBToUse === 1) {
        publisher.post('cpiomBAgsDiscrete', this.cpiomB1AgsDiscrete.get());
      }
    });

    subscriber.topic('cpiomB2AgsDiscreteRaw').listen((discreteWord) => {
      this.cpiomB2AgsDiscrete.setWord(discreteWord);
      this.cpiomBToUse = this.determineCpiomBToUse();
      if (this.cpiomBToUse === 2) {
        publisher.post('cpiomBAgsDiscrete', this.cpiomB2AgsDiscrete.get());
      }
    });

    subscriber.topic('cpiomB2AgsDiscreteRaw').listen((discreteWord) => {
      this.cpiomB3AgsDiscrete.setWord(discreteWord);
      this.cpiomBToUse = this.determineCpiomBToUse();
      if (this.cpiomBToUse === 3) {
        publisher.post('cpiomBAgsDiscrete', this.cpiomB3AgsDiscrete.get());
      }
    });

    subscriber.topic('cpiomB3AgsDiscreteRaw').listen((discreteWord) => {
      this.cpiomB4AgsDiscrete.setWord(discreteWord);
      this.cpiomBToUse = this.determineCpiomBToUse();
      if (this.cpiomBToUse === 4) {
        publisher.post('cpiomBAgsDiscrete', this.cpiomB4AgsDiscrete.get());
      }
    });
  }

  private determineCpiomBToUse() {
    if (!this.cpiomB1AgsDiscrete.get().isFailureWarning()) {
      return 1;
    } else if (!this.cpiomB2AgsDiscrete.get().isFailureWarning()) {
      return 2;
    } else if (!this.cpiomB3AgsDiscrete.get().isFailureWarning()) {
      return 3;
    } else {
      return 4;
    }
  }
}

//  Copyright (c) 2026 FlyByWire Simulations
//  SPDX-License-Identifier: GPL-3.0

import { Atc } from '@datalink/atc';
import { BusSignal, Bus } from 'xplane-js/ui';
import { CpiomData } from '@providers/CpiomPublisher';
export class AtcDatalink {
  private readonly available = BusSignal.from(
    this.bus.inbox<CpiomData>().topic('cpiom_d_available_1'),
    false,
  );

  private readonly atc: Atc;
  constructor(private readonly bus: Bus) {
    this.atc = new Atc(this.bus, false, false);
    this.available.watch((available: boolean) => {
      if (available) {
        this.atc.powerUp();
      } else {
        this.atc.powerDown();
      }
    }, true);
  }

  public init(): void {
    this.atc.initialize();
  }
}

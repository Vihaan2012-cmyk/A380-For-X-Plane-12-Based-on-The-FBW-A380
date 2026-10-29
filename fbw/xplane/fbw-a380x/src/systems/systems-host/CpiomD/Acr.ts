//  Copyright (c) 2026 FlyByWire Simulations
//  SPDX-License-Identifier: GPL-3.0
import { FrequencyMode, RegisteredSimVar } from '@flybywiresim/fbw-sdk';
import { BusSignal, Bus, VarUnit } from 'xplane-js/ui';
import { Router, VhfRadioInterface } from '@datalink/router';
import { CpiomData } from '@providers/CpiomPublisher';

class A380xVhfProvider implements VhfRadioInterface {
  private static readonly RmpModeActive3 = RegisteredSimVar.create<number>(
    'L:FBW_RMP_MODE_ACTIVE_3',
    VarUnit.Enum,
  );

  public isDataModeActive(): boolean {
    return A380xVhfProvider.RmpModeActive3.get() === FrequencyMode.Data;
  }
}

export class Acr {
  private readonly available = BusSignal.from(
    this.bus.inbox<CpiomData>().topic('cpiom_d_available_3'),
    false,
  );
  private readonly router: Router;

  constructor(private readonly bus: Bus) {
    this.router = new Router(bus, false, false, new A380xVhfProvider());
    this.available.watch((available) => {
      if (available) {
        this.router.powerUp();
      } else {
        this.router.powerDown();
      }
    }, true);
  }

  init(): void {
    this.router.initialize();
  }
}

//  Copyright (c) 2026 FlyByWire Simulations
//  SPDX-License-Identifier: GPL-3.0
import { Aoc } from '@datalink/aoc';
import { Bus, Inbox } from 'xplane-js/ui';
import { AtsuFmsClient } from './AtsuFmsClient';
import { PowerSupplyBusTypes } from '../Misc/powersupply';

export class AtsuSystem {
  //FIXME: This should be in NSS? Not CPIOM D.
  private readonly powerSupply: Inbox<PowerSupplyBusTypes>;

  private readonly aoc: Aoc;

  private readonly atsuFmsClient: AtsuFmsClient;

  constructor(private readonly bus: Bus) {
    this.aoc = new Aoc(this.bus, false);
    this.atsuFmsClient = new AtsuFmsClient(this.bus);

    this.powerSupply = this.bus.inbox<PowerSupplyBusTypes>();
    this.powerSupply.topic('acBus1').listen((powered: boolean) => {
      //FIXME: Should be NSS status
      if (powered) {
        this.aoc.powerUp();
      } else {
        this.aoc.powerDown();
      }
    });
  }

  public init(): void {
    this.aoc.initialize();
    this.atsuFmsClient.init();
  }
}

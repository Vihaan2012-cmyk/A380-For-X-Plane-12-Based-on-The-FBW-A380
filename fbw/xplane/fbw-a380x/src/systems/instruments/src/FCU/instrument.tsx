// Copyright (c) 2024 FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0

import { Bus, UI, Board } from 'xplane-js/ui';
import { FCUSimvarPublisher } from './Publishers/FcuSimvarPublisher';
import { FCUComponent } from './FCU';
import { MsfsAircraftSystemPublisher } from '@flybywiresim/fbw-sdk';
import { PanelInstrument, defineInstrument } from 'xplane-js/data';

// eslint-disable-next-line camelcase
class A380X_FCU extends PanelInstrument {
  private readonly bus = new Bus();
  private readonly backplane = new Board();

  private readonly simVarPublisher = new FCUSimvarPublisher(this.bus);
  private readonly msfsAircraftSystemPublisher = new MsfsAircraftSystemPublisher(this.bus);

  constructor() {
    super();

    this.backplane.attachFeeder('SimVarPublisher', this.simVarPublisher);
    this.backplane.attachFeeder('MsfsAircraftSystem', this.msfsAircraftSystemPublisher);
  }

  get templateId(): string {
    return 'A380X_FCU';
  }

  public onAttach(): void {
    super.onAttach();

    UI.mount(<FCUComponent bus={this.bus} />, document.getElementById('FCU_CONTENT'));

    // Remove "instrument didn't load" text
    document.getElementById('FCU_CONTENT')?.querySelector(':scope > h1')?.remove();

    this.backplane.setup();
  }

  public onFrame(): void {
    super.onFrame();

    this.backplane.tick();
  }
}

defineInstrument('a380x-fcu', A380X_FCU);

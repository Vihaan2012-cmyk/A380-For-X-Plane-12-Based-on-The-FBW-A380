//  Copyright (c) 2025 FlyByWire Simulations
//  SPDX-License-Identifier: GPL-3.0

import {
  ViewProps,
  BusSignal,
  View,
  Bus,
  UI,
  DerivedReadable,
  Bond,
  ViewNode,
} from 'xplane-js/ui';

import './style.scss';
import { OitDisplayUnit, OitDisplayUnitID } from './OitDisplayUnit';
import { FailuresConsumer } from '@flybywiresim/fbw-sdk';
import { OisLaptop } from './OisLaptop';
import { OitFltOpsContainer } from './OitFltOpsContainer';
import { OitUiService } from './OitUiService';
import { OitAvncsContainer } from './OitAvncsContainer';
import { OitSimvars } from './OitSimvarPublisher';
import { AnsuOps } from './System/AnsuOps';
import { initLoadsheetBus } from './System/Loadsheet';
import { AocUplink } from './System/AocUplink';
import { PanelInstrument } from 'xplane-js/data';

export interface AbstractOitFltOpsPageProps extends ViewProps {
  bus: Bus;
  uiService: OitUiService;
  container: OitFltOpsContainer;
}

export interface AbstractOitAvncsPageProps extends ViewProps {
  bus: Bus;
  uiService: OitUiService;
  container: OitAvncsContainer;
}

export type OisDomain = 'nss-avncs' | 'flt-ops';

export interface OitProps {
  readonly bus: Bus;
  readonly instrument: PanelInstrument;
  readonly captOrFo: 'CAPT' | 'FO';
  readonly failuresConsumer: FailuresConsumer;
  readonly laptop: OisLaptop;
  readonly avncsAnsu: AnsuOps;
}

export class OIT extends View<OitProps> {
  private readonly subscriptions: Bond[] = [];

  private readonly sub = this.props.bus.inbox<OitSimvars>();

  private readonly loadsheetBusInit = initLoadsheetBus(this.props.bus);

  private readonly aocUplink = new AocUplink(this.props.bus);

  /** True: NSS AVNCS, False: FLT OPS */
  private readonly oisDomainSwitch = BusSignal.from(
    this.sub.topic(this.props.captOrFo === 'CAPT' ? 'oisDomainSwitchCapt' : 'oisDomainSwitchFo'),
    false,
  );

  public readonly avncsOrFltOps: DerivedReadable<OisDomain> = this.oisDomainSwitch.derive((domainSwitch) =>
    domainSwitch ? 'nss-avncs' : 'flt-ops',
  );

  private readonly displayUnitRef = UI.ref<OitDisplayUnit>();

  public afterRender(node: ViewNode): void {
    super.afterRender(node);
    this.aocUplink.start();

    this.subscriptions.push(this.oisDomainSwitch, this.avncsOrFltOps);
  }

  destroy(): void {
    for (const s of this.subscriptions) {
      s.destroy();
    }

    super.destroy();
  }

  render(): ViewNode | null {
    return (
      <OitDisplayUnit
        ref={this.displayUnitRef}
        bus={this.props.bus}
        displayUnitId={this.props.captOrFo === 'CAPT' ? OitDisplayUnitID.CaptOit : OitDisplayUnitID.FoOit}
        failuresConsumer={this.props.failuresConsumer}
        avncsOrFltOps={this.avncsOrFltOps}
      >
        <OitFltOpsContainer
          bus={this.props.bus}
          laptop={this.props.laptop}
          displayUnitRef={this.displayUnitRef}
          captOrFo={this.props.captOrFo}
          avncsOrFltOps={this.avncsOrFltOps}
        />
        <OitAvncsContainer
          bus={this.props.bus}
          ansu={this.props.avncsAnsu}
          displayUnitRef={this.displayUnitRef}
          captOrFo={this.props.captOrFo}
          avncsOrFltOps={this.avncsOrFltOps}
        />
      </OitDisplayUnit>
    );
  }
}

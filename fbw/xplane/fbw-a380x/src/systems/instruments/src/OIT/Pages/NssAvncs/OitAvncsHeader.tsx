//  Copyright (c) 2025 FlyByWire Simulations
//  SPDX-License-Identifier: GPL-3.0

import { BusSignal, Bus, UI, Signal, Readable, ViewNode } from 'xplane-js/ui';
import { Button } from '../../../MsfsAvionicsCommon/UiWidgets/Button';
import { IconButton } from '../../../MsfsAvionicsCommon/UiWidgets/IconButton';
import { OitUiService } from '../../OitUiService';
import { OisDomain } from '../../OIT';
import { FmsData } from '@flybywiresim/fbw-sdk';
import { DestroyableComponent } from '@flybywiresim/msfs-avionics-common';

interface OitAvncsHeaderProps {
  readonly bus: Bus;
  readonly uiService: OitUiService;
  readonly avncsOrFltOps: Readable<OisDomain>;
}

/*
 * Complete header for the OIS system, both AVNCS and FLT OPS.
 */
export abstract class OitAvncsHeader extends DestroyableComponent<OitAvncsHeaderProps> {
  private readonly sub = this.props.bus.inbox<FmsData>();

  // FIXME maybe this should come from the ATC/ATSU when it's integrated
  private readonly fltNumber = BusSignal.from(this.sub.topic('fmsFlightNumber'), null);
  private readonly fltNumberText = this.fltNumber.derive((number) => (number ? number : ''));

  public afterRender(node: ViewNode): void {
    super.afterRender(node);

    this.subscriptions.push(this.fltNumber, this.fltNumberText);
  }

  render(): ViewNode {
    return (
      <div class="oit-header-row">
        <Button
          label={'NSS AVNCS MENU'}
          onClick={() => this.props.uiService.navigateTo('nss-avncs')}
          buttonStyle="width: 250px; font-size: 28px; height: 50px;"
        />
        <Button
          label={'MAXIMIZE'}
          onClick={() => {}}
          buttonStyle="width: 125px; font-size: 28px; height: 50px; margin-left: 5px;"
        />
        <div class="oit-msg-header" style="padding: 11px;">
          0 MSG
        </div>
        <div class="oit-msg-box"></div>
        <IconButton
          icon={'flat-single-down'}
          containerStyle="width: 60px; height: 50px; margin-left: 5px;"
          disabled={Signal.of(true)}
        />
        <Button
          label={'CLEAR'}
          onClick={() => {}}
          buttonStyle="font-size: 28px; height: 50px; margin-left: 15px;"
          disabled={Signal.of(true)}
        />
        <div class="oit-avncs-header-flt-nbr">{this.fltNumberText}</div>
      </div>
    );
  }
}

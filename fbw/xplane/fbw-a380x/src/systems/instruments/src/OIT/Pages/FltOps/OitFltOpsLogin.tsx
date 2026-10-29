//  Copyright (c) 2025 FlyByWire Simulations
//  SPDX-License-Identifier: GPL-3.0

import { Bus, UI, VarUnit, Logic, ViewNode } from 'xplane-js/ui';
import { DestroyableComponent } from '@flybywiresim/msfs-avionics-common';
import { Button } from '../../../MsfsAvionicsCommon/UiWidgets/Button';
import { OitUiService } from '../../OitUiService';
import { Vars } from 'xplane-js/data';

interface OitFltOpsLoginPageProps {
  readonly bus: Bus;
  readonly uiService: OitUiService;
  readonly captOrFo: 'CAPT' | 'FO';
}

export class OitFltOpsLogin extends DestroyableComponent<OitFltOpsLoginPageProps> {
  // Make sure to collect all subscriptions in this.subscriptions, otherwise page navigation doesn't work.

  private readonly hideLoginScreen = this.props.uiService.fltOpsLoginScreenVisible.derive(Logic.not());

  public afterRender(node: ViewNode): void {
    super.afterRender(node);

    this.subscriptions.push(this.hideLoginScreen);
  }

  render(): ViewNode {
    return (
      <>
        {/* begin page content */}
        <div class={{ 'oit-flt-ops-loading-screen-container': true, hidden: this.hideLoginScreen }}>
          <div class="oit-login-page-heading">FLT OPS Domain</div>
          <div class="oit-login-page-sub-heading">Login Page</div>
          <div class="oit-login-page-sub-sub-heading">
            {this.props.captOrFo === 'CAPT' ? 'CAPTAIN' : 'FIRST OFFICER'}
          </div>
          <div class="oit-login-page-button-container">
            <Button label="PILOT" onClick={() => this.props.uiService.fltOpsLoginScreenVisible.set(false)} />
            <Button
              label="MAINTAINER"
              onClick={() => this.props.uiService.fltOpsLoginScreenVisible.set(false)}
              containerStyle="margin-top: 10px;"
              disabled={true}
            />
          </div>
          <div class="oit-login-page-bottom-left-button">
            <Button
              label="SWITCH OFF<br />LAPTOP"
              onClick={() =>
                Vars.write(
                  `L:A380X_SWITCH_LAPTOP_POWER_${this.props.captOrFo === 'CAPT' ? 'LEFT' : 'RIGHT'}`,
                  VarUnit.Bool,
                  false,
                )
              }
            />
          </div>
        </div>
        {/* end page content */}
      </>
    );
  }
}

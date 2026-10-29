//  Copyright (c) 2025 FlyByWire Simulations
//  SPDX-License-Identifier: GPL-3.0

import { UI, Readable, ViewNode } from 'xplane-js/ui';
import { Button } from '../../../MsfsAvionicsCommon/UiWidgets/Button';
import { OitUiService } from '../../OitUiService';
import { OisDomain } from '../../OIT';
import { DestroyableComponent } from '@flybywiresim/msfs-avionics-common';

interface OitAvncsFooterProps {
  readonly uiService: OitUiService;
  readonly avncsOrFltOps: Readable<OisDomain>;
}

/*
 * Complete header for the ATCCOM system
 */
export abstract class OitAvncsFooter extends DestroyableComponent<OitAvncsFooterProps> {
  public afterRender(node: ViewNode): void {
    super.afterRender(node);
  }

  render(): ViewNode {
    return (
      <div class="oit-header-row">
        <Button
          label={'COMPANY COM'}
          onClick={() => this.props.uiService.navigateTo('nss-avncs/company-com')}
          buttonStyle="width: 225px; font-size: 28px; height: 50px;"
        />
        <div style="flex-grow: 1" />
      </div>
    );
  }
}

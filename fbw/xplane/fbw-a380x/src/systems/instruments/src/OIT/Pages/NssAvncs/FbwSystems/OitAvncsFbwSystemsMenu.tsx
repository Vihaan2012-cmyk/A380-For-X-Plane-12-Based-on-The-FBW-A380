//  Copyright (c) 2025 FlyByWire Simulations
//  SPDX-License-Identifier: GPL-3.0

import { Bus, UI, ViewNode } from 'xplane-js/ui';
import { DestroyableComponent } from '@flybywiresim/msfs-avionics-common';
import { OitFile, OitFolder } from '../OitAvncsFolderNavigator';
import { OitUiService } from '../../../OitUiService';

interface OitAvncsFbwSystemsMenuProps {
  readonly bus: Bus;
  readonly uiService: OitUiService;
}

export class OitAvncsFbwSystemsMenu extends DestroyableComponent<OitAvncsFbwSystemsMenuProps> {
  public afterRender(node: ViewNode): void {
    super.afterRender(node);
  }

  render(): ViewNode {
    return (
      <>
        <OitFolder name={'AUTO FLT'} initExpanded={true}>
          <OitFile
            name={'App & Ldg Capability'}
            uiService={this.props.uiService}
            navigationTarget="nss-avncs/a380x-systems/app-ldg-cap"
          />
        </OitFolder>
        <OitFolder name={'CDS / FWS'} initExpanded={true}>
          <OitFile
            name={'FWS Debug'}
            uiService={this.props.uiService}
            navigationTarget="nss-avncs/a380x-systems/debug-data"
          />
        </OitFolder>
      </>
    );
  }
}

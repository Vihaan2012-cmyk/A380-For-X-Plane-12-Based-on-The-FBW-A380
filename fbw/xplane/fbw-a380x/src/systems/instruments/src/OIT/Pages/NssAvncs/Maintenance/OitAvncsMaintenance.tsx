/** @jsx UI.node */
/** @jsxFrag UI.Group */
//  Copyright (c) 2026 FlyByWire Simulations
//  SPDX-License-Identifier: GPL-3.0

import { Part as View, Parts as UI, PartNode as ViewNode } from 'xplane-js/ui';
import { DestroyableComponent } from '@flybywiresim/msfs-avionics-common';
import { AbstractOitAvncsPageProps } from '../../../OIT';
import { OitAvncsSubHeader } from '../OitAvncsSubHeader';
import { OitUriInformation } from '../../../OitUiService';
import { avncsMaintenancePageForUrl } from '../../../OitPageDirectory';
import { OitAvncsMaintenanceMenu } from './OitAvncsMaintenanceMenu';
import { MaintenanceData } from '../../../System/MaintenanceData';

interface OitAvncsMaintenanceProps extends AbstractOitAvncsPageProps {}

export class OitAvncsMaintenance extends DestroyableComponent<OitAvncsMaintenanceProps> {
  private readonly activePageRef = UI.ref<HTMLDivElement>();

  private activePage: ViewNode = (<></>) as ViewNode;

  private activeUriChanged(uri: OitUriInformation) {
    if (!uri.uri.match(/nss-avncs\/maintenance(\/\S*)?$/gm)) {
      return;
    }

    if (this.activePageRef.peek()) {
      while (this.activePageRef.target.firstChild) {
        this.activePageRef.target.removeChild(this.activePageRef.target.firstChild);
      }
    }
    if (this.activePage && this.activePage.target instanceof View) {
      this.activePage.target.destroy();
    }

    const url = uri.extra ? `${uri.sys}/${uri.page}/${uri.extra}` : `${uri.sys}/${uri.page}`;
    this.activePage = avncsMaintenancePageForUrl(url, this.props.bus, this.props.uiService, this.props.container);
    UI.mount(this.activePage, this.activePageRef?.peek());
  }

  public afterMount(node: ViewNode): void {
    super.afterMount(node);

    MaintenanceData.instance().acquire();
    this.subscriptions.push(
      this.props.uiService.activeUri.watch((uri) => {
        this.activeUriChanged(uri);
      }, true),
    );
  }

  destroy(): void {
    MaintenanceData.instance().release();
    if (this.activePage && this.activePage.target instanceof View) {
      this.activePage.target.destroy();
    }
    super.destroy();
  }

  render(): ViewNode {
    return (
      <>
        <OitAvncsSubHeader title={'MAINTENANCE'} uiService={this.props.uiService} />
        <div class="oit-page-container">
          <div class="oit-avncs-navigator-container">
            <div class="oit-avncs-navigator-left">
              <OitAvncsMaintenanceMenu bus={this.props.bus} uiService={this.props.uiService} />
            </div>
            <div class="oit-avncs-navigator-right oit-centered" ref={this.activePageRef} />
          </div>
        </div>
      </>
    );
  }
}

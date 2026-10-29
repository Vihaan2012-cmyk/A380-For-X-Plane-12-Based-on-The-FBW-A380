//  Copyright (c) 2025 FlyByWire Simulations
//  SPDX-License-Identifier: GPL-3.0

import { View, UI, ViewNode } from 'xplane-js/ui';
import { DestroyableComponent } from '@flybywiresim/msfs-avionics-common';
import { AbstractOitAvncsPageProps } from '../../../OIT';
import { OitAvncsSubHeader } from '../OitAvncsSubHeader';
import { OitUriInformation } from '../../../OitUiService';
import { avncsFbwSystemsPageForUrl } from '../../../OitPageDirectory';
import { OitAvncsFbwSystemsMenu } from '../FbwSystems/OitAvncsFbwSystemsMenu';

interface OitAvncsFbwSystemsPageProps extends AbstractOitAvncsPageProps {}

export class OitAvncsFbwSystems extends DestroyableComponent<OitAvncsFbwSystemsPageProps> {
  // Make sure to collect all subscriptions in this.subscriptions, otherwise page navigation doesn't work.

  private readonly activePageRef = UI.ref<HTMLDivElement>();

  private activePage: ViewNode = (<></>) as ViewNode;

  private activeUriChanged(uri: OitUriInformation) {
    if (!uri.uri.match(/nss-avncs\/a380x-systems\/\S*/gm)) {
      // Navigate somewhere else outside of the A380X SYSTEMSs section
      return;
    }

    // Remove and destroy old OIT page
    if (this.activePageRef.peek()) {
      while (this.activePageRef.target.firstChild) {
        this.activePageRef.target.removeChild(this.activePageRef.target.firstChild);
      }
    }
    if (this.activePage && this.activePage.target instanceof View) {
      this.activePage.target.destroy();
    }

    // Mapping from URL to page component
    if (uri.page && uri.extra) {
      this.activePage = avncsFbwSystemsPageForUrl(
        `${uri.sys}/${uri.page}/${uri.extra}`,
        this.props.bus,
        this.props.uiService,
        this.props.container,
      );
    } else if (uri.page) {
      this.activePage = avncsFbwSystemsPageForUrl(
        `${uri.sys}/${uri.page}`,
        this.props.bus,
        this.props.uiService,
        this.props.container,
      );
    } else {
      this.activePage = avncsFbwSystemsPageForUrl(
        `${uri.sys}`,
        this.props.bus,
        this.props.uiService,
        this.props.container,
      );
    }

    UI.mount(this.activePage, this.activePageRef?.peek());
  }

  public afterRender(node: ViewNode): void {
    super.afterRender(node);

    this.subscriptions.push(
      this.props.uiService.activeUri.watch((uri) => {
        this.activeUriChanged(uri);
      }),
    );
  }

  render(): ViewNode {
    return (
      <>
        {/* begin page content */}
        <OitAvncsSubHeader title={'A380X SYSTEMS'} uiService={this.props.uiService} />
        <div class="oit-page-container">
          <div class="oit-avncs-navigator-container">
            <div class="oit-avncs-navigator-left">
              <OitAvncsFbwSystemsMenu bus={this.props.bus} uiService={this.props.uiService} />
            </div>
            <div class="oit-avncs-navigator-right oit-centered" ref={this.activePageRef} />
          </div>
        </div>
        {/* end page content */}
      </>
    );
  }
}

//  Copyright (c) 2025 FlyByWire Simulations
//  SPDX-License-Identifier: GPL-3.0

import { View, UI, Bond, ViewNode } from 'xplane-js/ui';
import { OitUiService } from '../OitUiService';

interface OitNotFoundProps {
  uiService: OitUiService;
}

export class OitNotFound extends View<OitNotFoundProps> {
  // Make sure to collect all subscriptions here, otherwise page navigation doesn't work.
  private readonly subs = [] as Bond[];

  public afterRender(node: ViewNode): void {
    super.afterRender(node);

    new Promise((resolve) => setTimeout(resolve, 500)).then(() => this.props.uiService.navigateTo('back'));
  }

  public destroy(): void {
    // Destroy all subscriptions to remove all references to this instance.
    for (const s of this.subs) {
      s.destroy();
    }

    super.destroy();
  }

  render(): ViewNode {
    return (
      <>
        {/* begin page content */}
        <div class="oit-page-container" />
        {/* end page content */}
      </>
    );
  }
}

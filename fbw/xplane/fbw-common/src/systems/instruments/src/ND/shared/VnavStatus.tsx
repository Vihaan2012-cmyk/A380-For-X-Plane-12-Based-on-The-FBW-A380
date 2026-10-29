// Copyright (c) 2021-2023 FlyByWire Simulations
//
// SPDX-License-Identifier: GPL-3.0

import { View, UI, ViewNode } from 'xplane-js/ui';
import { Bridge } from 'xplane-js/data';

export class VnavStatus extends View<{}> {
  private readonly textRef = UI.ref<SVGTextElement>();

  afterRender(node: ViewNode) {
    super.afterRender(node);

    Bridge.on('A32NX_FM_DEBUG_VNAV_STATUS', (message: string) => {
      this.textRef.target.innerHTML = message
        .split('\n')
        .map((line) => `<tspan x="50" dy="1.2em">${line}</tspan>`)
        .join('\r');
    });
  }

  render(): ViewNode | null {
    return <text x={50} y={400} ref={this.textRef} class="Magenta FontTiny StartAlign shadow" />;
  }
}

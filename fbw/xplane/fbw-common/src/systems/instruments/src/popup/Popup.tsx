// Copyright (c) 2026 FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0

import { View, ViewNode, UI, Readable, Derived } from 'xplane-js/ui';
import { MathUtils } from '../../../shared/src/MathUtils';
import { PopupDefinition } from '../../../shared/src/popup/PopupTypes';

export interface PopupProps {
  readonly currentPopup: Readable<PopupDefinition | undefined>;
  readonly timeRemaining: Readable<number | undefined>;
}
export class Popup extends View<PopupProps> {
  private readonly percentRemaining = Derived.from(
    ([currentPopup, timeRemaining]) =>
      timeRemaining && currentPopup?.timeout
        ? `${MathUtils.round((100 * timeRemaining) / currentPopup.timeout, 0.1)}%`
        : '0%',
    this.props.currentPopup,
    this.props.timeRemaining,
  );

  doRender(): ViewNode {
    return (
      <div
        id="resume"
        class={{
          'fbw-hidden': this.props.currentPopup.derive((v) => v === undefined),
          absolute: true,
          'inset-0': true,
          'z-50': true,
          flex: true,
          'items-center': true,
          'justify-center': true,
        }}
      >
        <div class="relative mx-6 w-full overflow-hidden rounded-xl bg-theme-body px-10 py-8 text-center">
          <h1 class="p-2 text-4xl font-bold">{this.props.currentPopup.derive((v) => v?.title ?? '')}</h1>
          <p class="p-8 text-xl leading-relaxed text-theme-text">
            {this.props.currentPopup.derive((v) => v?.message ?? '')}
          </p>
          <div class="absolute bottom-0 left-0 h-1 bg-theme-highlight" style={{ width: this.percentRemaining }} />
        </div>
      </div>
    );
  }

  render(): ViewNode | null {
    return this.doRender();
  }
}

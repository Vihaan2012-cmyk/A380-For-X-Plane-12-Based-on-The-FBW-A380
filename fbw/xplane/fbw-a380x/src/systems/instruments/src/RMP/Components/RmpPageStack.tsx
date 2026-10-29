// Copyright (c) 2024 FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0

import {
  ViewProps,
  View,
  Bus,
  UI,
  Ref,
  Signal,
  ViewNode,
} from 'xplane-js/ui';

import { RmpPage } from './RmpPage';
import { VhfComPage } from '../Pages/VhfCom/VhfComPage';
import { KeypadEvents, SystemKeys } from '../Systems/KeypadController';
import { TransponderPage } from '../Pages/Transponder/TransponderPage';

export interface RmpPageStackProps extends ViewProps {
  bus: Bus;
}

type Pages = 'vhf' | 'transponder';

type PageData = {
  route: string;
  ref: Ref<RmpPage>;
};

export class RmpPageStack extends View<RmpPageStackProps> {
  private readonly pages: Record<Pages, PageData> = {
    vhf: {
      route: '/vhf',
      ref: UI.ref<RmpPage>(),
    },
    transponder: {
      route: '/transponder',
      ref: UI.ref<RmpPage>(),
    },
  };

  private previousPage: Pages | null = null;
  private readonly activePage = Signal.of<Pages>('vhf');

  afterRender(): void {
    this.activePage.watch((v) => {
      if (this.previousPage) {
        this.pages[this.previousPage].ref.target.setActive(false);
      }
      this.previousPage = v;
      this.pages[v].ref.target.setActive(true);
    }, true);

    const sub = this.props.bus.inbox<KeypadEvents>();
    sub
      .topic('keypad_page_key_pressed')
      .listen((key) => this.pages[this.activePage.get()].ref.target.onKeyPressed?.(key));
    sub
      .topic('keypad_page_key_released')
      .listen((key) => this.pages[this.activePage.get()].ref.target.onKeyReleased?.(key));
    sub.topic('keypad_system_key_pressed').listen((key) => {
      switch (key) {
        case SystemKeys.VhfPage:
          this.navigateTo('/vhf');
          break;
        case SystemKeys.SquawkPage:
          this.navigateTo('/transponder');
          break;
      }
    });
  }

  private navigateTo(route: string): void {
    for (const page of Object.keys(this.pages) as Pages[]) {
      if (this.pages[page].route === route) {
        this.activePage.set(page);
        return;
      }
    }
    console.warn('Invalid route:', route);
  }

  /** @inheritdoc */
  render(): ViewNode | null {
    return (
      <div class="rmp-pages">
        <VhfComPage ref={this.pages.vhf.ref} bus={this.props.bus} />
        <TransponderPage ref={this.pages.transponder.ref} bus={this.props.bus} />
      </div>
    );
  }
}

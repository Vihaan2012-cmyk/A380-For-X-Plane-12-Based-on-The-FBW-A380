// Copyright (c) 2021-2023 FlyByWire Simulations
//
// SPDX-License-Identifier: GPL-3.0

import { ViewProps, Readable, Signal } from 'xplane-js/ui';

import { ArincEventBus, Arinc429WordData } from '@flybywiresim/fbw-sdk';

import { NDPage } from '../NDPage';
import { NDControlEvents } from '../../NDControlEvents';

export interface RoseModeProps<T extends number> extends ViewProps {
  bus: ArincEventBus;
  rangeValues: T[];
  headingWord: Readable<Arinc429WordData>;
  trueHeadingWord: Readable<Arinc429WordData>;
  trackWord: Readable<Arinc429WordData>;
  trueTrackWord: Readable<Arinc429WordData>;
  isUsingTrackUpMode: Readable<boolean>;
}

export abstract class RoseMode<T extends number, P extends RoseModeProps<T> = RoseModeProps<T>> extends NDPage<P> {
  abstract isVisible: Signal<boolean>;

  onShow() {
    super.onShow();

    this.movePlane();
  }

  private movePlane() {
    const publisher = this.props.bus.outbox<NDControlEvents>();

    publisher.post('set_show_plane', true);
    publisher.post('set_plane_x', 384);
    publisher.post('set_plane_y', 384);
  }
}

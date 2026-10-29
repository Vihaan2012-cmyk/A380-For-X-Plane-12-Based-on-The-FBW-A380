// Copyright (c) 2026 FlyByWire Simulations
//
// SPDX-License-Identifier: GPL-3.0

import { HostedInstrument } from 'xplane-js/ui';
import { PopupFsInstrument } from '@flybywiresim/popup';

import '../../../../../../fbw-common/src/systems/instruments/src/popup/style.scss';
import { defineInstrument } from 'xplane-js/data';

class A380X_POPUP extends HostedInstrument<PopupFsInstrument> {
  createDevice(): PopupFsInstrument {
    return new PopupFsInstrument(this);
  }

  get interactive(): boolean {
    return false;
  }

  get templateId(): string {
    return 'A380X_popup';
  }
}

defineInstrument('a380x-popup', A380X_POPUP);

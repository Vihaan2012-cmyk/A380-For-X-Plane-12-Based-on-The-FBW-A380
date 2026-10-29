// Copyright (c) 2024 FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0

import { ViewProps, View, Bus } from 'xplane-js/ui';

import { PageKeys } from '../Systems/KeypadController';

export interface RmpPage extends View<RmpPageProps> {
  setActive: (isActive: boolean) => void;
  onKeyPressed?: (key: PageKeys) => void;
  onKeyReleased?: (key: PageKeys) => void;
}

export interface RmpPageProps extends ViewProps {
  bus: Bus;
}

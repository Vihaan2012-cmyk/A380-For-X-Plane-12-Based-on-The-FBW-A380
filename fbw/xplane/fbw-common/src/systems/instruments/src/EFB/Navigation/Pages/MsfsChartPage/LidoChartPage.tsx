// Copyright (c) 2026 FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0

import React from 'react';

import { ChartOrigin } from 'xplane-js/ui';

import { MsfsChartUI } from './MsfsChartUI';
import { ChartProvider, NavigationTab } from '../../../Store/features/navigationPage';

export const LidoChartPage = () => (
  <MsfsChartUI msfsProvider={ChartOrigin.Lido} provider={ChartProvider.LIDO} tab={NavigationTab.LIDO} />
);

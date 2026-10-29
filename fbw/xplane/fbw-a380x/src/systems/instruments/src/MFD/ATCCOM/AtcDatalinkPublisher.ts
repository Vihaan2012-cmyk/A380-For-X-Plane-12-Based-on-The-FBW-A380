// Copyright (c) 2025-2026 FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0

import { IndexedTopic } from 'xplane-js/ui';
import { AirportAtis } from './AtcDatalinkSystem';

interface BaseAtcDatalinkMessages {
  atcAtis: AirportAtis;
}
type AtcDatalinkIndexedMessages = {
  [P in keyof BaseAtcDatalinkMessages as IndexedTopic<P>]: BaseAtcDatalinkMessages[P];
};

export interface AtcDatalinkMessages extends BaseAtcDatalinkMessages, AtcDatalinkIndexedMessages {}

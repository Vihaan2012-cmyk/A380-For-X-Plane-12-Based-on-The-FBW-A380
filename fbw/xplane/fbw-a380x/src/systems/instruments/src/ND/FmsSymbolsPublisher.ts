// Copyright (c) 2021-2024 FlyByWire Simulations
//
// SPDX-License-Identifier: GPL-3.0

import { TopicFeeder, Bus } from 'xplane-js/ui';
import { EfisSide, NdSymbol, NdTraffic, GenericDataListenerSync } from '@flybywiresim/fbw-sdk';

import { PathVector } from '@fmgc/guidance/lnav/PathVector';

export interface FmsSymbolsData {
  symbols: NdSymbol[];
  vectorsActive: PathVector[];
  vectorsActiveEosid: PathVector[];
  vectorsDashed: PathVector[];
  vectorsTemporary: PathVector[];
  vectorsMissed: PathVector[];
  vectorsAlternate: PathVector[];
  vectorsSecondary: PathVector[];
  traffic: NdTraffic[];
}

export class FmsSymbolsPublisher extends TopicFeeder<FmsSymbolsData> {
  private readonly events: GenericDataListenerSync[] = [];

  constructor(bus: Bus, side: EfisSide) {
    super(bus);

    this.events.push(
      new GenericDataListenerSync((ev, data) => {
        this.postTopic('symbols', data);
      }, `A32NX_EFIS_${side}_SYMBOLS`),
    );

    this.events.push(
      new GenericDataListenerSync((ev, data: PathVector[]) => {
        this.postTopic('vectorsActive', data);
      }, `A32NX_EFIS_VECTORS_${side}_ACTIVE`),
    );

    this.events.push(
      new GenericDataListenerSync((ev, data: PathVector[]) => {
        this.postTopic('vectorsActiveEosid', data);
      }, `A32NX_EFIS_VECTORS_${side}_ACTIVE_EOSID`),
    );

    this.events.push(
      new GenericDataListenerSync((ev, data: PathVector[]) => {
        this.postTopic('vectorsDashed', data);
      }, `A32NX_EFIS_VECTORS_${side}_DASHED`),
    );

    this.events.push(
      new GenericDataListenerSync((ev, data: PathVector[]) => {
        this.postTopic('vectorsTemporary', data);
      }, `A32NX_EFIS_VECTORS_${side}_TEMPORARY`),
    );

    this.events.push(
      new GenericDataListenerSync((ev, data: PathVector[]) => {
        this.postTopic('vectorsMissed', data);
      }, `A32NX_EFIS_VECTORS_${side}_MISSED`),
    );

    this.events.push(
      new GenericDataListenerSync((ev, data: PathVector[]) => {
        this.postTopic('vectorsAlternate', data);
      }, `A32NX_EFIS_VECTORS_${side}_ALTERNATE`),
    );

    this.events.push(
      new GenericDataListenerSync((ev, data: PathVector[]) => {
        this.postTopic('vectorsSecondary', data);
      }, `A32NX_EFIS_VECTORS_${side}_SECONDARY`),
    );

    this.events.push(
      new GenericDataListenerSync((ev, data: NdTraffic[]) => {
        this.postTopic('traffic', data);
      }, `A32NX_TCAS_${side}_TRAFFIC`),
    );
  }
}

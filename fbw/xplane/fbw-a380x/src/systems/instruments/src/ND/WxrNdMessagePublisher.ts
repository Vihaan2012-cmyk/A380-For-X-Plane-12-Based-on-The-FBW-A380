// Copyright (c) 2026 FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0

import { BusSignal, Bus, Derived, Bond } from 'xplane-js/ui';
import { EfisSide } from '@flybywiresim/fbw-sdk';
import { NdWxrMessage, NdWxrMessageEvents } from '@flybywiresim/navigation-display';
import { NDSimvars } from './NDSimvarPublisher';

export class WxrNdMessagePublisher {
  private readonly subs: Bond[] = [];

  private lastKey = '';

  constructor(
    private readonly bus: Bus,
    side: EfisSide,
  ) {
    const sub = bus.inbox<NDSimvars>();
    const l = side === 'L';
    const sysSelected = BusSignal.from(sub.topic('wxrTawsSysSelected'), 0);
    const failed1 = BusSignal.from(sub.topic('wxr1Failed'), true);
    const failed2 = BusSignal.from(sub.topic('wxr2Failed'), true);
    const off = BusSignal.from(sub.topic('wxrOff'), false);
    const overlay = BusSignal.from(sub.topic(l ? 'wxrOverlayL' : 'wxrOverlayR'), 0);
    const mode = BusSignal.from(sub.topic(l ? 'wxrElevnTiltL' : 'wxrElevnTiltR'), 0);
    const elevnFt = BusSignal.from(sub.topic(l ? 'wxrElevnFtL' : 'wxrElevnFtR'), 0);
    const tiltDeg = BusSignal.from(sub.topic(l ? 'wxrTiltDegL' : 'wxrTiltDegR'), 0);
    const gainMan = BusSignal.from(sub.topic(l ? 'wxrGainManL' : 'wxrGainManR'), false);
    const gainPct = BusSignal.from(sub.topic(l ? 'wxrGainPctL' : 'wxrGainPctR'), 50);
    const map = BusSignal.from(sub.topic(l ? 'wxrModeMapL' : 'wxrModeMapR'), false);
    const baroStd = BusSignal.from(sub.topic(l ? 'wxrBaroStdL' : 'wxrBaroStdR'), false);

    const message = Derived.from(
      ([sel, f1, f2, isOff, ov, m, ft, tilt, man, pct, isMap, std]): NdWxrMessage | null => {
        if (Math.round(ov) !== 1) {
          return null;
        }
        const failed = sel === 1 ? f1 : sel === 2 ? f2 : true;
        if (failed) {
          return { text: 'WXR INOP', className: 'Amber' };
        }
        if (isOff) {
          return { text: 'WXR OFF', className: 'White' };
        }
        if (isMap) {
          return { text: 'MAP', className: 'Green' };
        }
        const gain = man ? `GAIN ${Math.round(pct)}%` : undefined;
        switch (Math.round(m)) {
          case 1:
            return {
              text: std ? `ELEVN FL${Math.round(ft / 100).toString().padStart(3, '0')}` : `ELEVN ${Math.round(ft)}FT`,
              className: 'Cyan',
              secondLine: gain,
            };
          case 2:
            return {
              text: `TILT ${tilt >= 0 ? '+' : '-'}${Math.abs(tilt).toFixed(1)}°`,
              className: 'Cyan',
              secondLine: gain,
            };
          default:
            return gain ? { text: gain, className: 'Cyan' } : { text: 'WX', className: 'Green' };
        }
      },
      sysSelected,
      failed1,
      failed2,
      off,
      overlay,
      mode,
      elevnFt,
      tiltDeg,
      gainMan,
      gainPct,
      map,
      baroStd,
    );

    const publisher = bus.outbox<NdWxrMessageEvents>();
    this.subs.push(
      message.watch((m) => {
        const key = m ? `${m.className}|${m.text}|${m.secondLine ?? ''}` : '';
        if (key !== this.lastKey) {
          this.lastKey = key;
          publisher.post('ndWxrMessage', m, false, true);
        }
      }, true),
    );
  }
}

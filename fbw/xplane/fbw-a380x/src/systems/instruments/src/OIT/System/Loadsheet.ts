//  Copyright (c) 2026 FlyByWire Simulations
//  SPDX-License-Identifier: GPL-3.0

import { BusState as BusSignal, Exchange as Bus, Handle as Bond, makeBusState as $feed } from 'xplane-js/ui';
import { FmsData } from '@flybywiresim/fbw-sdk';
import { Vars } from 'xplane-js/data';

const MAX_ZFW_KG = 373_000;
const MAX_GW_KG = 512_000;

const PAX_STATION_VARS = [
  'L:A32NX_PAX_MAIN_FWD_A',
  'L:A32NX_PAX_MAIN_FWD_B',
  'L:A32NX_PAX_MAIN_MID_1A',
  'L:A32NX_PAX_MAIN_MID_1B',
  'L:A32NX_PAX_MAIN_MID_1C',
  'L:A32NX_PAX_MAIN_MID_2A',
  'L:A32NX_PAX_MAIN_MID_2B',
  'L:A32NX_PAX_MAIN_MID_2C',
  'L:A32NX_PAX_MAIN_AFT_A',
  'L:A32NX_PAX_MAIN_AFT_B',
  'L:A32NX_PAX_UPPER_FWD',
  'L:A32NX_PAX_UPPER_MID_A',
  'L:A32NX_PAX_UPPER_MID_B',
  'L:A32NX_PAX_UPPER_AFT',
];

function countSetBits(value: number): number {
  let v = Math.max(0, Math.round(value));
  let count = 0;
  while (v > 0) {
    count += v % 2;
    v = Math.floor(v / 2);
  }
  return count;
}

function totalPax(): number {
  return PAX_STATION_VARS.reduce((sum, name) => sum + countSetBits(Vars.read(name, 'number')), 0);
}

function fmt(n: number): string {
  return Math.round(n).toString();
}

function pad2(n: number): string {
  return n.toString().padStart(2, '0');
}

const MONTHS = ['JAN', 'FEB', 'MAR', 'APR', 'MAY', 'JUN', 'JUL', 'AUG', 'SEP', 'OCT', 'NOV', 'DEC'];

let flightNumber: string | null = null;
let origin: string | null = null;
let destination: string | null = null;
let busSubs: Bond[] | null = null;

export function initLoadsheetBus(bus: Bus): void {
  if (busSubs) {
    return;
  }
  const sub = bus.receiver<FmsData>();
  const flightNumberSub = $feed(sub.topic('fmsFlightNumber'), null);
  const originSub = $feed(sub.topic('fmsOrigin'), null);
  const destinationSub = $feed(sub.topic('fmsDestination'), null);
  busSubs = [
    flightNumberSub.watch((v) => (flightNumber = v), true),
    originSub.watch((v) => (origin = v), true),
    destinationSub.watch((v) => (destination = v), true),
    flightNumberSub,
    originSub,
    destinationSub,
  ];
}

export function loadsheetFlightKnown(): boolean {
  return !!origin;
}

export interface Loadsheet {
  title: string;
  lines: string[];
}

export function buildLoadsheet(kind: 'PRELIM' | 'FINAL'): Loadsheet | null {
  const zfw = Vars.read('L:A32NX_AIRFRAME_ZFW', 'number');
  const gw = Vars.read('L:A32NX_AIRFRAME_GW', 'number');
  if (!(zfw > 0) || !(gw > 0)) {
    return null;
  }

  if (kind === 'FINAL') {
    const mainDoorOpen = Vars.read('fbw/INTERACTIVE_POINT_OPEN_0', 'percent over 100');
    if (mainDoorOpen > 0) {
      return null;
    }
  }

  const macZfw = Vars.read('L:A32NX_AIRFRAME_ZFW_CG_PERCENT_MAC', 'number');
  const macTow = Vars.read('L:A32NX_AIRFRAME_GW_CG_PERCENT_MAC', 'number');
  const tof = Math.max(0, gw - zfw);
  const underload = Math.min(MAX_ZFW_KG - zfw, MAX_GW_KG - gw);
  const pax = totalPax();
  const waterPct = Vars.read('L:A32NX_CABIN_WATER_QTY_PERCENT', 'number');

  const reg = Vars.read('fbw/ATC_ID', 'string') || '';
  const airline = (Vars.read('fbw/ATC_AIRLINE', 'string') || '').toUpperCase();
  const flt = flightNumber ?? Vars.read('fbw/ATC_FLIGHT_NUMBER', 'string') ?? '';
  const callsign = `${airline}${flt}`.trim();

  const zuluSeconds = Vars.read('E:fbw/ZULU_TIME', 'seconds');
  const hh = Math.floor(zuluSeconds / 3600) % 24;
  const mm = Math.floor(zuluSeconds / 60) % 60;
  const day = Vars.read('E:fbw/ZULU_DAY_OF_MONTH', 'number');
  const month = MONTHS[Math.max(0, Math.min(11, Vars.read('E:fbw/ZULU_MONTH_OF_YEAR', 'number') - 1))];
  const year = Vars.read('E:fbw/ZULU_YEAR', 'number') % 100;

  const lines: string[] = [];
  lines.push(`${callsign}/${pad2(day)} ${pad2(day)}/${month}/${pad2(year)}`);

  const route = [origin, destination, reg].filter((x) => !!x).join(' ');
  if (route) {
    lines.push(route);
  }

  lines.push(`ZFW ${fmt(zfw)} MAX ${fmt(MAX_ZFW_KG)}`);
  lines.push(`TOF ${fmt(tof)}`);
  lines.push(`TOW ${fmt(gw)} MAX ${fmt(MAX_GW_KG)}`);
  lines.push(`MACZFW ${macZfw.toFixed(1)}`);
  lines.push(`MACTOW ${macTow.toFixed(1)}`);
  lines.push(`UNDLD ${fmt(underload)}`);
  lines.push(`PAX TTL ${pax}`);
  if (waterPct > 0) {
    lines.push(`POTABLE WATER ${Math.round(waterPct)}PCT`);
  }
  lines.push(`END/${callsign}`);

  return {
    title: `LOADSHEET ${kind} ${pad2(hh)}${pad2(mm)}`,
    lines,
  };
}

// Copyright (c) 2026 FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0

import { pluginJson } from './PluginLink';

const G = 9.80665;
const S = 845;
const KT = 0.514444;
const FT = 0.3048;

export type ToConf = '1+F' | '2' | '3';
export type LdgConf = '3' | 'FULL';

const VLS: Record<string, [number[], number[]]> = {
  '1+F': [
    [123, 133, 144, 154, 163, 172, 180, 188],
    [123, 130, 140, 150, 159, 167, 176, 183],
  ],
  '2': [
    [123, 128, 138, 148, 157, 165, 173, 181],
    [123, 125, 135, 144, 153, 161, 169, 176],
  ],
  '3': [
    [123, 124, 134, 144, 152, 161, 169, 176],
    [123, 123, 131, 140, 148, 156, 164, 171],
  ],
  FULL: [
    [123, 123, 131, 140, 148, 157, 165, 173],
    [123, 123, 127, 136, 144, 152, 160, 168],
  ],
};

const LD: Record<ToConf, [number, number]> = { '1+F': [0.074, 13.2], '2': [0.081, 12.1], '3': [0.092, 10.6] };

export interface RunwayCondition {
  mu: number;
  roll: number;
  wet: boolean;
  rev: boolean;
}

export const TO_COND: Record<'DRY' | 'WET', RunwayCondition> = {
  DRY: { mu: 0.42, roll: 0.015, wet: false, rev: false },
  WET: { mu: 0.25, roll: 0.02, wet: true, rev: true },
};

export const LDG_MU: Record<'DRY' | 'GOOD' | 'MEDIUM', number> = { DRY: 0.38, GOOD: 0.28, MEDIUM: 0.17 };

export const AUTOBRAKE: Record<string, number> = { LO: 1.7, '2': 2.2, '3': 3.0, HI: 3.5, MAX: 99 };

export const LINEUP: Record<string, [number, number]> = { '0': [0, 0], '90': [24, 45], '180': [35, 56] };

export interface PerfCalibration {
  thrust: number;
  roll: number;
  brake: number;
  takeoffs: number;
  landings: number;
}

export const NO_CALIBRATION: PerfCalibration = { thrust: 1, roll: 1, brake: 1, takeoffs: 0, landings: 0 };

export async function loadCalibration(): Promise<PerfCalibration> {
  const r = await pluginJson<{ factors?: Partial<PerfCalibration> }>('/perf/cal');
  const f = r?.factors;
  if (!f) {
    return NO_CALIBRATION;
  }
  const clamp = (v: unknown) => (typeof v === 'number' && Number.isFinite(v) ? Math.min(1.3, Math.max(0.7, v)) : 1);
  return { thrust: clamp(f.thrust), roll: clamp(f.roll), brake: clamp(f.brake), takeoffs: f.takeoffs ?? 0, landings: f.landings ?? 0 };
}

export function calibrationText(c: PerfCalibration): string {
  return c.takeoffs + c.landings === 0
    ? 'NO X-PLANE CALIBRATION YET: FACTORS ARE LEARNT FROM YOUR TAKE-OFFS AND LANDINGS'
    : `X-PLANE CALIBRATION FROM ${c.takeoffs} T/O AND ${c.landings} LDG: THRUST ${c.thrust.toFixed(2)}, ROLL ${c.roll.toFixed(2)}, BRAKING ${c.brake.toFixed(2)}`;
}

export interface Atmos {
  pa: number;
  delta: number;
  isa: number;
  sigma: number;
  rho: number;
}

export function atmos(elevFt: number, qnh: number, oat: number): Atmos {
  const pa = elevFt + (1013.25 - qnh) * 27.3;
  const delta = Math.pow(Math.max(0.2, 1 - 6.8756e-6 * pa), 5.2559);
  const isa = 15 - (1.98 * pa) / 1000;
  const sigma = (delta * 288.15) / (oat + 273.15);
  return { pa, delta, isa, sigma, rho: 1.225 * sigma };
}

export function vls(conf: string, tons: number, cg: number): number {
  const t = VLS[conf];
  const at = (row: number[]) => {
    const x = Math.max(350, Math.min(600, tons));
    const i = Math.min(6, Math.floor((x - 250) / 50));
    const v = row[i] + (row[i + 1] - row[i]) * ((x - 250) / 50 - i);
    return tons >= 350 ? v : v * Math.sqrt(tons / 350);
  };
  const k = Math.max(0, Math.min(1, (cg - 29) / 14));
  return at(t[0]) * (1 - k) + at(t[1]) * k;
}

const vmcg = (pa: number) => 118 - (0.6 * pa) / 1000;
const vmca = (pa: number) => 123 - (0.8 * pa) / 1000;

export type AntiIce = 'OFF' | 'ENG' | 'ENG+WING';

function engineThrust(ratingKn: number, atm: Atmos, tempEff: number, packs: boolean, ai: AntiIce, cal: PerfCalibration): number {
  const flat = atm.isa + 15;
  const temp = tempEff <= flat ? 1 : Math.max(0.35, 1 - 0.0095 * (tempEff - flat));
  const bleed = (packs ? 0.988 : 1) * (ai === 'ENG+WING' ? 0.975 : ai === 'ENG' ? 0.99 : 1);
  return ratingKn * 1000 * Math.pow(atm.delta, 0.8) * temp * bleed * cal.thrust;
}

const thrustAt = (f0: number, tasKt: number) => f0 * (1 - 0.00165 * tasKt + 1.3e-6 * tasKt * tasKt);

function ground(m: number, cd: number, mu: number, slope: number, atm: Atmos, f0: number, engines: number, v0: number, v1: number, hw: number, roll: number): number {
  let v = v0;
  let x = 0;
  let t = 0;
  const dt = 0.05;
  const sin = Math.sin(Math.atan(slope / 100));
  while (v < v1 && t < 200) {
    const q = 0.5 * atm.rho * v * v;
    const lift = q * S * 0.45;
    const a = (engines * thrustAt(f0, v / KT) - q * S * cd - mu * Math.max(0, m * G - lift) - m * G * sin) / m;
    if (a <= 0.01) {
      return Infinity;
    }
    v += a * dt;
    x += (v - hw) * dt;
    t += dt;
  }
  return x * roll;
}

function air(m: number, conf: ToConf, f0: number, engines: number, vlof: number, v2: number, hw: number, screenFt: number): number {
  const ld = LD[conf][1];
  const h = screenFt * FT;
  const v = (vlof + v2) / 2;
  const thrust = engines * thrustAt(f0, v / KT) - (engines < 4 ? 0.012 * m * G : 0);
  const gamma = thrust / (m * G) - 1 / (ld - 3.2);
  if (gamma <= 0.002) {
    return Infinity;
  }
  return ((h / (0.5 * gamma)) * (v - hw)) / v;
}

export interface TakeoffInput {
  atm: Atmos;
  oat: number;
  towT: number;
  cg: number;
  conf: ToConf | 'AUTO';
  flex: boolean;
  deratePct: number;
  ratingKn: number;
  packs: boolean;
  ai: AntiIce;
  slope: number;
  cond: RunwayCondition;
  toda: number;
  asda: number;
  lineup: [number, number];
  hw: number;
  cal: PerfCalibration;
}

export interface TakeoffResult {
  ok: boolean;
  limits: string[];
  v1: number;
  vr: number;
  v2: number;
  conf: ToConf;
  flex: number | null;
  todAeo: number;
  todOei: number;
  asd: number;
  toda: number;
  asda: number;
  grad2: number;
  trim: number;
  limitTowT: number;
}

function takeoffCase(p: TakeoffInput, tempEff: number, conf: ToConf, dryCheck = false): Omit<TakeoffResult, 'flex' | 'trim' | 'limitTowT'> {
  const atm = p.atm;
  const m = p.towT * 1000;
  const f0 = engineThrust(p.ratingKn, atm, tempEff, p.packs, p.ai, p.cal) * (1 - p.deratePct / 100);
  const [cd, ld] = LD[conf];
  const vsCas = vls(conf, p.towT, p.cg) / 1.23;
  const toTas = (cas: number) => cas / Math.sqrt(atm.sigma);
  const vmcRatio = Math.sqrt(1 - p.deratePct / 100);
  const vmcgV = vmcg(atm.pa) * vmcRatio;
  const vmcaV = vmca(atm.pa) * vmcRatio;
  const v2 = Math.ceil(Math.max(1.13 * vsCas, 1.1 * vmcaV));
  const vr = Math.ceil(Math.max(1.05 * vmcaV, v2 - 6));
  const hw = p.hw >= 0 ? p.hw * 0.5 : p.hw * 1.5;
  const hwM = hw * KT;
  const vrT = toTas(vr) * KT;
  const v2T = toTas(v2) * KT;
  const vlofT = toTas(vr + 4) * KT;
  const r = p.cal.roll;
  const gnd = ground(m, cd, p.cond.roll, p.slope, atm, f0, 4, 0, vrT, hwM, r);
  const rot = ground(m, cd * 1.15, p.cond.roll * 0.4, p.slope, atm, f0, 4, vrT, vlofT, hwM, r);
  const todAeo = 1.15 * (gnd + rot + air(m, conf, f0, 4, vlofT, v2T * 1.04, hwM, 35));
  const grad2 = (3 * thrustAt(f0, toTas(v2)) - 0.012 * m * G) / (m * G) - 1 / ld;
  const vlofGs = toTas(vr + 4) - hw;
  const sin = Math.sin(Math.atan(p.slope / 100));
  const caseFor = (v1: number) => {
    const v1T = toTas(v1) * KT;
    const vefT = Math.max(1, toTas(v1 - 3) * KT);
    const a1 = ground(m, cd, p.cond.roll, p.slope, atm, f0, 4, 0, vefT, hwM, r);
    const a2 = ground(m, cd, p.cond.roll, p.slope, atm, f0, 3, vefT, vrT, hwM, r);
    const a3 = ground(m, cd * 1.15, p.cond.roll * 0.4, p.slope, atm, f0, 3, vrT, toTas(vr + 3) * KT, hwM, r);
    const todOei = a1 + a2 + a3 + air(m, conf, f0, 3, toTas(vr + 3) * KT, v2T, hwM, p.cond.wet ? 15 : 35);
    const acc = ground(m, cd, p.cond.roll, p.slope, atm, f0, 4, 0, v1T, hwM, r);
    let v = v1T;
    let x = acc + 2 * (v1T - hwM);
    const mu = p.cond.mu * p.cal.brake;
    for (let t = 0; v > 0.5 && t < 120; t += 0.05) {
      const q = 0.5 * atm.rho * v * v;
      const rev = p.cond.rev ? 2 * 0.18 * thrustAt(f0, v / KT) * Math.min(1, v / (60 * KT)) : 0;
      const dec = mu * G * Math.max(0.2, 1 - (q * S * 0.2) / (m * G)) + (q * S * (cd + 0.06) + rev - 4 * 0.06 * f0) / m + G * sin;
      v -= Math.max(0.3, dec) * 0.05;
      x += Math.max(0, v - hwM) * 0.05;
    }
    return { v1, todOei, asd: x, ke: 0.5 * m * v1T * v1T };
  };
  const toda = p.toda - p.lineup[0];
  const asda = p.asda - p.lineup[1];
  let best: (ReturnType<typeof caseFor> & { margin: number }) | null = null;
  for (let v1 = Math.ceil(vmcgV); v1 <= vr; v1++) {
    const c = caseFor(v1);
    const margin = Math.min(toda - c.todOei, asda - c.asd);
    if (!best || margin > best.margin) {
      best = { ...c, margin };
    }
  }
  if (!best) {
    best = { ...caseFor(vr), margin: -Infinity };
  }
  const limits: string[] = [];
  if (todAeo > toda) limits.push('RUNWAY (TOD AEO)');
  if (best.todOei > toda) limits.push('RUNWAY (TOD OEI)');
  if (best.asd > asda) limits.push('RUNWAY (ASD)');
  if (grad2 < 0.03) limits.push('SECOND SEGMENT');
  if (vlofGs > 204) limits.push('TYRE SPEED');
  if (best.ke > 16 * 130e6) limits.push('BRAKE ENERGY');
  if (vr < vmcgV) limits.push('VMCG');
  if (p.cond.wet && !dryCheck) {
    const dry = takeoffCase({ ...p, cond: TO_COND.DRY }, tempEff, conf, true);
    for (const l of dry.limits) {
      if (!limits.includes(l)) limits.push(`${l} (DRY)`);
    }
  }
  return { ok: limits.length === 0, limits, v1: best.v1, vr, v2, conf, todAeo, todOei: best.todOei, asd: best.asd, toda, asda, grad2 };
}

export function trimFor(cg: number): number {
  if (cg < 29) return 5.8;
  if (cg <= 34.5) return 5.8 - ((cg - 29) * 56) / 275;
  if (cg <= 43) return 4.68 - ((cg - 34.5) * 243) / 425;
  return -0.18;
}

function limitWeight(p: TakeoffInput, conf: ToConf): number {
  let lo = 250;
  let hi = 620;
  for (let i = 0; i < 18; i++) {
    const mid = (lo + hi) / 2;
    if (takeoffCase({ ...p, towT: mid }, p.oat, conf).ok) lo = mid;
    else hi = mid;
  }
  return lo;
}

export function takeoff(p: TakeoffInput): TakeoffResult {
  const confs: ToConf[] = p.conf === 'AUTO' ? ['1+F', '2', '3'] : [p.conf];
  const flat = p.atm.isa + 15;
  const tmax = p.atm.isa + 70;
  type Scored = Omit<TakeoffResult, 'trim' | 'limitTowT'>;
  let best: Scored | null = null;
  const score = (x: Scored) => (x.ok ? 1000 + (x.flex ?? 0) * 10 + (x.conf === '2' ? 0.5 : 0) : -1);
  for (const conf of confs) {
    const toga = takeoffCase(p, p.oat, conf);
    if (!toga.ok) {
      if (!best || (!best.ok && toga.todAeo < best.todAeo)) best = { ...toga, flex: null };
      continue;
    }
    let flex: number | null = null;
    if (p.flex) {
      const togaF0 = engineThrust(p.ratingKn, p.atm, p.oat, p.packs, p.ai, p.cal);
      let lo = Math.max(p.oat + 1, Math.ceil(flat));
      let hi = Math.floor(tmax);
      const usable = (t: number) => engineThrust(p.ratingKn, p.atm, t, p.packs, p.ai, p.cal) >= 0.75 * togaF0 && takeoffCase(p, t, conf).ok;
      if (lo <= hi && usable(lo)) {
        while (lo < hi) {
          const mid = Math.ceil((lo + hi + 1) / 2);
          if (usable(mid)) lo = mid;
          else hi = mid - 1;
        }
        flex = lo;
      }
    }
    const r: Scored = flex !== null ? { ...takeoffCase(p, flex, conf), flex } : { ...toga, flex: null };
    if (!best || score(r) > score(best)) best = r;
  }
  const b = best as Scored;
  return { ...b, trim: trimFor(p.cg), limitTowT: limitWeight(p, b.conf) };
}

export interface LandingInput {
  atm: Atmos;
  lwT: number;
  cg: number;
  conf: LdgConf;
  mu: number;
  rev: boolean;
  autoland: boolean;
  slope: number;
  hw: number;
  lda: number;
  cal: PerfCalibration;
}

export interface LandingRow {
  ab: string;
  actual: number;
  inflight: number;
  dispatch: number;
}

export interface LandingResult {
  vls: number;
  vapp: number;
  rows: LandingRow[];
}

export function landing(p: LandingInput, modes: string[]): LandingResult {
  const atm = p.atm;
  const m = p.lwT * 1000;
  const vlsV = Math.max(vls(p.conf, p.lwT, p.cg), 123);
  const vapp = Math.round(vlsV + Math.min(15, Math.max(5, p.hw / 3)));
  const hwF = p.hw >= 0 ? p.hw * 0.5 : p.hw * 1.5;
  const tas = (cas: number) => cas / Math.sqrt(atm.sigma);
  const gsTd = (tas(vapp - 5) - hwF) * KT;
  const airDist = 430 * ((tas(vapp) - hwF) / 140);
  const mu = p.mu * p.cal.brake;
  const sin = Math.sin(Math.atan(p.slope / 100));
  const run = (target: number) => {
    let v = tas(vapp - 5) * KT;
    let x = airDist + 2.5 * gsTd;
    for (let t = 0; v > 0.5 && t < 200; t += 0.05) {
      const q = 0.5 * atm.rho * v * v;
      const aero = (q * S * 0.16) / m;
      const rev = p.rev ? (2 * 0.2 * 350e3 * atm.delta * Math.min(1, Math.max(0, (v / KT - 70) / 70))) / m : 0;
      const brakeMax = mu * G * Math.max(0.3, 1 - (q * S * 0.15) / (m * G));
      const dec = Math.min(target, brakeMax + aero + rev) + G * sin;
      v -= Math.max(0.3, dec) * 0.05;
      x += Math.max(0, v - hwF * KT) * 0.05;
    }
    return x;
  };
  const rows = modes.map((ab) => {
    const d = run(AUTOBRAKE[ab] ?? AUTOBRAKE.MAX);
    return { ab, actual: d, inflight: d * 1.15, dispatch: (d / 0.6) * (p.mu < 0.3 ? 1.15 : 1) };
  });
  return { vls: Math.round(vlsV), vapp, rows };
}

export function crosswindLimitKt(mu: number, autoland: boolean): number {
  if (autoland) return 20;
  return mu >= 0.38 ? 40 : mu >= 0.28 ? 38 : 29;
}

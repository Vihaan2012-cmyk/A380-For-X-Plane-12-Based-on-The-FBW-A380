// Copyright (c) 2026 FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0

export const WING_AREA_M2 = 845;
export const CD0 = 0.0135;
export const K_INDUCED = 0.045;
export const MAX_CERT_ALT_FT = 43100;
export const G = 9.80665;
const MCL_THRUST_N = 290000;
const MCT_THRUST_N = 242400;
const TSFC_CRUISE = 1.5e-5;
const TSFC_LOW = 1.3e-5;
const BUFFET_MARGIN_G = 1.3;
const MIN_ROC_FPM = 300;

export interface Atmosphere {
  pressurePa: number;
  temperatureK: number;
  densityKgM3: number;
  sigma: number;
  delta: number;
  soundMs: number;
}

export function atmosphere(pressureAltFt: number, isaDevC = 0): Atmosphere {
  const h = pressureAltFt * 0.3048;
  let tIsa: number;
  let p: number;
  if (h <= 11000) {
    tIsa = 288.15 - 0.0065 * h;
    p = 101325 * (tIsa / 288.15) ** 5.255876;
  } else {
    tIsa = 216.65;
    p = 22632.06 * Math.exp((-G * (h - 11000)) / (287.053 * 216.65));
  }
  const t = tIsa + isaDevC;
  const rho = p / (287.053 * t);
  return { pressurePa: p, temperatureK: t, densityKgM3: rho, sigma: rho / 1.225, delta: p / 101325, soundMs: Math.sqrt(1.4 * 287.053 * t) };
}

export function buffetCl(mach: number): number {
  return Math.max(0.45, 0.7 - 1.2 * Math.max(0, mach - 0.85) + 0.3 * Math.max(0, 0.8 - mach));
}

export function dragCoefficient(cl: number, mach: number): number {
  const mdd = 0.875 - 0.08 * (cl - 0.45);
  const wave = 20 * Math.max(0, mach - mdd) ** 4;
  const rise = 0.12 * Math.max(0, cl - 0.42) ** 2;
  return CD0 + K_INDUCED * cl * cl + rise + wave;
}

export interface CruisePoint {
  fl: number;
  mach: number;
  tasKt: number;
  cl: number;
  dragN: number;
  fuelFlowKgH: number;
  specificRangeNmPerT: number;
  thrustPerEngineN: number;
}

export function climbThrustPerEngine(atm: Atmosphere, isaDevC: number): number {
  const tempLapse = 1 - 0.006 * Math.max(0, isaDevC - 10);
  return MCL_THRUST_N * atm.sigma ** 1.12 * tempLapse;
}

export function continuousThrustPerEngine(atm: Atmosphere, isaDevC: number): number {
  const tempLapse = 1 - 0.006 * Math.max(0, isaDevC - 10);
  return MCT_THRUST_N * atm.sigma ** 1.12 * tempLapse;
}

export function cruise(weightKg: number, fl: number, mach: number, isaDevC = 0, fuelFactor = 1, engines = 4): CruisePoint {
  const atm = atmosphere(fl * 100, isaDevC);
  const q = 0.7 * atm.pressurePa * mach * mach;
  const cl = (weightKg * G) / (q * WING_AREA_M2);
  const outDrag = engines < 4 ? 1 + 0.025 * (4 - engines) : 1;
  const drag = q * WING_AREA_M2 * dragCoefficient(cl, mach) * outDrag;
  const tsfc = TSFC_CRUISE * (1 + 0.2 * (mach - 0.85)) * Math.sqrt(atm.temperatureK / 216.65);
  const ff = tsfc * drag * 3600 * fuelFactor;
  const tasKt = (mach * atm.soundMs) / 0.514444;
  return { fl, mach, tasKt, cl, dragN: drag, fuelFlowKgH: ff, specificRangeNmPerT: tasKt / (ff / 1000), thrustPerEngineN: drag / engines };
}

export function recMaxFl(weightKg: number, mach: number, isaDevC = 0): number {
  let best = 100;
  for (let fl = 100; fl * 100 <= MAX_CERT_ALT_FT; fl += 10) {
    const atm = atmosphere(fl * 100, isaDevC);
    const q = 0.7 * atm.pressurePa * mach * mach;
    const clNeeded = (BUFFET_MARGIN_G * weightKg * G) / (q * WING_AREA_M2);
    if (clNeeded > buffetCl(mach)) {
      break;
    }
    const point = cruise(weightKg, fl, mach, isaDevC);
    const tasMs = point.tasKt * 0.514444;
    const climbMargin = (weightKg * G * (MIN_ROC_FPM * 0.3048)) / 60 / tasMs;
    if (4 * climbThrustPerEngine(atm, isaDevC) < point.dragN + climbMargin) {
      break;
    }
    best = fl;
  }
  return Math.min(best, Math.floor(MAX_CERT_ALT_FT / 1000) * 10);
}

export function optimumFl(weightKg: number, mach: number, isaDevC = 0): number {
  const max = recMaxFl(weightKg, mach, isaDevC);
  let bestFl = 100;
  let bestSr = 0;
  for (let fl = 100; fl <= max; fl += 10) {
    const sr = cruise(weightKg, fl, mach, isaDevC).specificRangeNmPerT;
    if (sr > bestSr) {
      bestSr = sr;
      bestFl = fl;
    }
  }
  return bestFl;
}

export function greenDotCl(): number {
  return Math.sqrt(CD0 / K_INDUCED);
}

export function greenDotKcas(weightKg: number, fl: number): number {
  const atm = atmosphere(fl * 100);
  const tas = Math.sqrt((2 * weightKg * G) / (atm.densityKgM3 * WING_AREA_M2 * greenDotCl()));
  return (tas * Math.sqrt(atm.sigma)) / 0.514444;
}

export function greenDotDragN(weightKg: number): number {
  return weightKg * G * 2 * Math.sqrt(CD0 * K_INDUCED);
}

export function engineOutCeilingFl(weightKg: number, engines: number, isaDevC = 0): number {
  const drag = greenDotDragN(weightKg) * (1 + 0.025 * (4 - engines));
  let best = 0;
  for (let fl = 0; fl * 100 <= MAX_CERT_ALT_FT; fl += 5) {
    const atm = atmosphere(fl * 100, isaDevC);
    if (engines * continuousThrustPerEngine(atm, isaDevC) < drag) {
      break;
    }
    best = fl;
  }
  return best;
}

export interface Holding {
  kcas: number;
  fuelFlowKgH: number;
}

export function holding(weightKg: number, fl: number, isaDevC = 0, engines = 4): Holding {
  const atm = atmosphere(fl * 100, isaDevC);
  const kcas = greenDotKcas(weightKg, fl);
  const drag = greenDotDragN(weightKg) * (1 + 0.025 * (4 - engines)) * 1.05;
  const tsfc = TSFC_LOW * Math.sqrt(atm.temperatureK / 255);
  return { kcas, fuelFlowKgH: tsfc * drag * 3600 };
}

export interface Descent {
  distanceNm: number;
  timeMin: number;
  fuelKg: number;
}

export function descent(fromFl: number, toElevationFt: number, weightKg: number, tailwindKt = 0): Descent {
  const dh = Math.max(0, fromFl * 100 - toElevationFt);
  const still = (dh / 1000) * 3.1 + 10;
  const avgGs = 330 + tailwindKt;
  const distanceNm = still * (avgGs / 330);
  const timeMin = (distanceNm / avgGs) * 60;
  const fuelKg = (dh / 1000) * 42 * (weightKg / 450000) + 250;
  return { distanceNm, timeMin, fuelKg };
}

export interface PlanResult {
  tripFuelKg: number;
  timeMin: number;
  efobKg: number;
}

export function plan(weightKg: number, fobKg: number, distanceNm: number, fl: number, mach: number, isaDevC = 0, windKt = 0, fuelFactor = 1): PlanResult {
  let w = weightKg;
  let fuel = 0;
  let time = 0;
  let left = distanceNm;
  const step = 50;
  while (left > 0 && fuel < fobKg) {
    const d = Math.min(step, left);
    const p = cruise(w, fl, mach, isaDevC, fuelFactor);
    const gs = Math.max(150, p.tasKt + windKt);
    const hours = d / gs;
    const burn = p.fuelFlowKgH * hours;
    fuel += burn;
    w -= burn;
    time += hours * 60;
    left -= d;
  }
  return { tripFuelKg: fuel, timeMin: time, efobKg: fobKg - fuel };
}

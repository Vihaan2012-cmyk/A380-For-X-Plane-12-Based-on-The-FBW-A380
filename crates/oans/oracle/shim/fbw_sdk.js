import { ConsumerSubject } from './msfs_sdk.js';
export { MathUtils } from '@fbw/shared/MathUtils.ts';
export { Arinc429Word, Arinc429Register, Arinc429SignStatusMatrix } from '@fbw/shared/arinc429.ts';
export { OansMapProjection } from '@fbw/oans/OansMapProjection.ts';
export { BTV_MIN_TOUCHDOWN_ZONE_DISTANCE } from '@fbw/oans/BrakeToVacateConstants.ts';
export { RopsRunwayPrediction } from '@fbw/oans/RopsRunwayPrediction.ts';
export { OansFmsDataStore } from '@fbw/oans/OansFmsDataStore.ts';
import { Arinc429Word } from '@fbw/shared/arinc429.ts';

export const FeatureType = {
  RunwayElement: 0,
  RunwayThreshold: 2,
  PaintedCenterline: 4,
  TaxiwayGuidanceLine: 16,
  RunwayExitLine: 19,
  ParkingStandLocation: 23,
};

export const NearbyFacilityType = { Airport: 0 };

const f32 = new Float32Array(1);
const u32 = new Uint32Array(f32.buffer);

export class Arinc429LocalVarConsumerSubject extends ConsumerSubject {
  static create(consumer) {
    const s = new Arinc429LocalVarConsumerSubject(new Arinc429Word(0));
    if (consumer) consumer.handle((raw) => s.set(new Arinc429Word(raw)));
    return s;
  }
}

class Register {
  constructor() {
    this.value = 0;
    this.ssm = 0;
  }
  writeToSimVar(name) {
    f32[0] = this.value;
    globalThis.__vars[name] = u32[0] + Math.trunc(this.ssm) * 2 ** 32;
  }
}

export class Arinc429RegisterSubject {
  constructor() {
    this.reg = new Register();
    this.subs = [];
  }
  static createEmpty() {
    return new Arinc429RegisterSubject();
  }
  notify() {
    for (const fn of this.subs.slice()) fn(this.reg);
  }
  setValue(v) {
    this.reg.value = v;
    this.notify();
  }
  setSsm(s) {
    this.reg.ssm = s;
    this.notify();
  }
  setValueSsm(v, s) {
    this.reg.value = v;
    this.reg.ssm = s;
    this.notify();
  }
  sub(fn, notify) {
    this.subs.push(fn);
    if (notify) fn(this.reg);
  }
  get() {
    return this.reg;
  }
}

export class RegisteredSimVar {
  static createBoolean(name) {
    return {
      set(v) {
        globalThis.__vars[name] = v ? 1 : 0;
      },
    };
  }
}

export class UpdateThrottler {
  constructor(interval) {
    this.interval = interval;
    this.current = 0;
    this.number = 0;
  }
  canUpdate(dt) {
    this.current += dt;
    const n = Math.floor(this.current / this.interval);
    if (n > this.number) {
      this.number = n;
      return this.interval;
    }
    return -1;
  }
}

export class NearbyFacilityMonitor {
  constructor() {
    this.loc = undefined;
    this.radius = 0;
    this.max = 0;
  }
  setMaxResults(n) {
    this.max = n;
  }
  setRadius(r) {
    this.radius = r;
  }
  setLocation(lat, long) {
    this.loc = lat === undefined ? undefined : { lat, long };
  }
  getCurrentFacilities() {
    if (!this.loc) return [];
    const geo = globalThis.__geo;
    const out = [];
    for (const [ident, a] of Object.entries(globalThis.__airports)) {
      if (geo.distanceTo(this.loc, a.location) <= this.radius) out.push({ type: 0, ident });
    }
    return out.slice(0, this.max);
  }
}

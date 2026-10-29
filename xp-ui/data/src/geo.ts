import { GeoPos3 } from './types.ts';
import type { GeoPosData } from './types.ts';

const EARTH_RADIUS_NM = 3440.065;

export class Geo {
  static DEG_TO_RAD: number = Math.PI / 180;
  static RAD_TO_DEG: number = 180 / Math.PI;

  static wrapDegrees(a: number): number {
    const r = ((a % 360) + 360) % 360;
    return r >= 360 ? 0 : r;
  }

  static angleDelta(a: number, b: number): number {
    const d = Geo.wrapDegrees(b - a);
    return d > 180 ? d - 360 : d;
  }

  static lerpDegrees(from: number, to: number, t: number): number {
    return Geo.wrapDegrees(from + Geo.angleDelta(from, to) * t);
  }

  static distance(from: GeoPosData, to: GeoPosData): number {
    const p1 = from.lat * Geo.DEG_TO_RAD;
    const p2 = to.lat * Geo.DEG_TO_RAD;
    const dp = p2 - p1;
    const dl = (to.long - from.long) * Geo.DEG_TO_RAD;
    const h = Math.sin(dp / 2) ** 2 + Math.cos(p1) * Math.cos(p2) * Math.sin(dl / 2) ** 2;
    return 2 * EARTH_RADIUS_NM * Math.asin(Math.min(1, Math.sqrt(h)));
  }

  static bearing(from: GeoPosData, to: GeoPosData): number {
    const p1 = from.lat * Geo.DEG_TO_RAD;
    const p2 = to.lat * Geo.DEG_TO_RAD;
    const dl = (to.long - from.long) * Geo.DEG_TO_RAD;
    const y = Math.sin(dl) * Math.cos(p2);
    const x = Math.cos(p1) * Math.sin(p2) - Math.sin(p1) * Math.cos(p2) * Math.cos(dl);
    return Geo.wrapDegrees(Math.atan2(y, x) * Geo.RAD_TO_DEG);
  }

  static destination(bearing: number, distance: number, lat: number, long: number): GeoPos3 {
    const p1 = lat * Geo.DEG_TO_RAD;
    const l1 = long * Geo.DEG_TO_RAD;
    const th = bearing * Geo.DEG_TO_RAD;
    const d = distance / EARTH_RADIUS_NM;
    const sinP2 = Math.sin(p1) * Math.cos(d) + Math.cos(p1) * Math.sin(d) * Math.cos(th);
    const p2 = Math.asin(Math.max(-1, Math.min(1, sinP2)));
    const l2 = l1 + Math.atan2(Math.sin(th) * Math.sin(d) * Math.cos(p1), Math.cos(d) - Math.sin(p1) * sinP2);
    let lon = l2 * Geo.RAD_TO_DEG;
    if (lon > 180 || lon < -180) {
      lon = ((((lon + 180) % 360) + 360) % 360) - 180;
    }
    return new GeoPos3(p2 * Geo.RAD_TO_DEG, lon, 0);
  }

  static adfBcd32(frequency: number): number {
    let n = Math.abs(Math.round(frequency * 10));
    let result = 0;
    let scale = 1;
    while (n > 0) {
      result += (n % 10) * scale;
      n = Math.floor(n / 10);
      scale *= 16;
    }
    return result;
  }
}

export const NavMath = { Geo };

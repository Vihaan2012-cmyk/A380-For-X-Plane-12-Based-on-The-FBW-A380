import { decimals, Maths } from './scalar';
import { LinearMeasure, Measure, Dimension, UnitList } from './units';

export interface NumFormatOptions {
  precision?: number;
  maxDigits?: number;
  pad?: number;
  nanString?: string;
}

export class NumFormat {
  static make(options: NumFormatOptions = {}): (number: number) => string {
    const { precision, maxDigits, pad = 0, nanString = 'NaN' } = options;
    const stepped = precision !== undefined && precision > 0;
    const places = stepped ? decimals(precision) : undefined;

    return (input) => {
      if (!Number.isFinite(input)) {
        return nanString;
      }
      let rounded = stepped ? Maths.round(input, precision) : input;
      const split = (value: number, digits: number | undefined): [string, string] => {
        const text = digits === undefined ? String(Math.abs(value)) : Math.abs(value).toFixed(digits);
        const cut = text.indexOf('.');
        return cut < 0 ? [text, ''] : [text.slice(0, cut), text.slice(cut + 1)];
      };
      let [whole, fraction] = split(rounded, places);
      if (maxDigits !== undefined) {
        let keep = fraction.length;
        while (keep > 0 && whole.length + fraction.length > maxDigits) {
          keep = Math.max(0, Math.min(keep - 1, maxDigits - whole.length));
          const scale = Math.pow(10, keep);
          rounded = Math.round(input * scale) / scale;
          [whole, fraction] = split(rounded, keep);
        }
      }
      whole = whole.padStart(pad, '0');
      const sign = rounded < 0 && /[1-9]/.test(whole + fraction) ? '-' : '';
      return fraction.length > 0 ? `${sign}${whole}.${fraction}` : sign + whole;
    };
  }
}

const MONTHS = ['JAN', 'FEB', 'MAR', 'APR', 'MAY', 'JUN', 'JUL', 'AUG', 'SEP', 'OCT', 'NOV', 'DEC'];

function twoDigits(n: number): string {
  return String(n).padStart(2, '0');
}

function fillDateTokens(template: string, when: Date): string {
  const fields: Record<string, string> = {
    YYYY: String(when.getUTCFullYear()).padStart(4, '0'),
    YY: twoDigits(when.getUTCFullYear() % 100),
    MON: MONTHS[when.getUTCMonth()],
    MM: twoDigits(when.getUTCMonth() + 1),
    dd: twoDigits(when.getUTCDate()),
    HH: twoDigits(when.getUTCHours()),
    h: String(when.getUTCHours() % 12 === 0 ? 12 : when.getUTCHours() % 12),
    am: when.getUTCHours() < 12 ? 'am' : 'pm',
    mm: twoDigits(when.getUTCMinutes()),
    ss: twoDigits(when.getUTCSeconds()),
  };
  return template.replace(/\{(\w+)\}/g, (whole, name: string) => fields[name] ?? whole);
}

export class TimeFormat {
  static make(format: string, options: { nanString?: string } = {}): (time: number) => string {
    const fallback = options.nanString ?? 'NaN';
    return (time) => (Number.isFinite(time) ? fillDateTokens(format, new Date(time)) : fallback);
  }
}

export class AngleFormat {
  static make(format: string, unit: Measure<Dimension.Angle>, precision: number, nanString = 'NaN'): (angle: number) => string {
    const step = precision > 0 ? precision : 0;
    return (angle) => {
      if (!Number.isFinite(angle)) {
        return nanString;
      }
      const degrees = UnitList.DEGREE.from(angle, unit as LinearMeasure<Dimension.Angle>);
      const snapped = step > 0 ? Math.round(degrees / step) * step : degrees;
      const negative = snapped < 0;
      const arcMs = Math.round(Math.abs(snapped) * 3600000);
      const d = Math.floor(arcMs / 3600000);
      const m = Math.floor((arcMs % 3600000) / 60000);
      const sec = Math.floor((arcMs % 60000) / 1000);
      const fields: Record<string, string> = {
        d: String(d),
        dd: String(d).padStart(2, '0'),
        ddd: String(d).padStart(3, '0'),
        m: String(m),
        mm: String(m).padStart(2, '0'),
        s: String(sec),
        ss: String(sec).padStart(2, '0'),
        '-': negative ? '-' : '',
        '+': negative ? '' : '+',
      };
      return format.replace(/\{([^{}]*)\}/g, (_all, token: string) => {
        const hemi = /^\+\[([^\]]*)\]-\[([^\]]*)\]$/.exec(token);
        if (hemi) {
          return negative ? hemi[2] : hemi[1];
        }
        return fields[token] ?? '';
      });
    };
  }
}

export interface NavCycle {
  cycle?: number;
  cycleString?: string;
  ident?: string;
  effectiveTimestamp?: number;
  expirationTimestamp?: number;
}

function cycleCode(cycle: NavCycle): string {
  const effective = cycle.effectiveTimestamp;
  const year = effective !== undefined && Number.isFinite(effective) ? twoDigits(new Date(effective).getUTCFullYear() % 100) : '';
  return year + (cycle.cycleString ?? (cycle.cycle !== undefined ? twoDigits(cycle.cycle) : ''));
}

export class CycleFormat {
  static make(format: string): (cycle: NavCycle) => string {
    const outer = format.replace(/\{(eff|exp)\([^)]*\)\}/g, '');
    const unknown = /\{(?!YYCC\})[^}]*\}/.exec(outer);
    if (unknown) {
      throw new Error(`Invalid AIRAC cycle format fragment: ${unknown[0]}`);
    }
    return (cycle) => {
      const stamp = (t: number | undefined) => (t !== undefined && Number.isFinite(t) ? new Date(t) : undefined);
      return format
        .replace(/\{(eff|exp)\(([^)]*)\)\}/g, (_all, which: string, inner: string) => {
          const when = stamp(which === 'eff' ? cycle.effectiveTimestamp : cycle.expirationTimestamp);
          return when ? fillDateTokens(inner, when) : '';
        })
        .replace(/\{YYCC\}/g, cycleCode(cycle));
    };
  }
}

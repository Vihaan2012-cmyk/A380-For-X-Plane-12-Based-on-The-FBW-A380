// Copyright (c) 2026 FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0

import { NXDataStore } from '@flybywiresim/fbw-sdk';

export interface OfpSection {
  group: string;
  title: string;
  level: number;
  text?: string;
  image?: string;
}

export interface Ofp {
  raw: any;
  flight: string;
  origin: string;
  destination: string;
  alternate: string;
  generated: string;
  sections: OfpSection[];
}

let last: Ofp | null = null;

export function lastOfp(): Ofp | null {
  return last;
}

const arr = (v: any): any[] => (Array.isArray(v) ? v : v ? [v] : []);

function htmlText(html: string): string {
  const doc = new DOMParser().parseFromString(html.replace(/<br\s*\/?>/gi, '\n'), 'text/html');
  return (doc.body.textContent ?? '').replace(/\r/g, '').replace(/^\n+/, '').replace(/\s+$/, '');
}

function planSections(html: string): OfpSection[] {
  const body = html.replace(/^<div [^>]+>/, '').replace(/<\/div>$/, '');
  const re = /<!--BKMK\/\/\/(.*?)\/\/\/(\d+)-->/g;
  const marks: { title: string; level: number; at: number; end: number }[] = [];
  for (let m = re.exec(body); m; m = re.exec(body)) {
    marks.push({ title: m[1], level: Number(m[2]), at: m.index, end: m.index + m[0].length });
  }
  if (marks.length === 0) {
    return [{ group: 'OFP', title: 'OPERATIONAL FLIGHT PLAN', level: 0, text: htmlText(body) }];
  }
  const out: OfpSection[] = [];
  const head = htmlText(body.slice(0, marks[0].at));
  if (head.trim()) {
    out.push({ group: 'OFP', title: 'HEADER', level: 0, text: head });
  }
  marks.forEach((m, i) => {
    const text = htmlText(body.slice(m.end, i + 1 < marks.length ? marks[i + 1].at : body.length));
    if (text.trim() || (i + 1 < marks.length && marks[i + 1].level > m.level)) {
      out.push({ group: 'OFP', title: m.title.toUpperCase(), level: Math.min(m.level, 1), text: text.trim() ? text : undefined });
    }
  });
  return out;
}

function weatherText(w: any, a: any): string {
  const lines: string[] = [];
  const add = (label: string, v: any) => {
    const t = arr(v).filter((x) => typeof x === 'string' && x.trim()).join('\n');
    if (t) {
      lines.push(label, t, '');
    }
  };
  add(`DEPARTURE METAR ${a.origin}`, w.orig_metar);
  add(`DEPARTURE TAF ${a.origin}`, w.orig_taf);
  add(`DESTINATION METAR ${a.destination}`, w.dest_metar);
  add(`DESTINATION TAF ${a.destination}`, w.dest_taf);
  add(`ALTERNATE METAR ${a.alternate}`, w.altn_metar);
  add(`ALTERNATE TAF ${a.alternate}`, w.altn_taf);
  return lines.join('\n').trim();
}

export async function fetchOfp(): Promise<Ofp> {
  const id = NXDataStore.getLegacy('CONFIG_OVERRIDE_SIMBRIEF_USERID', '');
  const user = NXDataStore.getLegacy('NAVIGRAPH_USERNAME', '');
  if (!id && !user) {
    throw new Error('SET YOUR SIMBRIEF ID IN THE EFB SETTINGS');
  }
  const query = id ? `userid=${encodeURIComponent(id)}` : `username=${encodeURIComponent(user)}`;
  let raw: any;
  try {
    const r = await fetch(`https://www.simbrief.com/api/xml.fetcher.php?${query}&json=1`, { headers: { Accept: 'application/json' } });
    raw = await r.json();
  } catch {
    throw new Error('SIMBRIEF NOT REACHABLE');
  }
  const status = raw?.fetch?.status;
  if (status && status !== 'Success') {
    throw new Error(`SIMBRIEF: ${String(status).toUpperCase()}`);
  }
  const g = raw.general ?? {};
  const names = {
    origin: raw.origin?.icao_code ?? '',
    destination: raw.destination?.icao_code ?? '',
    alternate: arr(raw.alternate)[0]?.icao_code ?? '',
  };
  const sections = planSections(String(raw.text?.plan_html ?? ''));
  const fpl = String(raw.atc?.flightplan_text ?? '').trim();
  if (fpl) {
    sections.push({ group: 'ATC', title: 'ATC FLIGHT PLAN', level: 0, text: fpl });
  }
  const wx = weatherText(raw.weather ?? {}, names);
  if (wx) {
    sections.push({ group: 'WEATHER', title: 'METAR / TAF', level: 0, text: wx });
  }
  const dir = String(raw.images?.directory ?? '');
  for (const m of arr(raw.images?.map)) {
    if (m?.link) {
      sections.push({ group: 'GRAPHICAL ANALYSIS', title: String(m.name ?? m.link).toUpperCase(), level: 0, image: `${dir}${m.link}` });
    }
  }
  const time = Number(raw.params?.time_generated);
  last = {
    raw,
    flight: `${g.icao_airline ?? ''}${g.flight_number ?? ''}`,
    ...names,
    generated: Number.isFinite(time) && time > 0 ? `${new Date(time * 1000).toISOString().slice(0, 16).replace('T', ' ')}Z` : '',
    sections,
  };
  return last;
}

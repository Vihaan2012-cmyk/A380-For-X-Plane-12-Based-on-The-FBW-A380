// Copyright (c) 2026 FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0

export const PLUGIN_BASE =
  typeof location !== 'undefined' && location.protocol === 'coui:' ? `${location.origin}/__plugin` : 'http://127.0.0.1:8390';

export async function pluginJson<T>(path: string, init?: RequestInit): Promise<T | null> {
  try {
    const r = await fetch(`${PLUGIN_BASE}${path}`, init);
    if (!r.ok) {
      return null;
    }
    return (await r.json()) as T;
  } catch {
    return null;
  }
}

export interface OisDocument {
  path: string;
  name: string;
  folder: string;
  type: 'text' | 'pdf' | 'image';
  bytes: number;
}

export function listDocuments(): Promise<{ files: OisDocument[] } | null> {
  return pluginJson('/oit/docs');
}

export function readDocument(path: string): Promise<{ path: string; text: string } | null> {
  return pluginJson(`/oit/doc?path=${encodeURIComponent(path)}`);
}

export function exportText(name: string, text: string): Promise<{ ok: boolean; path?: string; message?: string } | null> {
  return pluginJson('/oit/export', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ name, text }) });
}

export interface OisMelItem {
  id: string;
  ata: string;
  title: string;
  category: string | null;
  intervalDays: number | null;
  installed: string | null;
  required: string | null;
  placard: string | null;
  conditions: string;
  opsProcedureRequired: boolean;
  maintenanceProcedureRequired: boolean;
  opsProcedure: string | null;
}

export function melChapter(ata: string): Promise<{ items: OisMelItem[] } | null> {
  return pluginJson(`/study/mel?ata=${encodeURIComponent(ata)}`);
}

export type ChartSource = 'navigraph' | 'faa' | 'local';

export interface OisChart {
  id: string;
  name?: string;
  category?: string;
  type_code?: string;
  kind?: 'pdf' | 'image';
  url?: string;
  image_day_url?: string;
  image_night_url?: string;
}

export async function listCharts(source: ChartSource, icao: string): Promise<{ charts: OisChart[]; error?: string }> {
  const path =
    source === 'navigraph' ? `/study/charts?icao=${encodeURIComponent(icao)}` : `/oit/charts?source=${source}&icao=${encodeURIComponent(icao)}`;
  try {
    const r = await fetch(`${PLUGIN_BASE}${path}`);
    const data = await r.json();
    if (data && data.error) {
      return { charts: [], error: String(data.error) };
    }
    if (!r.ok) {
      return { charts: [], error: `THE PLUGIN REPLIED ${r.status}` };
    }
    return { charts: Array.isArray(data) ? data : (data.charts ?? []) };
  } catch {
    return { charts: [], error: 'CHART SERVER NOT AVAILABLE' };
  }
}

export function pluginUrl(url: string): string {
  return url.startsWith('/') ? `${PLUGIN_BASE}${url}` : url;
}

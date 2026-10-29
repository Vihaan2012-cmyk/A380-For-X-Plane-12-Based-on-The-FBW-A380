// Copyright (c) 2023-2024 FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0

/**
 * The static systems catalogue: every failure, component, ECAM alert,
 * circuit breaker and MEL reference the deep systems model knows about.
 *
 * It is *static* -- none of it changes while the aircraft is running -- so it
 * ships as a file beside the EFB rather than crossing the simulator variable
 * boundary. Only live state (which failures are armed, which breakers are
 * open) comes from the sim, and that is a few hundred values rather than a
 * few thousand.
 *
 * It is deliberately NOT imported as a module: at ~3.8 MB it would sit in the
 * EFB's main bundle and be parsed at boot whether or not anyone opens the
 * Study pages. It is fetched once, on demand, and cached for the session.
 *
 * `ataChapterName` is stripped from every row and resolved from `chapters`
 * instead -- it was repeated on all 5,555 failures and cost ~300 KB to say
 * the same 35 things over and over.
 */

export interface AtaChapter {
  number: number;
  name: string;
}

export interface CatalogueFailure {
  id: number;
  name: string;
  ataChapterNumber: number;
  /** The component this failure belongs to, as the registry names it. */
  component: string;
  /** What the failure does, in the registry's own words. */
  cause: string;
  /**
   * What this failure's 0..1 magnitude means physically, where it means
   * something other than a plain fraction. `null` where the registry does
   * not say -- deliberately not filled in with a guess.
   */
  magnitudeSemantics: string | null;
}

export interface CatalogueComponent {
  id: string;
  name: string;
  ataChapterNumber: number;
  /** Which of several identical units this is, where the id encodes one. */
  instance: number | null;
  parameters: { name: string; meaning: string; healthyValue: string }[];
  failures: number[];
}

export interface CatalogueAlert {
  key: string;
  ataChapterNumber: number;
  title: string;
  level: string;
  aural: string | null;
  masterLight: string | null;
  confirmSeconds: number | null;
  inhibitedInPhases: number[];
  statusPageLines: string[];
  inoperativeSystemsListEntries: string[];
  failures: number[];
  procedureLineCount: number;
}

export interface CatalogueBreaker {
  id: string;
  name: string;
  ataChapterNumber: number;
  bus: string;
  ratingAmperes: number;
  kind: string;
  consumer: string;
  /** Where the rating came from; `GENERIC:` prefixed where it is assumed. */
  basis: string;
  panel: string;
  row: number;
  column: number;
  label: string;
  /** False where the breaker exists but protects nothing the model solves. */
  protectsModelledLoad: boolean;
}

export interface CatalogueMelEntry {
  melReference: string;
  ataChapterNumber: number;
  failures: number[];
}

export interface Catalogue {
  schemaVersion: number;
  generatedAtUnixSeconds: number;
  chapters: AtaChapter[];
  failures: CatalogueFailure[];
  components: CatalogueComponent[];
  alerts: CatalogueAlert[];
  breakers: CatalogueBreaker[];
  melEntries: CatalogueMelEntry[];
}

/**
 * Where the catalogue sits once the package is built. Relative so it resolves
 * against whichever instrument path the EFB was loaded from, which differs
 * between the A380X and A32NX trees.
 */
const CATALOGUE_URL = 'catalogue.json';

let cached: Catalogue | null = null;
let inFlight: Promise<Catalogue> | null = null;

/**
 * Fetch the catalogue, once per session. Concurrent callers share one
 * request rather than each pulling 3.8 MB.
 */
export function loadCatalogue(): Promise<Catalogue> {
  if (cached) {
    return Promise.resolve(cached);
  }
  if (inFlight) {
    return inFlight;
  }

  inFlight = fetch(CATALOGUE_URL)
    .then((response) => {
      if (!response.ok) {
        throw new Error(`catalogue.json: HTTP ${response.status}`);
      }
      return response.json();
    })
    .then((data: Catalogue) => {
      cached = data;
      inFlight = null;
      return data;
    })
    .catch((error) => {
      inFlight = null;
      throw error;
    });

  return inFlight;
}

/** ATA chapter number to its title, from the catalogue's own list. */
export function chapterNames(catalogue: Catalogue): Map<number, string> {
  return new Map(catalogue.chapters.map((chapter) => [chapter.number, chapter.name]));
}

/**
 * Group any catalogue row carrying an `ataChapterNumber` by chapter, in
 * ascending chapter order, dropping chapters nothing landed in.
 */
export function byChapter<T extends { ataChapterNumber: number }>(rows: T[]): Map<number, T[]> {
  const grouped = new Map<number, T[]>();
  rows.forEach((row) => {
    const existing = grouped.get(row.ataChapterNumber);
    if (existing) {
      existing.push(row);
    } else {
      grouped.set(row.ataChapterNumber, [row]);
    }
  });
  return new Map([...grouped.entries()].sort(([a], [b]) => a - b));
}

/** A breaker panel's rows, each row's breakers left to right. */
export function panelGrid(breakers: CatalogueBreaker[]): Map<number, CatalogueBreaker[]> {
  const rows = new Map<number, CatalogueBreaker[]>();
  breakers.forEach((breaker) => {
    const existing = rows.get(breaker.row);
    if (existing) {
      existing.push(breaker);
    } else {
      rows.set(breaker.row, [breaker]);
    }
  });
  rows.forEach((row) => row.sort((a, b) => a.column - b.column));
  return new Map([...rows.entries()].sort(([a], [b]) => a - b));
}

/** `secondaryPowerCentreForward` -> `Secondary Power Centre Forward`. */
export function humanisePanel(panel: string): string {
  return panel.replace(/([A-Z])/g, ' $1').replace(/^./, (c) => c.toUpperCase());
}

// Copyright (c) 2023-2024 FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0

import React from 'react';
import { ScrollableContainer } from '../../UtilComponents/ScrollableContainer';
import { Catalogue, CatalogueMelEntry, byChapter, chapterNames } from '../catalogue';

interface MelListProps {
  catalogue: Catalogue;
  query: string;
}

/**
 * The Minimum Equipment List cross-reference: which modelled failures each
 * MEL item covers.
 *
 * This is the *cross-reference only*. The operator's own MEL text is
 * copyrighted and is not shipped -- what is here is the mapping from a
 * reference to the failures it concerns, which is ours.
 */
export const MelList = ({ catalogue, query }: MelListProps) => {
  const failuresById = new Map(catalogue.failures.map((failure) => [failure.id, failure]));

  const matches = (entry: CatalogueMelEntry): boolean => {
    if (query === '') {
      return true;
    }
    if (entry.melReference.toUpperCase().includes(query)) {
      return true;
    }
    return entry.failures.some((id) => failuresById.get(id)?.name.toUpperCase().includes(query));
  };

  const filtered = catalogue.melEntries.filter(matches);
  const grouped = byChapter(filtered);
  const names = chapterNames(catalogue);

  return (
    <ScrollableContainer height={46}>
      {filtered.length === 0 && <p className="mt-4 text-theme-unselected">Nothing matches that search.</p>}

      {[...grouped.entries()].map(([chapter, entries]) => (
        <div key={chapter} className="mb-3">
          <h2 className="sticky top-0 bg-theme-body py-1 font-bold">{names.get(chapter) ?? `ATA ${chapter}`}</h2>

          <div className="space-y-1">
            {entries.map((entry) => {
              // The reference's own chapter digits and the chapter its
              // failures live in can disagree -- an MMEL item may sit under
              // the system that owns the part rather than the one the part
              // belongs to. Shown as it is rather than quietly reconciled.
              const failureChapters = new Set(
                entry.failures.map((id) => failuresById.get(id)?.ataChapterNumber).filter((n) => n !== undefined),
              );
              const crossChapter = !failureChapters.has(entry.ataChapterNumber);

              return (
                <div key={entry.melReference} className="rounded-md border border-theme-accent px-4 py-2">
                  <div className="flex flex-row items-baseline justify-between space-x-4">
                    <span className="font-mono font-bold">{entry.melReference}</span>
                    <span className="shrink-0 text-sm text-theme-unselected">
                      {entry.failures.length} {entry.failures.length === 1 ? 'failure' : 'failures'}
                    </span>
                  </div>

                  <div className="mt-1 space-y-0.5">
                    {entry.failures.map((id) => {
                      const failure = failuresById.get(id);
                      return (
                        <div key={id} className="flex flex-row items-baseline justify-between space-x-4 text-sm">
                          <span>{failure?.name ?? `Failure ${id}`}</span>
                          <span className="shrink-0 font-mono text-theme-unselected">{id}</span>
                        </div>
                      );
                    })}
                  </div>

                  {crossChapter && (
                    <p className="mt-2 text-xs text-theme-unselected">
                      Filed under ATA {entry.ataChapterNumber}, covering failures in{' '}
                      {[...failureChapters].sort((a, b) => a - b).join(', ')}.
                    </p>
                  )}
                </div>
              );
            })}
          </div>
        </div>
      ))}
    </ScrollableContainer>
  );
};

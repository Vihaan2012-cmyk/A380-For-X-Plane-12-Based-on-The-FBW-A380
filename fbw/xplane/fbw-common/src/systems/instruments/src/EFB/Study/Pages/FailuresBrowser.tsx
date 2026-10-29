// Copyright (c) 2023-2024 FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0

import React, { useState } from 'react';
import { ChevronDown, ChevronRight } from 'react-bootstrap-icons';
import { ScrollableContainer } from '../../UtilComponents/ScrollableContainer';
import { Catalogue, CatalogueFailure, byChapter, chapterNames } from '../catalogue';

interface FailuresBrowserProps {
  catalogue: Catalogue;
  query: string;
}

/**
 * Every failure the systems model registers, grouped by ATA chapter and
 * resolved to the component it belongs to and what it actually does.
 *
 * The aircraft's own Failures page lists what the crew may arm. This lists
 * what the model can represent, which is a much longer list and is the point
 * of the page: a failure here names a real part and a real mechanism.
 */
export const FailuresBrowser = ({ catalogue, query }: FailuresBrowserProps) => {
  const [openChapters, setOpenChapters] = useState<Set<number>>(new Set());

  const matches = (failure: CatalogueFailure): boolean => {
    if (query === '') {
      return true;
    }
    return (
      failure.name.toUpperCase().includes(query) ||
      failure.component.toUpperCase().includes(query) ||
      failure.cause.toUpperCase().includes(query) ||
      failure.id.toString().includes(query)
    );
  };

  const filtered = catalogue.failures.filter(matches);
  const grouped = byChapter(filtered);
  const names = chapterNames(catalogue);

  const toggle = (chapter: number) => {
    const next = new Set(openChapters);
    if (next.has(chapter)) {
      next.delete(chapter);
    } else {
      next.add(chapter);
    }
    setOpenChapters(next);
  };

  // A search narrow enough to be readable opens everything it matched, so a
  // hit is not hidden one click away inside a collapsed chapter.
  const expandAll = query !== '' && filtered.length <= 60;

  return (
    <ScrollableContainer height={46}>
      {filtered.length === 0 && <p className="mt-4 text-theme-unselected">Nothing matches that search.</p>}

      {[...grouped.entries()].map(([chapter, failures]) => {
        const open = expandAll || openChapters.has(chapter);

        return (
          <div key={chapter} className="mb-2">
            <button
              type="button"
              className="flex w-full flex-row items-center justify-between rounded-md bg-theme-accent px-4 py-2 text-left transition duration-100 hover:bg-theme-highlight hover:text-theme-body"
              onClick={() => toggle(chapter)}
            >
              <div className="flex flex-row items-center space-x-3">
                {open ? <ChevronDown size={20} /> : <ChevronRight size={20} />}
                <span className="font-bold">{names.get(chapter) ?? `ATA ${chapter}`}</span>
              </div>
              <span className="text-sm opacity-75">{failures.length}</span>
            </button>

            {open && (
              <div className="mt-1 space-y-1">
                {failures.map((failure) => (
                  <div key={failure.id} className="rounded-md border border-theme-accent px-4 py-2">
                    <div className="flex flex-row items-baseline justify-between space-x-4">
                      <span className="font-bold">{failure.name}</span>
                      <span className="shrink-0 font-mono text-sm text-theme-unselected">{failure.id}</span>
                    </div>
                    <p className="mt-1 text-sm text-theme-unselected">{failure.cause}</p>
                    <div className="mt-1 flex flex-row flex-wrap items-baseline gap-x-4 text-sm">
                      <span className="text-theme-highlight">{failure.component}</span>
                      {failure.magnitudeSemantics !== null && (
                        <span className="text-theme-unselected">magnitude: {failure.magnitudeSemantics}</span>
                      )}
                    </div>
                  </div>
                ))}
              </div>
            )}
          </div>
        );
      })}
    </ScrollableContainer>
  );
};

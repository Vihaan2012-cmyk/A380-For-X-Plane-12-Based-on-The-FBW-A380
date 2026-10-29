// Copyright (c) 2023-2024 FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0

import React, { useState } from 'react';
import { ScrollableContainer } from '../../UtilComponents/ScrollableContainer';
import { Catalogue, CatalogueComponent, byChapter, chapterNames } from '../catalogue';

interface ComponentsBrowserProps {
  catalogue: Catalogue;
  query: string;
}

/**
 * The parts the systems model is built from, and what each one can do wrong.
 *
 * A component here is a real unit -- a pump, a transformer rectifier, a
 * transducer -- with the parameters the model solves for it and the failures
 * registered against it. This is the page that answers "what is actually
 * modelled", which a flat failure list cannot.
 */
export const ComponentsBrowser = ({ catalogue, query }: ComponentsBrowserProps) => {
  const [selectedId, setSelectedId] = useState<string | null>(null);

  const matches = (component: CatalogueComponent): boolean => {
    if (query === '') {
      return true;
    }
    return component.name.toUpperCase().includes(query) || component.id.toUpperCase().includes(query);
  };

  const filtered = catalogue.components.filter(matches);
  const grouped = byChapter(filtered);
  const names = chapterNames(catalogue);

  const selected = filtered.find((component) => component.id === selectedId) ?? null;
  const failuresById = new Map(catalogue.failures.map((failure) => [failure.id, failure]));

  return (
    <div className="flex flex-row space-x-4">
      <div className="w-1/2 shrink-0">
        <ScrollableContainer height={46}>
          {filtered.length === 0 && <p className="mt-4 text-theme-unselected">Nothing matches that search.</p>}

          {[...grouped.entries()].map(([chapter, components]) => (
            <div key={chapter} className="mb-3">
              <h2 className="sticky top-0 bg-theme-body py-1 font-bold">{names.get(chapter) ?? `ATA ${chapter}`}</h2>

              <div className="space-y-1">
                {components.map((component) => (
                  <button
                    key={component.id}
                    type="button"
                    onClick={() => setSelectedId(component.id)}
                    className={`flex w-full flex-row items-baseline justify-between space-x-4 rounded-md border px-3 py-2 text-left transition duration-100 ${
                      selectedId === component.id
                        ? 'border-theme-highlight bg-theme-highlight text-theme-body'
                        : 'border-theme-accent hover:border-theme-highlight'
                    }`}
                  >
                    <span>
                      {component.name}
                      {component.instance !== null && <span className="opacity-75"> {component.instance}</span>}
                    </span>
                    <span className="shrink-0 text-sm opacity-75">{component.failures.length}</span>
                  </button>
                ))}
              </div>
            </div>
          ))}
        </ScrollableContainer>
      </div>

      <div className="grow rounded-md border border-theme-accent p-4">
        {selected === null ? (
          <p className="text-theme-unselected">Select a component.</p>
        ) : (
          <ScrollableContainer height={44}>
            <h2 className="font-bold">{selected.name}</h2>
            <p className="mt-1 font-mono text-sm text-theme-unselected">{selected.id}</p>

            {selected.parameters.length > 0 && (
              <>
                <h3 className="mt-4 font-bold">What the model solves for it</h3>
                <div className="mt-2 space-y-2">
                  {selected.parameters.map((parameter) => (
                    <div key={parameter.name} className="rounded-md border border-theme-accent px-3 py-2">
                      <div className="flex flex-row items-baseline justify-between space-x-4">
                        <span className="font-mono text-sm">{parameter.name}</span>
                        <span className="shrink-0 text-sm text-theme-unselected">healthy: {parameter.healthyValue}</span>
                      </div>
                      <p className="mt-1 text-sm text-theme-unselected">{parameter.meaning}</p>
                    </div>
                  ))}
                </div>
              </>
            )}

            <h3 className="mt-4 font-bold">What can go wrong with it</h3>
            <div className="mt-2 space-y-1">
              {selected.failures.map((id) => {
                const failure = failuresById.get(id);
                return (
                  <div key={id} className="rounded-md border border-theme-accent px-3 py-2">
                    <div className="flex flex-row items-baseline justify-between space-x-4">
                      <span>{failure?.name ?? `Failure ${id}`}</span>
                      <span className="shrink-0 font-mono text-sm text-theme-unselected">{id}</span>
                    </div>
                    {failure && <p className="mt-1 text-sm text-theme-unselected">{failure.cause}</p>}
                  </div>
                );
              })}
            </div>
          </ScrollableContainer>
        )}
      </div>
    </div>
  );
};

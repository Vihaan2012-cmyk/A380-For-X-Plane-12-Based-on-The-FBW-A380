// Copyright (c) 2023-2024 FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0

import React, { useState } from 'react';
import { ScrollableContainer } from '../../UtilComponents/ScrollableContainer';
import { Catalogue, CatalogueBreaker, humanisePanel, panelGrid } from '../catalogue';

interface BreakerPanelsProps {
  catalogue: Catalogue;
  query: string;
}

/**
 * The circuit breakers, drawn as the panels they actually sit on.
 *
 * Each breaker carries a panel, a row and a column, so this is a real
 * layout rather than a list -- the same geometry a walk-around would find.
 * Most of these are remote power-centre units rather than flight-deck
 * breakers, which is why the panel name matters.
 *
 * Whether a breaker is *open right now* is live state and does not come from
 * the catalogue. Until the systems model publishes it, every breaker here is
 * shown in its catalogued form only, and the page says so rather than
 * implying they are all closed.
 */
export const BreakerPanels = ({ catalogue, query }: BreakerPanelsProps) => {
  const [selected, setSelected] = useState<CatalogueBreaker | null>(null);

  const matches = (breaker: CatalogueBreaker): boolean => {
    if (query === '') {
      return true;
    }
    return (
      breaker.label.toUpperCase().includes(query) ||
      breaker.name.toUpperCase().includes(query) ||
      breaker.consumer.toUpperCase().includes(query) ||
      breaker.bus.toUpperCase().includes(query) ||
      humanisePanel(breaker.panel).toUpperCase().includes(query)
    );
  };

  const filtered = catalogue.breakers.filter(matches);

  const panels = new Map<string, CatalogueBreaker[]>();
  filtered.forEach((breaker) => {
    const existing = panels.get(breaker.panel);
    if (existing) {
      existing.push(breaker);
    } else {
      panels.set(breaker.panel, [breaker]);
    }
  });

  return (
    <div className="flex flex-row space-x-4">
      <div className="grow">
        <ScrollableContainer height={46}>
          {filtered.length === 0 && <p className="mt-4 text-theme-unselected">Nothing matches that search.</p>}

          {[...panels.entries()].sort(([a], [b]) => a.localeCompare(b)).map(([panel, breakers]) => (
            <div key={panel} className="mb-4">
              <div className="flex flex-row items-baseline justify-between">
                <h2 className="font-bold">{humanisePanel(panel)}</h2>
                <span className="text-sm text-theme-unselected">{breakers.length}</span>
              </div>

              <div className="mt-2 space-y-1 rounded-md border border-theme-accent p-2">
                {[...panelGrid(breakers).entries()].map(([row, inRow]) => (
                  <div key={row} className="flex flex-row flex-wrap gap-1">
                    {inRow.map((breaker) => (
                      <button
                        key={breaker.id}
                        type="button"
                        onClick={() => setSelected(breaker)}
                        title={`${breaker.consumer} — ${breaker.ratingAmperes} A on ${breaker.bus}`}
                        className={`w-28 shrink-0 rounded-sm border px-1 py-1 text-center text-xs transition duration-100 ${
                          selected?.id === breaker.id
                            ? 'border-theme-highlight bg-theme-highlight text-theme-body'
                            : 'border-theme-accent hover:border-theme-highlight'
                        } ${breaker.protectsModelledLoad ? '' : 'opacity-50'}`}
                      >
                        <div className="truncate font-mono">{breaker.label}</div>
                        <div className="opacity-75">{breaker.ratingAmperes} A</div>
                      </button>
                    ))}
                  </div>
                ))}
              </div>
            </div>
          ))}
        </ScrollableContainer>
      </div>

      <div className="w-96 shrink-0 rounded-md border border-theme-accent p-4">
        {selected === null ? (
          <p className="text-theme-unselected">Select a breaker.</p>
        ) : (
          <>
            <h2 className="font-bold">{selected.label}</h2>
            <p className="mt-1 text-sm text-theme-unselected">{selected.consumer}</p>

            <dl className="mt-4 space-y-2 text-sm">
              <Row label="Panel" value={humanisePanel(selected.panel)} />
              <Row label="Position" value={`Row ${selected.row}, column ${selected.column}`} />
              <Row label="Bus" value={selected.bus} />
              <Row label="Rating" value={`${selected.ratingAmperes} A`} />
              <Row label="Type" value={selected.kind.replace(/([A-Z])/g, ' $1').toLowerCase()} />
              <Row label="State" value="not published yet" muted />
            </dl>

            <p className="mt-4 border-t border-theme-accent pt-3 text-xs text-theme-unselected">{selected.basis}</p>

            {!selected.protectsModelledLoad && (
              <p className="mt-2 text-xs text-utility-amber">
                This breaker exists on the aircraft but protects nothing the model currently solves, so opening it
                would change nothing.
              </p>
            )}
          </>
        )}
      </div>
    </div>
  );
};

const Row = ({ label, value, muted }: { label: string; value: string; muted?: boolean }) => (
  <div className="flex flex-row justify-between space-x-4">
    <dt className="text-theme-unselected">{label}</dt>
    <dd className={`text-right ${muted ? 'italic text-theme-unselected' : ''}`}>{value}</dd>
  </div>
);

// Copyright (c) 2023-2024 FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0

import React, { useEffect, useState } from 'react';
import { Route } from 'react-router-dom';
import { ExclamationTriangleFill } from 'react-bootstrap-icons';
import { Navbar } from '../UtilComponents/Navbar';
import { SimpleInput } from '../UtilComponents/Form/SimpleInput/SimpleInput';
import { PageLink, PageRedirect } from '../Utils/routing';
import { Catalogue, loadCatalogue } from './catalogue';
import { FailuresBrowser } from './Pages/FailuresBrowser';
import { ComponentsBrowser } from './Pages/ComponentsBrowser';
import { BreakerPanels } from './Pages/BreakerPanels';
import { MelList } from './Pages/MelList';

/**
 * The Study pages: what the aircraft is made of, rather than what it is
 * doing. Every component the systems model knows about, every failure that
 * can be armed against it, every circuit breaker with the panel it really
 * sits on, and the MEL references that cover them.
 *
 * All of it comes from the static catalogue -- see `catalogue.ts` for why it
 * is fetched rather than bundled. Live state (armed failures, open breakers,
 * accumulated wear) is a separate and much smaller problem, and the pages
 * here show it as absent rather than guessing at it.
 */
export const Study = () => {
  const [catalogue, setCatalogue] = useState<Catalogue | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [searchQuery, setSearchQuery] = useState('');

  useEffect(() => {
    let cancelled = false;
    loadCatalogue()
      .then((loaded) => {
        if (!cancelled) {
          setCatalogue(loaded);
        }
      })
      .catch((loadError: Error) => {
        if (!cancelled) {
          setError(loadError.message);
        }
      });
    return () => {
      cancelled = true;
    };
  }, []);

  if (error !== null) {
    return (
      <>
        <h1 className="font-bold">Study</h1>
        <div className="mt-4 h-content-section-reduced rounded-lg border-2 border-theme-accent p-8">
          <div className="flex flex-row items-center space-x-4">
            <ExclamationTriangleFill size={30} className="text-utility-amber" />
            <div>
              <p className="font-bold">The systems catalogue could not be loaded.</p>
              <p className="mt-1 text-theme-unselected">{error}</p>
            </div>
          </div>
        </div>
      </>
    );
  }

  if (catalogue === null) {
    return (
      <>
        <h1 className="font-bold">Study</h1>
        <div className="mt-4 flex h-content-section-reduced items-center justify-center rounded-lg border-2 border-theme-accent">
          <p className="text-theme-unselected">Loading the systems catalogue…</p>
        </div>
      </>
    );
  }

  const query = searchQuery.trim().toUpperCase();

  const tabs: PageLink[] = [
    {
      name: 'Failures',
      alias: 'Failures',
      component: <FailuresBrowser catalogue={catalogue} query={query} />,
    },
    {
      name: 'Components',
      alias: 'Components',
      component: <ComponentsBrowser catalogue={catalogue} query={query} />,
    },
    {
      name: 'Breakers',
      alias: 'Breakers',
      component: <BreakerPanels catalogue={catalogue} query={query} />,
    },
    {
      name: 'MEL',
      alias: 'MEL',
      component: <MelList catalogue={catalogue} query={query} />,
    },
  ];

  return (
    <>
      <div className="flex flex-row items-center justify-between space-x-4">
        <h1 className="font-bold">Study</h1>
        <p className="text-theme-unselected">
          {catalogue.failures.length.toLocaleString()} failures · {catalogue.components.length.toLocaleString()}{' '}
          components · {catalogue.breakers.length.toLocaleString()} breakers
        </p>
      </div>

      <div className="mt-4 h-content-section-reduced space-y-4 rounded-lg border-2 border-theme-accent p-4">
        <div className="flex flex-row space-x-4">
          <SimpleInput
            placeholder="Search"
            className="grow uppercase"
            value={searchQuery}
            onChange={setSearchQuery}
          />
          <Navbar basePath="/study" tabs={tabs} />
        </div>

        <Route path="/study/failures">
          <FailuresBrowser catalogue={catalogue} query={query} />
        </Route>
        <Route path="/study/components">
          <ComponentsBrowser catalogue={catalogue} query={query} />
        </Route>
        <Route path="/study/breakers">
          <BreakerPanels catalogue={catalogue} query={query} />
        </Route>
        <Route path="/study/mel">
          <MelList catalogue={catalogue} query={query} />
        </Route>
      </div>

      <PageRedirect basePath="/study" tabs={tabs} />
    </>
  );
};

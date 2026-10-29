import React, { FC, useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { ipcRenderer } from 'electron';
import { Search, Trash, Download, ArrowRepeat } from 'react-bootstrap-icons';
import channels from 'common/channels';
import { Addon } from 'renderer/utils/InstallerConfiguration';
import { Directories } from 'renderer/utils/Directories';

interface Edition {
  file: string;
  folder: string;
  size: number;
  url: string;
  installed: boolean;
}

interface Livery {
  id: string;
  airline: string;
  name: string;
  registration: string;
  country: string;
  iata: string;
  model: string;
  engines: string;
  creator: string;
  preview_url?: string | null;
  editions: Record<string, Edition>;
}

interface LiveryList {
  liveries: Livery[];
  installedFolders: string[];
  error?: string | null;
}

interface EngineEvent {
  kind: 'status' | 'log' | 'progress' | 'done';
  text?: string;
  value?: number;
  ok?: boolean;
}

type Facet = 'airline' | 'country' | 'creator' | 'engines';
type InstallFilter = 'all' | 'installed' | 'available';
type Sort = 'airline' | 'registration' | 'country' | 'size';

const FACETS: { key: Facet; label: string }[] = [
  { key: 'airline', label: 'Airline' },
  { key: 'country', label: 'Country' },
  { key: 'creator', label: 'Creator' },
  { key: 'engines', label: 'Engines' },
];

const SORTS: { key: Sort; label: string }[] = [
  { key: 'airline', label: 'Airline' },
  { key: 'registration', label: 'Registration' },
  { key: 'country', label: 'Country' },
  { key: 'size', label: 'Download size' },
];

const EDITIONS = ['2K', '4K'];

let nextJob = 1_000_000;

const megabytes = (bytes: number) => `${Math.round(bytes / 1_048_576)} MB`;

const installedAny = (l: Livery) => Object.values(l.editions).some((e) => e.installed);

const Chip: FC<{ active: boolean; onClick: () => void }> = ({ active, onClick, children }) => (
  <button
    className={`rounded-full border-2 px-4 py-1 text-xl transition ${
      active
        ? 'border-cyan bg-cyan text-navy-dark'
        : 'border-navy-lightest bg-navy-light text-quasi-white hover:border-cyan'
    }`}
    onClick={onClick}
  >
    {children}
  </button>
);

const FacetMenu: FC<{
  label: string;
  values: string[];
  selected: Set<string>;
  onToggle: (value: string) => void;
}> = ({ label, values, selected, onToggle }) => {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const close = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) {
        setOpen(false);
      }
    };
    document.addEventListener('mousedown', close);
    return () => document.removeEventListener('mousedown', close);
  }, []);

  return (
    <div className="relative" ref={ref}>
      <Chip active={selected.size > 0} onClick={() => setOpen((o) => !o)}>
        {label}
        {selected.size > 0 ? ` (${selected.size})` : ''} ▾
      </Chip>
      {open && (
        <div className="absolute left-0 top-full z-20 mt-2 max-h-80 min-w-[14rem] overflow-y-auto rounded-md border-2 border-navy-lightest bg-navy-dark p-2 shadow-lg">
          {values.map((v) => (
            <label
              key={v}
              className="flex cursor-pointer items-center gap-x-3 rounded px-3 py-2 text-xl text-quasi-white hover:bg-navy-light"
            >
              <input type="checkbox" checked={selected.has(v)} onChange={() => onToggle(v)} />
              {v}
            </label>
          ))}
        </div>
      )}
    </div>
  );
};

export const Liveries: FC<{ addon: Addon }> = ({ addon }) => {
  const xplane = Directories.simulatorBasePath(addon.simulator);
  const [data, setData] = useState<LiveryList | null>(null);
  const [loading, setLoading] = useState(true);
  const [search, setSearch] = useState('');
  const [sort, setSort] = useState<Sort>('airline');
  const [installFilter, setInstallFilter] = useState<InstallFilter>('all');
  const [facets, setFacets] = useState<Record<Facet, Set<string>>>({
    airline: new Set(),
    country: new Set(),
    creator: new Set(),
    engines: new Set(),
  });
  const [jobs, setJobs] = useState<Record<string, { text: string; progress: number }>>({});
  const [message, setMessage] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      const list = (await ipcRenderer.invoke(channels.xplaneEngine.query, [
        'liveries',
        ...(xplane ? ['--xplane', xplane] : []),
      ])) as LiveryList;
      setData(list);
    } catch (e) {
      setData({ liveries: [], installedFolders: [], error: String(e) });
    }
    setLoading(false);
  }, [xplane]);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const run = useCallback(
    async (key: string, args: string[], label: string) => {
      const id = nextJob++;
      setJobs((j) => ({ ...j, [key]: { text: label, progress: 0 } }));
      const onEvent = (_: unknown, jobId: number, event: EngineEvent) => {
        if (jobId !== id) {
          return;
        }
        if (event.kind === 'status' && event.text) {
          const text = event.text.replace(/\.\.\.$/, '');
          setJobs((j) => ({ ...j, [key]: { ...j[key], text } }));
        } else if (event.kind === 'progress') {
          setJobs((j) => ({ ...j, [key]: { ...j[key], progress: event.value ?? 0 } }));
        }
      };
      ipcRenderer.on(channels.xplaneEngine.event, onEvent);
      const result = (await ipcRenderer.invoke(channels.xplaneEngine.install, id, args)) as EngineEvent;
      ipcRenderer.removeListener(channels.xplaneEngine.event, onEvent);
      setJobs((j) => {
        const rest = { ...j };
        delete rest[key];
        return rest;
      });
      setMessage(result.ok ? (result.text ?? null) : `Failed: ${result.text}`);
      await refresh();
    },
    [refresh],
  );

  const install = (l: Livery, edition: string) =>
    xplane &&
    run(
      `${l.id}/${edition}`,
      ['livery-install', '--xplane', xplane, '--id', l.id, '--edition', edition],
      `Downloading ${edition}`,
    );

  const remove = (l: Livery, edition: string) =>
    xplane &&
    run(
      `${l.id}/${edition}`,
      ['livery-remove', '--xplane', xplane, '--folder', l.editions[edition].folder],
      `Removing ${edition}`,
    );

  const values = useMemo(() => {
    const out = {} as Record<Facet, string[]>;
    for (const { key } of FACETS) {
      out[key] = [...new Set((data?.liveries ?? []).map((l) => l[key]).filter(Boolean))].sort();
    }
    return out;
  }, [data]);

  const shown = useMemo(() => {
    const q = search.trim().toLowerCase();
    const list = (data?.liveries ?? []).filter((l) => {
      if (installFilter === 'installed' && !installedAny(l)) {
        return false;
      }
      if (installFilter === 'available' && installedAny(l)) {
        return false;
      }
      for (const { key } of FACETS) {
        if (facets[key].size > 0 && !facets[key].has(l[key])) {
          return false;
        }
      }
      if (!q) {
        return true;
      }
      return [l.airline, l.name, l.registration, l.country, l.iata, l.model, l.engines, l.creator]
        .join(' ')
        .toLowerCase()
        .includes(q);
    });
    const size = (l: Livery) => Math.max(...Object.values(l.editions).map((e) => e.size));
    return list.sort((a, b) => {
      switch (sort) {
        case 'registration':
          return (a.registration || a.name).localeCompare(b.registration || b.name);
        case 'country':
          return a.country.localeCompare(b.country) || a.airline.localeCompare(b.airline);
        case 'size':
          return size(b) - size(a);
        default:
          return (
            a.airline.localeCompare(b.airline) || (a.registration || a.name).localeCompare(b.registration || b.name)
          );
      }
    });
  }, [data, search, sort, installFilter, facets]);

  const toggleFacet = (key: Facet, value: string) =>
    setFacets((f) => {
      const next = new Set(f[key]);
      if (next.has(value)) {
        next.delete(value);
      } else {
        next.add(value);
      }
      return { ...f, [key]: next };
    });

  const clear = () => {
    setFacets({ airline: new Set(), country: new Set(), creator: new Set(), engines: new Set() });
    setInstallFilter('all');
    setSearch('');
  };

  const installedCount = (data?.liveries ?? []).filter(installedAny).length;

  return (
    <div className="flex size-full flex-col overflow-hidden">
      <div className="flex shrink-0 flex-col gap-y-4 px-7 pb-4 pt-6">
        <div className="flex items-end justify-between">
          <div>
            <h2 className="mb-0 font-bold text-white">Liveries</h2>
            <span className="text-xl text-quasi-white">
              {data ? `${data.liveries.length} liveries, ${installedCount} installed` : 'Loading the livery list...'}
            </span>
          </div>
          <button
            className="flex items-center gap-x-2 rounded-md bg-navy-light px-4 py-2 text-xl text-quasi-white hover:bg-navy-lightest"
            onClick={refresh}
          >
            <ArrowRepeat size={18} className={loading ? 'animate-spin' : ''} /> Refresh
          </button>
        </div>

        <div className="flex flex-wrap items-center gap-3">
          <Chip active onClick={() => undefined}>
            Aircraft is A380
          </Chip>
          {FACETS.map(({ key, label }) => (
            <FacetMenu
              key={key}
              label={label}
              values={values[key]}
              selected={facets[key]}
              onToggle={(v) => toggleFacet(key, v)}
            />
          ))}
          <Chip
            active={installFilter === 'installed'}
            onClick={() => setInstallFilter((f) => (f === 'installed' ? 'all' : 'installed'))}
          >
            Installed
          </Chip>
          <Chip
            active={installFilter === 'available'}
            onClick={() => setInstallFilter((f) => (f === 'available' ? 'all' : 'available'))}
          >
            Not installed
          </Chip>
          <button className="text-xl text-cyan hover:underline" onClick={clear}>
            Clear
          </button>
        </div>

        <div className="flex items-center gap-x-4">
          <div className="flex grow items-center gap-x-3 rounded-md border-2 border-navy-lightest bg-navy-dark px-4 py-2">
            <Search size={18} className="text-quasi-white" />
            <input
              className="w-full bg-transparent text-xl text-white outline-none placeholder:text-utility-grey"
              placeholder="Search airline, registration, country, engine or creator"
              value={search}
              onChange={(e) => setSearch(e.target.value)}
            />
          </div>
          <select
            className="rounded-md border-2 border-navy-lightest bg-navy-dark px-4 py-2 text-xl text-white"
            value={sort}
            onChange={(e) => setSort(e.target.value as Sort)}
          >
            {SORTS.map((s) => (
              <option key={s.key} value={s.key}>
                Sort: {s.label}
              </option>
            ))}
          </select>
        </div>

        {message && (
          <div className="flex items-center justify-between rounded-md bg-navy-light px-4 py-2 text-xl text-quasi-white">
            {message}
            <button className="text-cyan" onClick={() => setMessage(null)}>
              Dismiss
            </button>
          </div>
        )}
        {data?.error && (
          <div className="rounded-md border-2 border-utility-amber px-4 py-2 text-xl text-utility-amber">
            {data.error.includes('no release on GitHub has liveries')
              ? 'No liveries are published yet. They come with the next release.'
              : `Could not read the livery list: ${data.error}`}
          </div>
        )}
        {!xplane && (
          <div className="rounded-md border-2 border-utility-amber px-4 py-2 text-xl text-utility-amber">
            Set your X-Plane 12 folder in Settings to install liveries.
          </div>
        )}
      </div>

      <div className="grow overflow-y-auto px-7 pb-7">
        <div className="grid grid-cols-[repeat(auto-fill,minmax(19rem,1fr))] gap-5">
          {shown.map((l) => (
            <div
              key={l.id}
              className="flex flex-col overflow-hidden rounded-lg border-2 border-navy-lightest bg-navy-light"
            >
              <div className="relative aspect-video bg-navy-dark">
                {l.preview_url && (
                  <img
                    className="size-full object-cover"
                    src={l.preview_url}
                    alt={`${l.airline} ${l.name}`}
                    loading="lazy"
                  />
                )}
                {installedAny(l) && (
                  <span className="absolute right-2 top-2 rounded-full bg-utility-green px-3 py-0.5 text-lg font-semibold text-navy-dark">
                    Installed
                  </span>
                )}
              </div>
              <div className="flex grow flex-col gap-y-2 p-4">
                <span className="text-lg uppercase tracking-wider text-cyan">{l.airline}</span>
                <span className="text-3xl font-semibold text-white">{l.registration || l.name}</span>
                <span className="text-xl text-quasi-white">
                  {[l.model, l.registration && l.name !== l.registration ? l.name : null].filter(Boolean).join(', ')}
                </span>
                <div className="flex flex-wrap gap-2 pt-1">
                  {[l.country, l.engines, l.creator && `by ${l.creator}`].filter(Boolean).map((t) => (
                    <span key={t} className="rounded bg-navy-lightest px-2 py-0.5 text-lg text-quasi-white">
                      {t}
                    </span>
                  ))}
                </div>
                <div className="mt-auto flex flex-col gap-y-2 pt-3">
                  {EDITIONS.filter((ed) => l.editions[ed]).map((ed) => {
                    const e = l.editions[ed];
                    const job = jobs[`${l.id}/${ed}`];
                    if (job) {
                      return (
                        <div key={ed} className="flex flex-col gap-y-1">
                          <span className="text-lg text-quasi-white">
                            {job.text} {job.progress > 0 ? `${Math.round(job.progress * 100)}%` : ''}
                          </span>
                          <div className="h-2 overflow-hidden rounded bg-navy-dark">
                            <div
                              className="h-full bg-cyan transition-all"
                              style={{ width: `${Math.round(job.progress * 100)}%` }}
                            />
                          </div>
                        </div>
                      );
                    }
                    return e.installed ? (
                      <button
                        key={ed}
                        className="flex items-center justify-between rounded-md border-2 border-utility-red px-4 py-2 text-xl text-utility-red hover:bg-utility-red hover:text-white"
                        onClick={() => remove(l, ed)}
                        disabled={!xplane}
                      >
                        <span>Remove {ed}</span>
                        <Trash size={18} />
                      </button>
                    ) : (
                      <button
                        key={ed}
                        className="flex items-center justify-between rounded-md bg-cyan px-4 py-2 text-xl font-semibold text-navy-dark hover:bg-cyan-medium disabled:opacity-50"
                        onClick={() => install(l, ed)}
                        disabled={!xplane}
                      >
                        <span>
                          Install {ed} <span className="font-normal">({megabytes(e.size)})</span>
                        </span>
                        <Download size={18} />
                      </button>
                    );
                  })}
                </div>
              </div>
            </div>
          ))}
        </div>
        {data && !loading && shown.length === 0 && !data.error && (
          <p className="mt-10 text-center text-2xl text-quasi-white">No livery matches these filters.</p>
        )}
      </div>
    </div>
  );
};

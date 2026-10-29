/** @jsx UI.node */
/** @jsxFrag UI.Group */
//  Copyright (c) 2026 FlyByWire Simulations
//  SPDX-License-Identifier: GPL-3.0

import { Part as View, Parts as UI, State as Signal, PartNode as ViewNode } from 'xplane-js/ui';
import { AbstractOitFltOpsPageProps } from '../../OIT';
import { ChartSource, OisChart, listCharts, pluginUrl } from '../../System/PluginLink';
import { fetchOfp, lastOfp } from '../../System/SimbriefOfp';
import { oefbElement } from './OitFltOpsFltFolder';

type Tab = 'ENROUTE' | 'TERMINAL';

interface Sheet {
  title: string;
  url: string;
  nightUrl?: string;
  pdf: boolean;
}

const SOURCES: [ChartSource, string][] = [
  ['navigraph', 'NAVIGRAPH'],
  ['faa', 'FAA'],
  ['local', 'MY CHARTS'],
];

const NIGHT_FILTER = 'invert(0.92) hue-rotate(180deg)';

function stored(key: string, fallback: string): string {
  try {
    return localStorage.getItem(key) || fallback;
  } catch {
    return fallback;
  }
}

function store(key: string, value: string): void {
  try {
    localStorage.setItem(key, value);
  } catch {
    /* empty */
  }
}

export class OitFltOpsCharts extends View<AbstractOitFltOpsPageProps> {
  private readonly listRef = UI.ref<HTMLDivElement>();

  private readonly stageRef = UI.ref<HTMLDivElement>();

  private readonly tabsRef = UI.ref<HTMLDivElement>();

  private readonly sourceRef = UI.ref<HTMLDivElement>();

  private readonly airportsRef = UI.ref<HTMLDivElement>();

  private readonly icaoRef = UI.ref<HTMLInputElement>();

  private readonly status = Signal.of('');

  private readonly nightLabel = Signal.of('NIGHT');

  private tab: Tab = 'TERMINAL';

  private source = stored('oit.charts.source', 'navigraph') as ChartSource;

  private night = stored('oit.charts.night', '0') === '1';

  private sheet: Sheet | null = null;

  private sheetEl: HTMLElement | null = null;

  private selected: HTMLElement | null = null;

  private zoom = 1;

  private rotation = 0;

  private panX = 0;

  private panY = 0;

  private drag: { x: number; y: number; px: number; py: number } | null = null;

  private segment(host: HTMLElement | undefined, items: [string, string][], current: string, pick: (v: string) => void): void {
    if (!host) {
      return;
    }
    host.replaceChildren();
    for (const [value, label] of items) {
      const b = oefbElement('div', value === current ? 'on' : '', label);
      b.addEventListener('click', () => pick(value));
      host.append(b);
    }
  }

  private renderControls(): void {
    this.segment(this.tabsRef.peek(), [['TERMINAL', 'TERMINAL'], ['ENROUTE', 'ENROUTE']], this.tab, (v) => {
      this.tab = v as Tab;
      this.renderControls();
      this.load();
    });
    const source = this.sourceRef.peek();
    this.segment(source, SOURCES, this.source, (v) => {
      this.source = v as ChartSource;
      store('oit.charts.source', v);
      this.renderControls();
      this.load();
    });
    if (source) {
      source.style.display = this.tab === 'TERMINAL' ? '' : 'none';
    }
    const airports = this.airportsRef.peek();
    if (airports) {
      airports.replaceChildren();
      const ofp = lastOfp();
      const pairs: [string, string][] = ofp
        ? ([
            ['DEP', ofp.origin],
            ['ARR', ofp.destination],
            ['ALTN', ofp.alternate],
          ] as [string, string][]).filter(([, icao]) => icao)
        : [];
      for (const [label, icao] of pairs) {
        const chip = oefbElement('div', 'oefb-chip');
        chip.append(oefbElement('span', 'k', label), oefbElement('span', '', icao));
        chip.addEventListener('click', () => {
          this.setIcao(icao);
          this.load();
        });
        airports.append(chip);
      }
      airports.style.display = this.tab === 'TERMINAL' ? '' : 'none';
    }
    const input = this.icaoRef.peek();
    if (input) {
      input.style.display = this.tab === 'TERMINAL' ? '' : 'none';
    }
  }

  private icao(): string {
    return (this.icaoRef.peek()?.value ?? '').trim().toUpperCase();
  }

  private setIcao(icao: string): void {
    const input = this.icaoRef.peek();
    if (input) {
      input.value = icao;
    }
  }

  private stageMessage(text: string, warn = false): void {
    this.sheet = null;
    this.sheetEl = null;
    this.stageRef.peek()?.replaceChildren(oefbElement('div', warn ? 'oefb-msg warn' : 'oefb-msg', text));
  }

  private listMessage(text: string): void {
    this.listRef.peek()?.replaceChildren(oefbElement('div', 'oefb-msg small', text));
  }

  private renderList(groups: Map<string, Sheet[]>): void {
    const list = this.listRef.peek();
    if (!list) {
      return;
    }
    list.replaceChildren();
    let first: [Sheet, HTMLElement] | null = null;
    for (const [group, sheets] of groups) {
      list.append(oefbElement('div', 'oefb-group', group));
      for (const s of sheets) {
        const item = oefbElement('div', 'oefb-item', s.title);
        item.addEventListener('click', () => this.open(s, item));
        list.append(item);
        if (!first) {
          first = [s, item];
        }
      }
    }
    if (first) {
      this.open(first[0], first[1]);
    }
  }

  private async loadEnroute(): Promise<void> {
    this.status.set('ROUTE MAPS FROM THE SIMBRIEF OFP');
    let ofp = lastOfp();
    if (!ofp) {
      this.listMessage('FETCHING THE OFP...');
      try {
        ofp = await fetchOfp();
        this.renderControls();
      } catch (e) {
        this.listMessage('');
        this.stageMessage(e instanceof Error ? e.message : String(e), true);
        return;
      }
    }
    const maps = ofp.sections.filter((s) => s.image).map((s) => ({ title: s.title, url: s.image as string, pdf: false }));
    if (maps.length === 0) {
      this.listMessage('');
      this.stageMessage('THE OFP HAS NO ROUTE MAPS');
      return;
    }
    this.renderList(new Map([['ROUTE MAPS', maps]]));
  }

  private async loadTerminal(): Promise<void> {
    const icao = this.icao();
    if (icao.length !== 4) {
      this.status.set('');
      this.listMessage('');
      this.stageMessage('ENTER A FOUR-LETTER ICAO CODE');
      return;
    }
    store('oit.charts.icao', icao);
    const label = SOURCES.find(([s]) => s === this.source)?.[1] ?? '';
    this.status.set(`${label} - ${icao}`);
    this.listMessage('LOADING...');
    this.stageMessage(this.source === 'faa' ? 'LOADING THE FAA CHART INDEX...' : 'LOADING...');
    const { charts, error } = await listCharts(this.source, icao);
    if (error || charts.length === 0) {
      this.listMessage('');
      this.stageMessage((error ?? `NO CHARTS FOR ${icao}`).toUpperCase(), true);
      return;
    }
    const groups = new Map<string, Sheet[]>();
    for (const c of charts as OisChart[]) {
      const key = (c.category || c.type_code || 'OTHER').toUpperCase();
      const url = c.url || c.image_day_url || '';
      if (!url) {
        continue;
      }
      if (!groups.has(key)) {
        groups.set(key, []);
      }
      groups.get(key)?.push({
        title: (c.name || c.id).toUpperCase(),
        url: pluginUrl(url),
        nightUrl: c.image_night_url ? pluginUrl(c.image_night_url) : undefined,
        pdf: c.kind === 'pdf' || /\.pdf($|\?)/i.test(url),
      });
    }
    this.status.set(`${label} - ${icao} - ${charts.length} CHARTS`);
    this.renderList(groups);
  }

  private load(): void {
    this.selected = null;
    if (this.tab === 'ENROUTE') {
      this.loadEnroute();
    } else {
      this.loadTerminal();
    }
  }

  private open(s: Sheet, item: HTMLElement): void {
    const stage = this.stageRef.peek();
    if (!stage) {
      return;
    }
    this.selected?.classList.remove('sel');
    item.classList.add('sel');
    this.selected = item;
    this.sheet = s;
    this.rotation = 0;
    this.panX = 0;
    this.panY = 0;
    this.zoom = 1;
    if (s.pdf) {
      const frame = oefbElement('iframe', 'oefb-pdf') as HTMLIFrameElement;
      frame.src = s.url;
      this.sheetEl = frame;
      stage.replaceChildren(frame);
      this.place();
      return;
    }
    const img = oefbElement('img', 'oefb-sheet') as HTMLImageElement;
    img.alt = s.title;
    img.addEventListener('load', () => this.fit());
    img.addEventListener('error', () => this.stageMessage('THE CHART COULD NOT BE LOADED', true));
    img.src = this.night && s.nightUrl ? s.nightUrl : s.url;
    this.sheetEl = img;
    stage.replaceChildren(img);
    this.place();
  }

  private place(): void {
    const el = this.sheetEl;
    if (!el || !this.sheet) {
      return;
    }
    el.style.transform = `translate(-50%, -50%) translate(${this.panX}px, ${this.panY}px) scale(${this.zoom}) rotate(${this.rotation}deg)`;
    el.style.filter = this.night && !this.sheet.nightUrl ? NIGHT_FILTER : 'none';
  }

  private fit(): void {
    const stage = this.stageRef.peek();
    const el = this.sheetEl;
    if (!stage || !el || !this.sheet) {
      return;
    }
    this.panX = 0;
    this.panY = 0;
    if (this.sheet.pdf) {
      this.zoom = 1;
    } else {
      const img = el as HTMLImageElement;
      const turned = this.rotation % 180 !== 0;
      const w = turned ? img.naturalHeight : img.naturalWidth;
      const h = turned ? img.naturalWidth : img.naturalHeight;
      this.zoom = w > 0 && h > 0 ? Math.min((stage.clientWidth - 20) / w, (stage.clientHeight - 20) / h) : 1;
    }
    this.place();
  }

  private zoomBy(f: number): void {
    if (!this.sheet || this.sheet.pdf) {
      return;
    }
    this.zoom = Math.min(8, Math.max(0.05, this.zoom * f));
    this.panX *= f;
    this.panY *= f;
    this.place();
  }

  private rotate(): void {
    this.rotation = (this.rotation + 90) % 360;
    this.fit();
  }

  private toggleNight(): void {
    this.night = !this.night;
    store('oit.charts.night', this.night ? '1' : '0');
    this.nightLabel.set(this.night ? 'DAY' : 'NIGHT');
    if (this.sheet && this.sheet.nightUrl && this.sheetEl && !this.sheet.pdf) {
      (this.sheetEl as HTMLImageElement).src = this.night ? this.sheet.nightUrl : this.sheet.url;
    }
    this.place();
  }

  public afterMount(node: ViewNode): void {
    super.afterMount(node);
    this.nightLabel.set(this.night ? 'DAY' : 'NIGHT');
    const ofp = lastOfp();
    this.setIcao(stored('oit.charts.icao', ofp ? ofp.origin : ''));
    const input = this.icaoRef.peek();
    input?.addEventListener('keydown', (e) => {
      if (e.key === 'Enter') {
        this.load();
      }
    });
    const stage = this.stageRef.peek();
    if (stage) {
      stage.addEventListener('pointerdown', (e) => {
        if (!this.sheet || this.sheet.pdf) {
          return;
        }
        this.drag = { x: e.clientX, y: e.clientY, px: this.panX, py: this.panY };
        stage.setPointerCapture?.(e.pointerId);
      });
      stage.addEventListener('pointermove', (e) => {
        if (this.drag) {
          this.panX = this.drag.px + e.clientX - this.drag.x;
          this.panY = this.drag.py + e.clientY - this.drag.y;
          this.place();
        }
      });
      const end = () => {
        this.drag = null;
      };
      stage.addEventListener('pointerup', end);
      stage.addEventListener('pointercancel', end);
      stage.addEventListener('wheel', (e) => {
        e.preventDefault();
        this.zoomBy(e.deltaY < 0 ? 1.15 : 1 / 1.15);
      });
    }
    this.renderControls();
    this.load();
  }

  render(): ViewNode {
    return (
      <div class="oit-page-container oefb">
        <div class="oefb-bar">
          <div class="oefb-title">CHARTS</div>
          <div ref={this.tabsRef} class="oefb-seg" />
          <div class="oefb-sub">{this.status}</div>
          <div class="oefb-spacer" />
          <div ref={this.airportsRef} class="oefb-chips" />
          <input ref={this.icaoRef} class="oefb-input" maxlength={4} placeholder="ICAO" />
          <div class="oefb-btn primary" onClick={() => this.load()}>
            LOAD
          </div>
        </div>
        <div class="oefb-body">
          <div class="oefb-side">
            <div ref={this.sourceRef} class="oefb-seg full" />
            <div ref={this.listRef} class="oefb-tree" />
          </div>
          <div class="oefb-viewer">
            <div ref={this.stageRef} class="oefb-stage" />
            <div class="oefb-tools">
              <div class="oefb-btn" onClick={() => this.zoomBy(1 / 1.25)}>
                -
              </div>
              <div class="oefb-btn" onClick={() => this.zoomBy(1.25)}>
                +
              </div>
              <div class="oefb-btn" onClick={() => this.fit()}>
                FIT
              </div>
              <div class="oefb-btn" onClick={() => this.rotate()}>
                ROTATE
              </div>
              <div class="oefb-btn" onClick={() => this.toggleNight()}>
                {this.nightLabel}
              </div>
            </div>
          </div>
        </div>
      </div>
    );
  }
}

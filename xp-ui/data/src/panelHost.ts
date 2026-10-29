import type { Context } from './host.ts';
import { clockNow, describeError, errorMessage, frameTime, requestFrame } from './host.ts';
import { SessionPhase, SoundId, sessionPhase } from './types.ts';
import type { GeoPos3, GeoPosData, PanelTemplate } from './types.ts';
import { Text } from './text.ts';

interface InstrumentState {
  guid: string;
  started: boolean;
  attached: boolean;
  templated: boolean;
  failed: boolean;
  flown: boolean;
  frameDelta: number;
  previousTime: number;
}

interface InstrumentHooks {
  onPrepare(): void;
  onFrame(): void;
  onPanelEvent(args: string[]): void;
  onSessionChange(from: SessionPhase, to: SessionPhase): void;
  onFlightBegin(): void;
  readPanelConfig(): void;
}

class FallbackElement {
  appendChild<T>(node: T): T {
    return node;
  }
}

function baseElement(): typeof HTMLElement {
  const g = globalThis as { HTMLElement?: typeof HTMLElement };
  return (g.HTMLElement ? g.HTMLElement : FallbackElement) as typeof HTMLElement;
}

function pageDocument(): Document | undefined {
  const g = globalThis as { document?: Document };
  return g.document;
}

export function whenReady(fn: () => void): void {
  const doc = pageDocument();
  if (!doc) {
    return;
  }
  if (doc.readyState !== 'loading') {
    fn();
  } else {
    doc.addEventListener('DOMContentLoaded', () => fn(), { once: true });
  }
}

export function createInstrumentHost(ctx: Context, signal: (name: string, ...args: any[]) => void) {
  const Base = baseElement();
  const states = new WeakMap<object, InstrumentState>();
  const tags = new WeakMap<object, string>();
  const attached: PanelInstrument[] = [];

  function stateOf(el: object): InstrumentState {
    let st = states.get(el);
    if (!st) {
      st = { guid: Text.newGuid(), started: false, attached: false, templated: false, failed: false, flown: false, frameDelta: 0, previousTime: 0 };
      states.set(el, st);
    }
    return st;
  }

  function tagOf(el: PanelInstrument): string {
    const local = (el as { localName?: string }).localName;
    if (local) {
      return local;
    }
    const byCtor = tags.get(el.constructor);
    if (byCtor) {
      return byCtor;
    }
    const tagName = (el as { tagName?: string }).tagName;
    return tagName ? tagName.toLowerCase() : '';
  }

  function hooks(el: PanelInstrument): InstrumentHooks {
    return el as unknown as InstrumentHooks;
  }

  class PanelInstrument extends Base implements PanelTemplate {
    panelConfig!: Document;

    get templateId(): string {
      return '';
    }

    get interactive(): boolean {
      return true;
    }

    get screenIndex(): number {
      const g = globalThis as { location?: { search?: string } };
      const search = g.location && typeof g.location.search === 'string' ? g.location.search : '';
      const params = new URLSearchParams(search);
      let result = 1;
      let found = false;
      params.forEach((value, key) => {
        if (!found && key.toLowerCase() === 'index') {
          found = true;
          const n = parseInt(value, 10);
          if (Number.isFinite(n)) {
            result = n;
          }
        }
      });
      return result;
    }

    get frameDelta(): number {
      return stateOf(this).frameDelta;
    }

    connectedCallback(): void {
      const st = stateOf(this);
      if (st.started) {
        return;
      }
      st.started = true;
      const chain = (time: number): void => {
        if (st.failed) {
          return;
        }
        requestFrame(chain);
        if (st.attached) {
          instrumentFrame(this, st, frameTime(time));
        }
      };
      st.previousTime = clockNow();
      requestFrame(chain);
      try {
        this.onAttach();
      } catch (e) {
        st.failed = true;
        const h = ctx.host();
        if (h) {
          h.log(2, describeError(e));
          h.loaded(false, errorMessage(e));
        }
        return;
      }
      st.attached = true;
      attached.push(this);
      const h = ctx.host();
      if (h) {
        h.loaded(true, '');
      }
      signal('instrument.ready', st.guid, tagOf(this));
    }

    onAttach(): void {
      const st = stateOf(this);
      if (!st.templated) {
        st.templated = true;
        const id = this.templateId;
        const doc = pageDocument();
        if (id && doc) {
          const tpl = doc.getElementById(id) as HTMLTemplateElement | null;
          if (tpl && tpl.content) {
            this.appendChild(tpl.content.cloneNode(true));
          }
        }
      }
      this.readPanelConfig();
      this.onPrepare();
    }

    protected onPrepare(): void {}

    protected onFrame(): void {}

    onPanelEvent(_args: Array<string>): void {}

    protected onSessionChange(_from: SessionPhase, _to: SessionPhase): void {}

    protected onFlightBegin(): void {}

    onSoundDone(_id: SoundId): void {}

    onPowered(): void {}

    onShutdown(): void {}

    sessionState(): SessionPhase {
      return sessionPhase();
    }

    protected readPanelConfig(): void {
      const h = ctx.host();
      const text = h ? h.readFile('/VFS/panel/panel.xml') : null;
      const xml = text === null || text === undefined ? '<PanelInfo/>' : text;
      this.panelConfig = new DOMParser().parseFromString(xml, 'text/xml');
    }
  }

  class MapElement extends Base {
    configId: number = 0;
    configs: any[] = [];
    setMapId(_id: any): void {}
    setConfig(_id: number): void {}
    setParams(_params: { lla: GeoPos3 | GeoPosData; radius: number }): void {}
    setVisible(_visible: boolean): void {}
  }

  function defineInstrument(tag: string, type: CustomElementConstructor): void {
    tags.set(type, tag);
    const g = globalThis as { customElements?: CustomElementRegistry };
    const registry = g.customElements;
    if (registry) {
      try {
        if (!registry.get(tag)) {
          registry.define(tag, type);
        }
      } catch {}
    }
    whenReady(() => {
      const doc = pageDocument();
      if (doc && doc.body) {
        // Each page holds one panel element with the instrument in it, its url the page's own, as FlyByWire's display index lookups expect.
        const panel = doc.createElement('vcockpit-panel');
        panel.style.cssText = 'display:block;position:absolute;left:0;top:0;width:100%;height:100%';
        const el = doc.createElement(tag);
        el.setAttribute('url', String((globalThis as { location?: { href?: string } }).location?.href ?? ''));
        panel.appendChild(el);
        doc.body.appendChild(panel);
      }
    });
  }

  function panelEvent(name: string): void {
    for (const el of attached.slice()) {
      try {
        hooks(el).onPanelEvent([name]);
      } catch (e) {
        ctx.log(2, describeError(e));
      }
    }
  }

  function instrumentFrame(el: PanelInstrument, st: InstrumentState, time: number): void {
    st.frameDelta = time - st.previousTime;
    st.previousTime = time;
    try {
      if (!st.flown) {
        st.flown = true;
        hooks(el).onSessionChange(SessionPhase.briefing, SessionPhase.flying);
        hooks(el).onFlightBegin();
      }
      hooks(el).onFrame();
    } catch (e) {
      ctx.log(2, describeError(e));
    }
  }

  return { PanelInstrument, MapElement, defineInstrument, panelEvent };
}

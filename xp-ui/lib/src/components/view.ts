import type { Lifespan, Readable, ReadableList, Bond } from 'xplane-js/ui';

export interface ViewNode {
  target: any;
}

export type ViewChildren = ViewNode | string | number | Readable<any> | ReadableList<any> | null | undefined;

export interface ViewProps {
  children?: ViewChildren[];
  ref?: Ref<any>;
}

export class Ref<T> {
  private held: T | undefined = undefined;

  get target(): T {
    if (this.held === undefined) {
      throw new Error('Ref was read before a node was bound to it');
    }
    return this.held;
  }

  set target(value: T) {
    this.held = value;
  }

  peek(): T | null {
    return this.held === undefined ? null : this.held;
  }
}

type RecKind = 'el' | 'text' | 'raw' | 'group' | 'comp';

class NodeRec implements ViewNode {
  kids: NodeRec[] = [];
  out: NodeRec | null = null;
  dom: Node | null = null;
  subs: Bond[] = [];
  trail: Node[] = [];
  placed = false;

  constructor(
    readonly kind: RecKind,
    public target: any,
  ) {}
}

const SVG_NS = 'http://www.w3.org/2000/svg';
const XLINK_NS = 'http://www.w3.org/1999/xlink';
const XML_NS = 'http://www.w3.org/XML/1998/namespace';

const SVG_TAGS = new Set([
  'svg',
  'g',
  'path',
  'line',
  'text',
  'rect',
  'circle',
  'ellipse',
  'defs',
  'linearGradient',
  'radialGradient',
  'stop',
  'tspan',
  'textPath',
  'use',
  'clipPath',
  'mask',
  'pattern',
  'marker',
  'symbol',
  'polygon',
  'polyline',
  'image',
  'filter',
  'foreignObject',
  'feBlend',
  'feColorMatrix',
  'feComposite',
  'feFlood',
  'feGaussianBlur',
  'feMerge',
  'feMergeNode',
  'feOffset',
]);

const FLAG_ATTRS = new Set([
  'disabled',
  'hidden',
  'checked',
  'readonly',
  'readOnly',
  'required',
  'selected',
  'multiple',
  'autofocus',
  'autoplay',
  'controls',
  'loop',
  'muted',
]);

const compRecs = new WeakMap<object, NodeRec>();

function followable(v: any): boolean {
  return v !== null && typeof v === 'object' && typeof v.watch === 'function' && typeof v.get === 'function';
}

function shownText(v: unknown): string {
  return v === null || v === undefined ? '' : String(v);
}

function boundTextOf(v: unknown): string {
  return v === null || v === undefined || v === '' ? ' ' : String(v);
}

function plainText(s: string): NodeRec {
  const rec = new NodeRec('text', null);
  rec.dom = document.createTextNode(s);
  rec.target = rec.dom;
  return rec;
}

const MARKUP = /<[a-zA-Z\/!]|&[#a-zA-Z0-9]+;/;

function parseMarkup(s: string): Node[] {
  const holder = document.createElement('template');
  holder.innerHTML = s;
  return Array.from(holder.content.childNodes);
}

function boundText(src: Readable<any>): NodeRec {
  const rec = plainText('');
  const text = rec.dom as Text;
  rec.subs.push(
    src.watch((v) => {
      for (const n of rec.trail) {
        n.parentNode?.removeChild(n);
      }
      rec.trail = [];
      const s = boundTextOf(v);
      if (typeof v === 'string' && MARKUP.test(s)) {
        text.data = '';
        rec.trail = parseMarkup(s);
        const host = text.parentNode;
        if (host) {
          let after: Node = text;
          for (const n of rec.trail) {
            host.insertBefore(n, after.nextSibling);
            after = n;
          }
        }
      } else {
        text.data = s;
      }
    }, true),
  );
  return rec;
}

function emptyGroup(): NodeRec {
  return new NodeRec('group', null);
}

function gatherRecs(input: unknown, out: NodeRec[]): void {
  if (input === null || input === undefined || typeof input === 'boolean') {
    return;
  }
  if (input instanceof NodeRec) {
    out.push(input);
    return;
  }
  if (Array.isArray(input)) {
    for (const item of input) {
      gatherRecs(item, out);
    }
    return;
  }
  if (typeof input === 'object') {
    const o = input as any;
    if (typeof Node !== 'undefined' && o instanceof Node) {
      const rec = new NodeRec('raw', o);
      rec.dom = o;
      out.push(rec);
      return;
    }
    if (typeof o.snapshot === 'function' && typeof o.watch === 'function') {
      return;
    }
    if (followable(o)) {
      out.push(boundText(o));
      return;
    }
  }
  if (typeof input === 'string' && MARKUP.test(input)) {
    for (const node of parseMarkup(input)) {
      const rec = new NodeRec('raw', node);
      rec.dom = node;
      out.push(rec);
    }
    return;
  }
  out.push(plainText(String(input)));
}

function attachTo(parent: Node, rec: NodeRec): void {
  switch (rec.kind) {
    case 'el':
    case 'text':
    case 'raw':
      parent.appendChild(rec.dom as Node);
      for (const extra of rec.trail) {
        parent.appendChild(extra);
      }
      break;
    case 'comp':
      if (rec.out) {
        attachTo(parent, rec.out);
      }
      break;
    default:
      for (const k of rec.kids) {
        attachTo(parent, k);
      }
  }
}

function mountRec(rec: NodeRec): void {
  if (rec.kind === 'comp') {
    if (rec.out) {
      mountRec(rec.out);
    }
    if (!rec.placed) {
      rec.placed = true;
      (rec.target as View<any>).afterRender(rec.out ?? emptyGroup());
    }
    return;
  }
  if (rec.kind === 'el' || rec.kind === 'group') {
    for (const k of rec.kids) {
      mountRec(k);
    }
  }
}

function dropSubs(rec: NodeRec): void {
  if (rec.kind === 'comp') {
    return;
  }
  const held = rec.subs.splice(0);
  for (const s of held) {
    s.destroy();
  }
  for (const k of rec.kids) {
    dropSubs(k);
  }
}

function writeAttr(el: Element, name: string, v: unknown): void {
  let ns: string | null = null;
  if (name.startsWith('xlink:')) {
    ns = XLINK_NS;
  } else if (name.startsWith('xml:')) {
    ns = XML_NS;
  }
  const attrName = name === 'htmlFor' ? 'for' : name;
  const flag = FLAG_ATTRS.has(name);
  let text: string | null;
  if (typeof v === 'boolean' && flag) {
    text = v ? '' : null;
  } else {
    text = String(v);
  }
  if (name === 'value' && text !== null && 'value' in el) {
    (el as any).value = text;
  }
  if (text === null) {
    if (ns) {
      el.removeAttributeNS(ns, attrName.slice(attrName.indexOf(':') + 1));
    } else {
      el.removeAttribute(attrName);
    }
  } else if (ns) {
    el.setAttributeNS(ns, attrName, text);
  } else {
    el.setAttribute(attrName, text);
  }
}

function bindClass(el: Element, rec: NodeRec, v: any): void {
  if (v === null || v === undefined) {
    return;
  }
  if (typeof v === 'string') {
    el.setAttribute('class', v);
    return;
  }
  if (typeof v.get === 'function') {
    if (v.get() instanceof Set) {
      const paint = (s: ReadonlySet<string>): void => el.setAttribute('class', Array.from(s).join(' '));
      if (typeof v.watch === 'function') {
        rec.subs.push(v.watch(() => paint(v.get()), true));
      } else {
        const joined = v.map((s: ReadonlySet<string>) => Array.from(s).join(' '));
        rec.subs.push(joined, joined.watch((c: string) => el.setAttribute('class', c), true));
      }
    } else {
      rec.subs.push(v.watch((c: string | undefined) => el.setAttribute('class', String(c)), true));
    }
    return;
  }
  for (const key of Object.keys(v)) {
    const names = UI.splitClasses(key);
    const flip = (on: boolean): void => {
      for (const n of names) {
        if (on) {
          el.classList.add(n);
        } else {
          el.classList.remove(n);
        }
      }
    };
    const state = v[key];
    if (followable(state)) {
      rec.subs.push(state.watch((on: boolean) => flip(!!on), true));
    } else {
      flip(!!state);
    }
  }
}

function writeStyleProp(el: Element, key: string, v: unknown): void {
  const css = (el as HTMLElement).style;
  const text = shownText(v);
  if (key.includes('-')) {
    if (text === '') {
      css.removeProperty(key);
    } else {
      css.setProperty(key, text);
    }
  } else {
    (css as any)[key] = text;
  }
}

function bindStyle(el: Element, rec: NodeRec, v: any): void {
  if (v === null || v === undefined) {
    return;
  }
  const writeWhole = (s: unknown): void => {
    if (s === null || s === undefined) {
      el.removeAttribute('style');
    } else {
      el.setAttribute('style', String(s));
    }
  };
  if (typeof v === 'string') {
    writeWhole(v);
    return;
  }
  if (typeof v.get === 'function') {
    if (v.get() instanceof Map) {
      const live = new Set<string>();
      const css = (el as HTMLElement).style;
      rec.subs.push(
        v.watch(() => {
          const table: ReadonlyMap<string, string> = v.get();
          for (const k of Array.from(live)) {
            if (!table.has(k)) {
              css.removeProperty(k);
              live.delete(k);
            }
          }
          table.forEach((val, k) => {
            css.setProperty(k, val);
            live.add(k);
          });
        }, true),
      );
    } else {
      rec.subs.push(v.watch(writeWhole, true));
    }
    return;
  }
  for (const key of Object.keys(v)) {
    const entry = v[key];
    if (followable(entry)) {
      rec.subs.push(entry.watch((x: unknown) => writeStyleProp(el, key, x), true));
    } else {
      writeStyleProp(el, key, entry);
    }
  }
}

function buildIntrinsic(tag: string, props: any, children: unknown[]): NodeRec {
  const el: Element = SVG_TAGS.has(tag) ? document.createElementNS(SVG_NS, tag) : document.createElement(tag);
  const rec = new NodeRec('el', el);
  rec.dom = el;
  if (props) {
    for (const key of Object.keys(props)) {
      const v = props[key];
      if (key === 'children' || key === 'ref' || key === 'key') {
        continue;
      }
      if (key === 'class' || key === 'className') {
        bindClass(el, rec, v);
      } else if (key === 'style') {
        bindStyle(el, rec, v);
      } else if (typeof v === 'function') {
        if (key.length > 2 && key.startsWith('on')) {
          el.addEventListener(key.slice(2).toLowerCase(), v);
        }
      } else if (followable(v)) {
        rec.subs.push(v.watch((x: unknown) => writeAttr(el, key, x), true));
      } else {
        writeAttr(el, key, v);
      }
    }
  }
  gatherRecs(children, rec.kids);
  for (const k of rec.kids) {
    attachTo(el, k);
  }
  if (props && props.ref instanceof Ref) {
    props.ref.target = el;
  }
  return rec;
}

function interpretResult(res: unknown): NodeRec | null {
  if (res === null) {
    return null;
  }
  if (res instanceof NodeRec) {
    return res;
  }
  const group = emptyGroup();
  gatherRecs(res, group.kids);
  return group;
}

function buildClassNode(Ctor: new (p: any) => View<any>, props: any): NodeRec {
  const instance = new Ctor(props);
  const rec = new NodeRec('comp', instance);
  compRecs.set(instance, rec);
  if (props.ref instanceof Ref) {
    props.ref.target = instance;
  }
  const produced = instance.render();
  rec.out = produced === null || produced === undefined ? null : interpretResult(produced);
  return rec;
}

export abstract class View<P, Contexts extends unknown[] = []> {
  props: P & ViewProps;

  constructor(props: P) {
    this.props = props as P & ViewProps;
  }

  afterRender(node: ViewNode): void {
    this.afterMount(node);
  }

  afterMount(_node: ViewNode): void {}

  abstract render(): ViewNode | null;

  destroy(): void {
    const rec = compRecs.get(this);
    if (rec && rec.out) {
      dropSubs(rec.out);
    }
  }
}

export abstract class ScopedView<P, Contexts extends unknown[] = []> extends View<P, Contexts> {
  private readonly heldLife: Array<{ destroy(): void }> = [];

  protected readonly scope: Lifespan = {
    adopt: (item) => {
      this.heldLife.push(item);
    },
  };

  destroy(): void {
    for (const item of this.heldLife.splice(0)) {
      item.destroy();
    }
    super.destroy();
  }
}

export namespace UI {
  export function node(type: any, props: any, ...children: any[]): ViewNode | null {
    if (typeof type === 'string') {
      return buildIntrinsic(type, props, children);
    }
    if (typeof type !== 'function') {
      return null;
    }
    const full = { ...(props ?? {}), children };
    if (type.prototype instanceof View) {
      return buildClassNode(type, full);
    }
    return interpretResult(type(full) ?? emptyGroup());
  }

  export function Group(props: ViewProps): ViewChildren[] | undefined {
    return props.children;
  }

  export function mount(node: ViewNode, ref: HTMLElement | SVGElement | null): void {
    if (!ref || !(node instanceof NodeRec)) {
      return;
    }
    attachTo(ref, node);
    mountRec(node);
  }

  export function ref<T>(): Ref<T> {
    return new Ref<T>();
  }

  export function remove(element: Element | null): void {
    if (element && element.parentNode) {
      element.parentNode.removeChild(element);
    }
  }

  export function splitClasses(classString: string, filter?: (cssClass: string) => boolean): string[] {
    return classString.split(/\s+/).filter((c) => c.length > 0 && (!filter || filter(c)));
  }

  export function childNodes(_thisNode: ViewNode, children: ViewChildren[]): ViewNode[] | null {
    const found: NodeRec[] = [];
    gatherRecs(children, found);
    return found.length > 0 ? found : null;
  }

  export namespace JSX {
    export interface ElementClass {
      render(): ViewNode | null;
    }
    export interface ElementAttributesProperty {
      props: any;
    }
    export interface IntrinsicAttributes {
      ref?: Ref<any>;
      key?: string | number;
    }
    export interface IntrinsicElements {
      [elemName: string]: any;
    }
  }
}

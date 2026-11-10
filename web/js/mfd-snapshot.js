// The MFD page's rendered document for the native MFD (a380-displays mfdx/dom.cpp): every
// element with its computed style in data-cs, sent as topic a380.mfd.dom on the xp-ui.bus
// relay while L:A380X_NATIVE_MFD_DOM is up. Same string format as mfd_native's document().
(function () {
  'use strict';

  const GATE = 'L:A380X_NATIVE_MFD_DOM';
  const KEY = 'xp-ui.bus';
  const TOPIC = 'a380.mfd.dom';
  const MIN_INTERVAL_MS = 100;
  const REFRESH_MS = 500;

  // The properties the native renderer reads (css_cascade PROPS; any order), and the values it
  // fills back in when missing (mfd_native CS_DEFAULTS), so those are left out.
  const SIDES = ['top', 'right', 'bottom', 'left'];
  const side = (f) => SIDES.map(f);
  const PROPS = [
    'display', 'position', ...SIDES,
    ...['width', 'height'].flatMap((d) => [d, `min-${d}`, `max-${d}`]),
    ...side((s) => `margin-${s}`), ...side((s) => `padding-${s}`),
    ...['width', 'style', 'color'].flatMap((k) => side((s) => `border-${s}-${k}`)),
    ...['top-left', 'top-right', 'bottom-right', 'bottom-left'].map((c) => `border-${c}-radius`),
    'box-sizing', 'order', 'gap', 'row-gap', 'column-gap',
    ...['direction', 'wrap', 'grow', 'shrink', 'basis'].map((k) => `flex-${k}`),
    'justify-content', ...['items', 'self', 'content'].map((k) => `align-${k}`),
    ...['template-columns', 'template-rows', 'column', 'row'].map((k) => `grid-${k}`),
    ...['family', 'size', 'weight', 'style'].map((k) => `font-${k}`), 'line-height', 'letter-spacing',
    ...['align', 'transform', 'decoration-line', 'overflow'].map((k) => `text-${k}`), 'vertical-align', 'white-space',
    'color', ...['color', 'image', 'size', 'position'].map((k) => `background-${k}`), 'box-shadow',
    'opacity', 'visibility', 'overflow', 'transform', 'transform-origin', 'z-index',
    'filter', 'backdrop-filter', 'pointer-events', 'aspect-ratio',
  ];
  const FILLED = new Map([
    ...side((s) => [s, 'auto']),
    ...['min-width', 'min-height', ...['margin', 'padding'].flatMap((b) => side((s) => `${b}-${s}`)), ...side((s) => `border-${s}-width`)].map((p) => [p, '0px']),
    ...['max-width', 'max-height', 'border-top-style', 'grid-template-columns', 'grid-template-rows', 'text-decoration-line', 'text-transform', 'transform'].map((p) => [p, 'none']),
    ...['justify-content', 'align-items', 'align-content', 'gap', 'row-gap', 'column-gap'].map((p) => [p, 'normal']),
    ...['align-self', 'flex-basis', 'grid-column', 'grid-row', 'z-index'].map((p) => [p, 'auto']),
    ['position', 'static'], ['flex-direction', 'row'], ['flex-wrap', 'nowrap'], ['flex-grow', '0'], ['flex-shrink', '1'], ['order', '0'],
    ['vertical-align', 'baseline'], ['background-color', 'rgba(0, 0, 0, 0)'], ['opacity', '1'], ['visibility', 'visible'], ['overflow', 'visible'],
  ]);
  const PAINT = ['fill', 'stroke'].flatMap((k) => (k === 'fill' ? [k, `${k}-opacity`] : [k, ...['width', 'opacity', 'dasharray', 'linecap', 'linejoin'].map((x) => `${k}-${x}`)]));
  const SHAPE_PROPS = ['display', 'visibility', 'opacity', ...PAINT];
  const TEXT_PROPS = ['family', 'size', 'weight'].map((k) => `font-${k}`).concat('letter-spacing');
  const SKIP = new Set('head title meta link style script noscript template'.split(' '));
  const VOID = new Set('br hr img input wbr col area base embed link meta param source track'.split(' '));
  const SVG_NS = 'http://www.w3.org/2000/svg';

  const escText = (s) => s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
  const escAttr = (s) => s.replace(/&/g, '&amp;').replace(/"/g, '&quot;').replace(/</g, '&lt;');

  function elementStyle(style) {
    const color = style.getPropertyValue('color');
    const out = [];
    for (const p of PROPS) {
      const v = style.getPropertyValue(p);
      if (FILLED.get(p) === v) continue;
      if (p.startsWith('border-') && p.endsWith('-color') && v === color) continue;
      out.push(`${p}:${v}`);
    }
    return out.join(';');
  }

  function shapeStyle(el, style) {
    const out = [];
    for (const p of SHAPE_PROPS) out.push(`${p}:${style.getPropertyValue(p)}`);
    if (!el.hasAttribute('transform')) out.push(`transform:${style.getPropertyValue('transform')}`);
    if (el.localName === 'text' || el.localName === 'tspan') for (const p of TEXT_PROPS) out.push(`${p}:${style.getPropertyValue(p)}`);
    return out.join(';');
  }

  function hidden(el, out) {
    out.push('<i data-cs="display:none">');
    for (const c of el.children) if (!SKIP.has(c.localName)) hidden(c, out);
    out.push('</i>');
  }

  function attributes(el, out) {
    const input = el.localName === 'input' || el.localName === 'textarea';
    for (const a of el.attributes) {
      if (a.name === 'data-cs' || a.name === 'data-st' || a.name === 'data-sl' || (input && (a.name === 'value' || a.name === 'checked'))) continue;
      out.push(` ${a.name}="${escAttr(a.value)}"`);
    }
    if (input) {
      if (el.checked) out.push(' checked=""');
      out.push(` value="${escAttr(String(el.value))}"`);
    }
    if (el.scrollTop > 0) out.push(` data-st="${el.scrollTop}"`);
    if (el.scrollLeft > 0) out.push(` data-sl="${el.scrollLeft}"`);
  }

  function node(el, inSvg, out) {
    const tag = el.localName;
    if (SKIP.has(tag)) return;
    const style = getComputedStyle(el);
    const shape = inSvg && el.namespaceURI === SVG_NS && tag !== 'svg';
    if (!shape && style.getPropertyValue('display') === 'none') {
      hidden(el, out);
      return;
    }
    out.push(`<${tag}`);
    attributes(el, out);
    out.push(` data-cs="${escAttr(shape ? shapeStyle(el, style) : elementStyle(style))}">`);
    if (VOID.has(tag)) return;
    const svg = inSvg || tag === 'svg';
    for (const c of el.childNodes) {
      if (c.nodeType === 1) node(c, svg, out);
      else if (c.nodeType === 3) out.push(escText(c.data));
    }
    out.push(`</${tag}>`);
  }

  function documentHtml() {
    const out = [];
    node(document.body, false, out);
    return out.join('');
  }

  function start() {
    const data = window.xplaneJsData;
    if (!data || !document.body) return;
    const relay = data.openRelay();
    let dirty = true;
    let last = '';
    let sentAt = -Infinity;
    new MutationObserver(() => {
      dirty = true;
    }).observe(document.body, { subtree: true, childList: true, attributes: true, characterData: true });
    document.addEventListener('scroll', () => {
      dirty = true;
    }, true);
    const tick = (now) => {
      requestAnimationFrame(tick);
      if (!data.Vars.read(GATE, 'number')) {
        last = '';
        return;
      }
      if (now - sentAt < MIN_INTERVAL_MS || (!dirty && now - sentAt < REFRESH_MS)) return;
      dirty = false;
      sentAt = now;
      const html = documentHtml();
      if (html === last) return;
      last = html;
      relay.send(KEY, JSON.stringify({ events: [{ topic: TOPIC, data: html }] }));
    };
    requestAnimationFrame(tick);
  }

  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', start, { once: true });
  else start();
})();

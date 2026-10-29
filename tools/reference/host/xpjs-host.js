// Loads a screen's gauges (JS Library for X-Plane page format) into one page in the
// VCockpit host's order: one gauge at a time, each instrument element placed at its
// panel.cfg rectangle and attached as soon as it is created, before the next gauge loads.
(function () {
  'use strict';
  const cfg = window.__REFERENCE__ || {};
  const gauges = (cfg.screen && cfg.screen.gauges) || [];
  window.__xpjsGauges = gauges;
  let current = -1;
  let created = -1;
  let lastTag = null;
  const append = HTMLBodyElement.prototype.appendChild;
  // The runtime mounts each instrument in its own panel element, as in XPHFBW; here it is placed at its
  // panel.cfg rectangle and its url stands in for the address of the view XPHFBW would give that gauge.
  HTMLBodyElement.prototype.appendChild = function (node) {
    const inst = !(node instanceof HTMLElement) ? null : node.tagName === 'VCOCKPIT-PANEL' ? node.firstElementChild : node.tagName.includes('-') ? node : null;
    if (inst && current >= 0 && created < current) {
      const i = current;
      const g = gauges[i];
      created = i;
      lastTag = inst.tagName;
      inst.setAttribute('guid', String(i));
      inst.setAttribute('url', new URL('/Pages/VCockpit/Instruments/' + g.url, location.origin).href);
      inst.style.position = 'absolute';
      inst.style.left = `${g.x}px`;
      inst.style.top = `${g.y}px`;
      inst.style.width = `${g.w}px`;
      inst.style.height = `${g.h}px`;
      let panel = node;
      if (node === inst) {
        panel = document.createElement('vcockpit-panel');
        panel.appendChild(inst);
      }
      append.call(this, panel);
      Promise.resolve().then(() => load(i + 1));
      return node;
    }
    return append.call(this, node);
  };

  // A gauge's script runs while its own page is still parsing: the page reads as loading,
  // and DOMContentLoaded listeners added meanwhile run once the script has loaded.
  let parsing = false;
  const ready = [];
  const state = Object.getOwnPropertyDescriptor(Document.prototype, 'readyState');
  Object.defineProperty(document, 'readyState', { configurable: true, get: () => (parsing ? 'loading' : state.get.call(document)) });
  const listen = document.addEventListener;
  document.addEventListener = function (type, fn, opts) {
    if (parsing && type === 'DOMContentLoaded') {
      ready.push(fn);
      return;
    }
    return listen.call(this, type, fn, opts);
  };
  const script = (src) =>
    new Promise((resolve) => {
      const s = document.createElement('script');
      s.src = src;
      s.async = false;
      s.onload = s.onerror = () => {
        parsing = false;
        for (const fn of ready.splice(0)) typeof fn === 'function' ? fn.call(document, new Event('DOMContentLoaded')) : fn.handleEvent(new Event('DOMContentLoaded'));
        resolve();
      };
      parsing = true;
      document.head.appendChild(s);
    });
  const css = (href) =>
    new Promise((resolve) => {
      const l = document.createElement('link');
      l.rel = 'stylesheet';
      l.href = href;
      l.onload = l.onerror = () => resolve();
      document.head.appendChild(l);
    });

  const tags = new Map();
  async function load(i) {
    if (i >= gauges.length) {
      for (const src of (cfg.pageScripts || '').split(',').filter(Boolean)) await script(src);
      return;
    }
    current = i;
    const url = new URL('/Pages/VCockpit/Instruments/' + gauges[i].url, location.origin);
    if (i > 0 && created === i - 1) tags.set(gauges[i - 1].url.split('?')[0].toLowerCase(), lastTag);
    const seen = tags.get(gauges[i].url.split('?')[0].toLowerCase());
    if (seen) {
      document.body.appendChild(document.createElement(seen));
      return;
    }
    const text = await (await fetch(url.pathname)).text();
    const doc = new DOMParser().parseFromString(text, 'text/html');
    for (const link of doc.querySelectorAll('link[rel="stylesheet"]')) await css(new URL(link.getAttribute('href'), url).pathname);
    for (const t of doc.querySelectorAll('template')) if (!document.getElementById(t.id)) append.call(document.body, document.importNode(t, true));
    for (const s of doc.querySelectorAll('script[src]')) await script(new URL(s.getAttribute('src'), url).pathname);
    if (created < i) load(i + 1);
  }

  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', () => load(0));
  else load(0);
})();

let mdTree = null;
const mdCache = new Map();
const mdChecks = new Map();

async function mdLoadTree() {
  if (!location.protocol.startsWith('http')) { mdTree = { kids: [] }; return; }
  try { mdTree = await (await fetch('/api/documents')).json(); } catch { mdTree = { kids: [] }; }
}

function mdNodes(list, trail) {
  const out = { kids: [], leaves: [] };
  for (const n of list || []) {
    if (n.kids) out.kids.push({ key: 'md:' + n.path, t: n.name, ...mdNodes(n.kids, [...trail, n.name]) });
    else out.leaves.push({ id: 'md:' + n.path, t: n.name, kind: n.type === 'image' ? 'mdimg' : n.type === 'pdf' ? 'mdpdf' : 'md', path: n.path });
  }
  return out;
}

function mdEsc(s) { return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;'); }

function mdDir(path) { const i = (path || '').lastIndexOf('/'); return i < 0 ? '' : path.slice(0, i + 1); }

function mdSlug(t) { return t.toLowerCase().replace(/<[^>]+>/g, '').replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, ''); }

function mdInline(s, base) {
  const codes = [];
  let h = mdEsc(s).replace(/`([^`]+)`/g, (_, c) => { codes.push(c); return `\u0000${codes.length - 1}\u0000`; });
  h = h.replace(/!\[([^\]]*)\]\(([^)\s]+)\)/g, (_, alt, src) => `<img class="md-img" alt="${alt}" src="${/^https?:/.test(src) ? src : '/api/docfile?path=' + encodeURIComponent(base + src)}">`);
  h = h.replace(/\*\*([^*]+)\*\*/g, '<b>$1</b>').replace(/__([^_]+)__/g, '<b>$1</b>')
    .replace(/(^|[^*])\*([^*]+)\*/g, '$1<i>$2</i>').replace(/(^|[^_\w])_([^_]+)_/g, '$1<i>$2</i>')
    .replace(/~~([^~]+)~~/g, '<s>$1</s>')
    .replace(/\[([^\]]+)\]\(([^)\s]+)\)/g, (_, t, href) => `<span class="md-link" data-href="${href.startsWith('#') ? href : /^https?:/.test(href) ? href : base + href}">${t}</span>`);
  return h.replace(/\u0000(\d+)\u0000/g, (_, i) => `<code>${codes[+i]}</code>`);
}

function mdCheckLine(text, base) {
  const parts = text.split(/\s*\.{3,}\s*|\s+\|\s+/);
  const name = parts[0], value = parts.slice(1).join(' ');
  return `<div class="qd-line md-check"><span class="qd-n">${mdInline(name, base)}</span>${value ? '<span class="qd-dots"></span><span class="qd-v">' + mdInline(value, base) + '</span>' : ''}</div>`;
}

function mdRender(text, path) {
  const base = mdDir(path);
  const root = el('div', { class: 'md' });
  const lines = text.replace(/\r\n/g, '\n').split('\n');
  const outline = [];
  const used = new Map();
  let i = 0;
  const html = [];
  const heading = (level, raw) => {
    const inner = mdInline(raw, base);
    let id = mdSlug(raw) || 'section';
    const n = used.get(id) || 0;
    used.set(id, n + 1);
    if (n) id += '-' + n;
    outline.push({ level, id, text: raw.replace(/[*_`~]/g, '') });
    return `<div class="md-h${level}" id="mdh-${id}">${inner}</div>`;
  };
  while (i < lines.length) {
    const line = lines[i];
    const open = line.match(/^(?::::\s*(?:details|collapse)\s+(.*)|<details>\s*(?:<summary>(.*?)<\/summary>)?\s*)$/i);
    if (open) {
      const title = (open[1] || open[2] || 'Details').trim();
      html.push(`<details class="md-fold"><summary class="qd-card"><span>${mdInline(title, base)}</span><span class="qd-chev">▾</span></summary><div class="qd-cardbody">`);
      outline.push({ level: 3, id: '', text: title, fold: true });
      i++;
      continue;
    }
    if (/^(:::|<\/details>)\s*$/i.test(line)) { html.push('</div></details>'); i++; continue; }
    if (/^```/.test(line)) {
      const buf = [];
      i++;
      while (i < lines.length && !/^```/.test(lines[i])) buf.push(mdEsc(lines[i++]));
      i++;
      html.push(`<pre class="md-pre">${buf.join('\n')}</pre>`);
      continue;
    }
    const h = line.match(/^(#{1,4})\s+(.*)$/);
    if (h) { html.push(heading(h[1].length, h[2])); i++; continue; }
    if (/^\s*(-{3,}|\*{3,})\s*$/.test(line)) { html.push('<hr class="md-hr">'); i++; continue; }
    if (/^\s*>/.test(line)) {
      const buf = [];
      while (i < lines.length && /^\s*>/.test(lines[i])) buf.push(mdInline(lines[i++].replace(/^\s*>\s?/, ''), base));
      html.push(`<div class="md-quote">${buf.join('<br>')}</div>`);
      continue;
    }
    if (/^\|.*\|\s*$/.test(line) && i + 1 < lines.length && /^\|?\s*:?-+/.test(lines[i + 1])) {
      const row = (l) => l.trim().replace(/^\||\|$/g, '').split('|').map((c) => mdInline(c.trim(), base));
      const head = row(line);
      i += 2;
      const body = [];
      while (i < lines.length && /^\|.*\|\s*$/.test(lines[i])) body.push(row(lines[i++]));
      html.push(`<table class="md-table"><tr>${head.map((c) => `<th>${c}</th>`).join('')}</tr>${body.map((r) => `<tr>${r.map((c) => `<td>${c}</td>`).join('')}</tr>`).join('')}</table>`);
      continue;
    }
    if (/^\s*[-*+]\s+\[[ xX]\]\s+/.test(line)) {
      while (i < lines.length && /^\s*[-*+]\s+\[[ xX]\]\s+/.test(lines[i])) html.push(mdCheckLine(lines[i++].replace(/^\s*[-*+]\s+\[[ xX]\]\s+/, ''), base));
      continue;
    }
    if (/^\s*([-*+]|\d+\.)\s+/.test(line)) {
      const items = [];
      const ordered = /^\s*\d+\./.test(line);
      while (i < lines.length && /^\s*([-*+]|\d+\.)\s+/.test(lines[i]) && !/^\s*[-*+]\s+\[[ xX]\]\s+/.test(lines[i])) {
        const m = lines[i].match(/^(\s*)([-*+]|\d+\.)\s+(.*)$/);
        items.push(`<li class="md-l${Math.min(3, Math.floor(m[1].length / 2))}">${mdInline(m[3], base)}</li>`);
        i++;
      }
      html.push(`<${ordered ? 'ol' : 'ul'} class="md-list">${items.join('')}</${ordered ? 'ol' : 'ul'}>`);
      continue;
    }
    if (!line.trim()) { i++; continue; }
    const buf = [];
    while (i < lines.length && lines[i].trim() && !/^(#{1,4}\s|```|\s*>|\||:::|<\/?details)/i.test(lines[i]) && !/^\s*([-*+]|\d+\.)\s+/.test(lines[i])) buf.push(mdInline(lines[i++], base));
    html.push(`<p class="md-p">${buf.join('<br>')}</p>`);
  }
  root.innerHTML = html.join('');
  root.mdOutline = outline;
  return root;
}

function mdWire(root, key) {
  const ticks = mdChecks.get(key) || new Set();
  mdChecks.set(key, ticks);
  const checks = [...root.querySelectorAll('.md-check')];
  const mark = () => {
    let current = -1;
    checks.forEach((c, n) => {
      c.classList.toggle('done', ticks.has(n));
      if (current < 0 && !ticks.has(n)) current = n;
    });
    checks.forEach((c, n) => c.classList.toggle('current', n === current));
  };
  checks.forEach((c, n) => c.addEventListener('click', () => { ticks.has(n) ? ticks.delete(n) : ticks.add(n); mark(); }));
  mark();
  for (const a of root.querySelectorAll('.md-link')) {
    a.addEventListener('click', () => {
      const href = a.dataset.href || '';
      if (href.startsWith('#')) { root.querySelector('#mdh-' + mdSlug(href.slice(1)))?.scrollIntoView({ behavior: 'smooth', block: 'start' }); return; }
      if (/^https?:/.test(href)) { window.open(href, '_blank'); return; }
      const [file, frag] = href.split('#');
      docSel = 'md:' + file;
      renderDocs();
      if (frag) setTimeout(() => document.querySelector('#mdh-' + mdSlug(frag))?.scrollIntoView({ block: 'start' }), 60);
    });
  }
}

function mdOutlinePane(root) {
  const items = (root.mdOutline || []).filter((o) => o.id || o.fold);
  if (items.length < 2) return null;
  const pane = el('div', { class: 'md-toc' }, el('div', { class: 'md-toc-h', text: 'ON THIS PAGE' }));
  const min = Math.min(...items.map((o) => o.level));
  const folds = [...root.querySelectorAll('details.md-fold')];
  let f = 0;
  for (const o of items) {
    const b = el('button', { type: 'button', class: 'md-toc-i', style: `padding-left:${12 + (o.level - min) * 16}px`, text: o.text });
    const fold = o.fold ? folds[f++] : null;
    b.addEventListener('click', () => {
      const t = fold || root.querySelector('#mdh-' + o.id);
      if (fold) fold.open = true;
      t?.scrollIntoView({ behavior: 'smooth', block: 'start' });
    });
    pane.append(b);
  }
  return pane;
}

function mdShow(body, root, key) {
  mdWire(root, key);
  const toc = mdOutlinePane(root);
  if (!toc) { body.append(root); return; }
  body.append(el('div', { class: 'md-wrap' }, el('div', { class: 'md-col' }, root), toc));
}

async function mdPage(body, it) {
  if (it.kind === 'mdimg') { body.append(el('img', { class: 'md-img md-full', src: '/api/docfile?path=' + encodeURIComponent(it.path), alt: it.t })); return; }
  if (it.kind === 'mdpdf') { body.append(el('iframe', { class: 'md-pdf', src: '/api/docfile?path=' + encodeURIComponent(it.path) })); return; }
  let text = mdCache.get(it.path);
  if (text == null) {
    body.append(el('div', { class: 'qd-empty', text: 'Loading...' }));
    try { text = await (await fetch('/api/document?path=' + encodeURIComponent(it.path))).text(); } catch { text = '# ' + it.t + '\nCould not read this document.'; }
    mdCache.set(it.path, text);
    renderDocs();
    return;
  }
  mdShow(body, mdRender(text, it.path), it.id);
}

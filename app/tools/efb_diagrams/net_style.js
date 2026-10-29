const NS = {
  bg: '#25272b', green: '#3fd27f', orange: '#ffb000', red: '#ff3d63', grey: '#61656c', greyText: '#8b9097',
  fill: '#1f2226', warnFill: '#3a2f14', failFill: '#ff3d63',
};

function nsSquircle(x, y, w, h, r) {
  const c = Math.min(r * 1.5, w / 2, h / 2);
  const p = 2 / 4.5;
  const f = (v) => Math.pow(Math.max(0, v), p);
  const pts = [];
  const arc = (cx, cy, sx, sy, from, to) => {
    for (let i = 0; i <= 10; i++) {
      const t = from + ((to - from) * i) / 10;
      pts.push([cx + sx * c * f(Math.cos(t)), cy + sy * c * f(Math.sin(t))]);
    }
  };
  const q = Math.PI / 2;
  arc(x + w - c, y + c, 1, -1, q, 0);
  arc(x + w - c, y + h - c, 1, 1, 0, q);
  arc(x + c, y + h - c, -1, 1, q, 0);
  arc(x + c, y + c, -1, -1, 0, q);
  return 'M' + pts.map((pt) => pt[0].toFixed(1) + ',' + pt[1].toFixed(1)).join(' L') + 'Z';
}

function nsStroke(state) {
  return state === 'fail' ? NS.red : state === 'warn' ? NS.orange : state === 'live' || state === 'alt' ? NS.green : NS.grey;
}

function nsBox(x, y, w, h, label, state, o = {}) {
  const c = nsStroke(state);
  const fill = state === 'fail' ? NS.failFill : state === 'warn' ? NS.warnFill : NS.fill;
  const txt = state === 'fail' ? '#fff' : state === 'off' ? NS.greyText : c;
  const lines = String(label).split('\n');
  const size = o.size || 13;
  const ty = y + h / 2 + size * 0.36 - ((lines.length - 1) * (size + 2)) / 2;
  return `<path d="${nsSquircle(x, y, w, h, o.r || 9)}" fill="${fill}" stroke="${c}" stroke-width="${state === 'off' ? 1.4 : 2.2}"/>` +
    lines.map((l, i) => exT(x + w / 2, ty + i * (size + 2), l, { size, fill: txt })).join('') +
    (o.tag ? exT(x + w / 2, y - 5, o.tag, { size: 10, fill: c }) : '');
}

function nsWire(pts, state, dash, o = {}) {
  const stroke = nsStroke(state);
  const d = pts.map((p, i) => (i ? 'L' : 'M') + p[0].toFixed(1) + ',' + p[1].toFixed(1)).join('');
  return `<path d="${d}" fill="none" stroke="${stroke}" stroke-width="${state === 'off' ? 1.4 : 2.6}" stroke-linejoin="round"${dash ? ` stroke-dasharray="${dash}"` : ''}${state === 'alt' && o.arrow !== false ? ' marker-end="url(#nsArrow)"' : ''}/>`;
}

function nsElbow(a, b, state, dash) {
  const ym = (a[1] + b[1]) / 2;
  return nsWire([a, [a[0], ym], [b[0], ym], b], state, dash);
}

function nsSvg(w, h, parts, focus) {
  const defs = `<defs><marker id="nsArrow" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse"><path d="M0,0 L10,5 L0,10 z" fill="${NS.green}"/></marker></defs>`;
  const d = exSvg(w, h, [defs, `<rect x="0" y="0" width="${w}" height="${h}" rx="10" fill="${NS.bg}"/>`, ...parts]);
  if (!focus) return d;
  const pad = focus.length > 4 ? focus[4] : 40;
  let [x, y, fw, fh] = [focus[0] - pad, focus[1] - pad, focus[2] + pad * 2, focus[3] + pad * 2];
  const minW = focus.length > 4 ? Math.min(520, focus[2] + pad * 2) : Math.min(w, 720), minH = Math.min(h, 380);
  if (fw < minW) { x -= (minW - fw) / 2; fw = minW; }
  if (fh < minH) { y -= (minH - fh) / 2; fh = minH; }
  x = Math.max(0, Math.min(x, w - fw));
  y = Math.max(0, Math.min(y, h - fh));
  const svg = d.querySelector('svg');
  const crop = `${x.toFixed(0)} ${y.toFixed(0)} ${Math.min(fw, w).toFixed(0)} ${Math.min(fh, h).toFixed(0)}`;
  const full = `0 0 ${w} ${h}`;
  svg.setAttribute('viewBox', crop);
  d.classList.add('ns-crop');
  d.title = 'Tap for the full diagram';
  d.addEventListener('click', () => svg.setAttribute('viewBox', svg.getAttribute('viewBox') === full ? crop : full));
  return d;
}

function nsBounds(rects) {
  if (!rects.length) return null;
  const x0 = Math.min(...rects.map((r) => r[0])), y0 = Math.min(...rects.map((r) => r[1]));
  const x1 = Math.max(...rects.map((r) => r[0] + r[2])), y1 = Math.max(...rects.map((r) => r[1] + r[3]));
  return [x0, y0, x1 - x0, y1 - y0];
}

function nsValve(x, y, state, open, label, o = {}) {
  const c = nsStroke(state);
  const bar = open ? `M${x},${y - 7} L${x},${y + 7}` : `M${x - 7},${y} L${x + 7},${y}`;
  return `<circle cx="${x}" cy="${y}" r="9" fill="${state === 'fail' ? NS.red : NS.fill}" stroke="${c}" stroke-width="2"/><path d="${bar}" stroke="${state === 'fail' ? '#fff' : c}" stroke-width="2.2"/>` +
    (label ? exT(o.left ? x - 13 : x + 13, y + 4, label, { size: 9, anchor: o.left ? 'end' : 'start', fill: state === 'live' || state === 'off' ? NS.greyText : c }) : '');
}

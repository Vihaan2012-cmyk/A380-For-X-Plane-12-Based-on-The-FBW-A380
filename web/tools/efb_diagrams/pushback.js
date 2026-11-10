const PB_VARS = ['PLANE HEADING DEGREES TRUE', 'PLANE LATITUDE', 'PLANE LONGITUDE', 'GROUND VELOCITY', 'PUSHBACK ATTACHED', 'A32NX_PARK_BRAKE_LEVER_POS', 'A32NX_PUSHBACK_SPD_FACTOR', 'A32NX_PUSHBACK_HDG_FACTOR'];
const pb = { v: {}, speed: 0, heading: 0, plan: { dir: 'LEFT', angle: 90, after: 15, pull: 0, pace: 'NORMAL' }, auto: null, timer: null, sent: '' };

function pbDist(a, b) {
  const r = 6371000, p1 = a[0] * Math.PI / 180, p2 = b[0] * Math.PI / 180;
  const dp = p2 - p1, dl = (b[1] - a[1]) * Math.PI / 180;
  const h = Math.sin(dp / 2) ** 2 + Math.cos(p1) * Math.cos(p2) * Math.sin(dl / 2) ** 2;
  return 2 * r * Math.asin(Math.sqrt(h));
}

function pbTurned(from, to) { let d = to - from; while (d > 180) d -= 360; while (d < -180) d += 360; return d; }

async function pbSend(speed, heading) {
  speed = Math.max(-1, Math.min(1, speed));
  heading = Math.max(-1, Math.min(1, heading));
  const key = speed.toFixed(2) + '|' + heading.toFixed(2);
  pb.speed = speed; pb.heading = heading;
  if (key === pb.sent) return;
  pb.sent = key;
  try { await studyPost('/study/action', { kind: 'pushback', speed, heading }); } catch { pb.sent = ''; }
}

async function pbTug() {
  try { await studyPost('/study/action', { kind: 'command', command: 'fbw/pushback/toggle' }); } catch { /* offline */ }
}

function pbStop() { pb.auto = null; pbSend(0, 0); }

function pbStartAuto() {
  const v = pb.v;
  const hdg = +v['PLANE HEADING DEGREES TRUE'] || 0;
  const pos = [+v['PLANE LATITUDE'] || 0, +v['PLANE LONGITUDE'] || 0];
  const steps = [];
  const pace = pb.plan.pace === 'SLOW' ? 0.22 : pb.plan.pace === 'FAST' ? 0.45 : 0.32;
  if (pb.plan.dir === 'STRAIGHT') steps.push({ kind: 'straight', dist: Math.max(10, pb.plan.after), speed: -pace });
  else {
    steps.push({ kind: 'straight', dist: 12, speed: -pace });
    steps.push({ kind: 'turn', angle: pb.plan.angle, tail: pb.plan.dir, speed: -pace });
    if (pb.plan.after > 0) steps.push({ kind: 'straight', dist: pb.plan.after, speed: -pace });
  }
  if (pb.plan.pull > 0) steps.push({ kind: 'straight', dist: pb.plan.pull, speed: pace * 0.8 });
  pb.auto = { steps, i: 0, start: { hdg, pos }, stepStart: { hdg, pos }, t0: performance.now() };
}

function pbAutoTick() {
  const a = pb.auto;
  if (!a) return;
  const v = pb.v;
  if (!(+v['PUSHBACK ATTACHED'])) { pb.auto = null; pbSend(0, 0); return; }
  const hdg = +v['PLANE HEADING DEGREES TRUE'] || 0;
  const pos = [+v['PLANE LATITUDE'] || 0, +v['PLANE LONGITUDE'] || 0];
  const s = a.steps[a.i];
  if (!s) { pbStop(); return; }
  let done = false;
  if (s.kind === 'straight') {
    done = pbDist(a.stepStart.pos, pos) >= s.dist;
    const drift = pbTurned(a.stepStart.hdg, hdg);
    pbSend(s.speed, Math.max(-0.3, Math.min(0.3, drift * 0.08 * (s.speed < 0 ? 1 : -1))));
  } else {
    const turned = Math.abs(pbTurned(a.stepStart.hdg, hdg));
    const left = s.angle - turned;
    done = left <= 1.5;
    const tailLeft = s.tail === 'LEFT';
    const h = (tailLeft ? -1 : 1) * Math.max(0.25, Math.min(1, left / 25));
    pbSend(s.speed * (left < 15 ? 0.7 : 1), h);
  }
  if (done) { a.i++; a.stepStart = { hdg, pos }; if (a.i >= a.steps.length) pbStop(); }
}

const PB_LAYERS = ['apronelement', 'taxiwayelement', 'runwayelement', 'parkingstandarea', 'verticalpolygonalstructure', 'taxiwayguidanceline', 'standguidanceline', 'parkingstandlocation'];
pb.map = { icao: '', layers: null, state: '' };

async function pbLoadMap() {
  const v = pb.v;
  const lat = +v['PLANE LATITUDE'], lon = +v['PLANE LONGITUDE'];
  if (pb.map.state === 'loading' || (!lat && !lon)) return;
  pb.map.state = 'loading';
  let icao = '';
  try { const r = await (await fetch(`/api/nearest?lat=${lat}&lon=${lon}`)).json(); if (r.icao && r.distance_m < 8000) icao = r.icao; } catch { /* no XPHFBW */ }
  if (!icao && typeof lastOfp !== 'undefined' && lastOfp && lastOfp.origin) icao = lastOfp.origin.icao_code || '';
  if (!icao) { pb.map.state = 'none'; return; }
  if (icao === pb.map.icao && pb.map.layers) { pb.map.state = 'ready'; return; }
  pb.map.icao = icao;
  for (let tries = 0; tries < 40; tries++) {
    try {
      const r = await fetch(`${PLUGIN_ORIGIN}/study/amdb?icao=${icao}&include=${PB_LAYERS.join(',')}`);
      if (r.status === 202) { await new Promise((ok) => setTimeout(ok, 1500)); continue; }
      if (!r.ok) break;
      pb.map.layers = await r.json();
      pb.map.state = 'ready';
      const box = document.querySelector('#pb-content .pb-preview');
      if (box) box.innerHTML = pbPathSvg();
      return;
    } catch { break; }
  }
  pb.map.state = 'none';
}

function pbMapLayer(lat0, lon0, hdg, sx, sy, scale) {
  const L = pb.map.layers;
  if (!L) return '';
  const k = Math.cos(lat0 * Math.PI / 180);
  const c = Math.cos(hdg * Math.PI / 180), sn = Math.sin(hdg * Math.PI / 180);
  const pt = ([lon, lat]) => {
    const e = (lon - lon0) * 111320 * k, n = (lat - lat0) * 110540;
    const x = e * c - n * sn, y = e * sn + n * c;
    return `${(sx + x * scale).toFixed(1)},${(sy - y * scale).toFixed(1)}`;
  };
  const near = (ring) => ring.some(([lon, lat]) => Math.abs((lat - lat0) * 110540) < 400 && Math.abs((lon - lon0) * 111320 * k) < 400);
  const out = [];
  const polys = (name, fill, stroke, sw) => {
    for (const f of ((L[name] || {}).features || [])) {
      const g = f.geometry || {};
      const rings = g.type === 'Polygon' ? [g.coordinates] : g.type === 'MultiPolygon' ? g.coordinates : [];
      for (const poly of rings) {
        if (!poly[0] || !near(poly[0])) continue;
        out.push(`<path d="${poly.map((r) => 'M' + r.map(pt).join('L') + 'Z').join('')}" fill="${fill}" fill-rule="evenodd" stroke="${stroke}" stroke-width="${sw}"/>`);
      }
    }
  };
  const lines = (name, stroke, sw, dash) => {
    for (const f of ((L[name] || {}).features || [])) {
      const g = f.geometry || {};
      const ls = g.type === 'LineString' ? [g.coordinates] : g.type === 'MultiLineString' ? g.coordinates : [];
      for (const l of ls) if (l.length > 1 && near(l)) out.push(`<polyline points="${l.map(pt).join(' ')}" fill="none" stroke="${stroke}" stroke-width="${sw}"${dash ? ` stroke-dasharray="${dash}"` : ''}/>`);
    }
  };
  polys('apronelement', '#2c2f34', 'none', 0);
  polys('taxiwayelement', '#383b41', 'none', 0);
  polys('runwayelement', '#45484e', 'none', 0);
  polys('parkingstandarea', 'none', '#6d7178', 1);
  polys('verticalpolygonalstructure', '#4d5158', '#62666d', 1);
  lines('taxiwayguidanceline', '#d9b23a', 1.4);
  lines('standguidanceline', '#d9b23a', 1, '4 3');
  for (const f of ((L.parkingstandlocation || {}).features || [])) {
    const g = f.geometry || {};
    if (g.type !== 'Point' || !near([g.coordinates])) continue;
    const [x, y] = pt(g.coordinates).split(',');
    const id = (f.properties && (f.properties.idstd || f.properties.ident)) || '';
    if (id) out.push(`<text x="${x}" y="${y}" fill="#c7cad0" font-size="10" font-family="Manrope, Segoe UI, sans-serif" text-anchor="middle">${String(id).replace(/[<&]/g, '')}</text>`);
  }
  return out.join('');
}

function pbPathSvg() {
  const W = 420, H = 420, cx = 210, cy = 130, scale = 1.6;
  const p = pb.plan;
  const pts = [[cx, cy]];
  let x = cx, y = cy, h = 0;
  const go = (d) => { x -= Math.sin(h) * d * scale; y += Math.cos(h) * d * scale; pts.push([x, y]); };
  if (p.dir === 'STRAIGHT') go(Math.max(10, p.after));
  else {
    go(12);
    const sign = p.dir === 'LEFT' ? 1 : -1;
    const n = 24, radius = 32;
    const arc = radius * p.angle * Math.PI / 180;
    for (let i = 0; i < n; i++) { h += (sign * p.angle * Math.PI / 180) / n; go(arc / n); }
    if (p.after > 0) go(p.after);
  }
  const path = pts.map(([a, b]) => `${a.toFixed(1)},${b.toFixed(1)}`).join(' ');
  const end = pts[pts.length - 1];
  const k = (72.7 * scale) / 56;
  const plane = (ang, ex, ey, op) => `<g transform="translate(${ex},${ey}) rotate(${ang}) scale(${k.toFixed(3)})" opacity="${op}"><path d="M0,-26 L4,-21 L4,-6 L28,8 L28,12 L4,6 L3,22 L10,27 L10,29 L0,27 L-10,29 L-10,27 L-3,22 L-4,6 L-28,12 L-28,8 L-4,-6 L-4,-21 Z" fill="#3a3c40" stroke="#eef0f2" stroke-width="${(1.2 / k).toFixed(2)}"/></g>`;
  const endAng = p.dir === 'STRAIGHT' ? 0 : (p.dir === 'LEFT' ? 1 : -1) * p.angle;
  const v = pb.v;
  const map = pbMapLayer(+v['PLANE LATITUDE'] || 0, +v['PLANE LONGITUDE'] || 0, +v['PLANE HEADING DEGREES TRUE'] || 0, cx, cy, scale);
  const label = pb.map.layers ? pb.map.icao : pb.map.state === 'loading' ? 'LOADING AIRPORT...' : 'NO AIRPORT DATA';
  return `<svg viewBox="0 0 ${W} ${H}" class="pb-path"><defs><clipPath id="pbclip"><rect x="0" y="0" width="${W}" height="${H}" rx="12"/></clipPath></defs><g clip-path="url(#pbclip)"><rect x="0" y="0" width="${W}" height="${H}" fill="#1f2124"/>${map}`
    + `<polyline points="${path}" fill="none" stroke="#ff4fd8" stroke-width="3" stroke-dasharray="7 5"/>`
    + plane(endAng, end[0], end[1], 0.55) + plane(0, cx, cy, 1)
    + `<text x="12" y="${H - 12}" fill="#9da3aa" font-size="11" font-weight="700" font-family="Manrope, Segoe UI, sans-serif">${label}  \u2022  50 m</text><line x1="${W - 12 - 50 * scale}" y1="${H - 16}" x2="${W - 12}" y2="${H - 16}" stroke="#9da3aa" stroke-width="2"/></g></svg>`;
}

function pbPad() {
  const pad = el('div', { class: 'pb-pad' });
  const knob = el('div', { class: 'pb-knob' });
  pad.append(el('span', { class: 'pb-pl t', text: 'FWD' }), el('span', { class: 'pb-pl b', text: 'BACK' }), el('span', { class: 'pb-pl l', text: 'NOSE L' }), el('span', { class: 'pb-pl r', text: 'NOSE R' }), knob);
  const place = () => { knob.style.left = `${50 + pb.heading * 42}%`; knob.style.top = `${50 - pb.speed * 42}%`; };
  place();
  let drag = false;
  const move = (e) => {
    const r = pad.getBoundingClientRect();
    const nx = Math.max(-1, Math.min(1, ((e.clientX - r.left) / r.width - 0.5) / 0.42));
    const ny = Math.max(-1, Math.min(1, -((e.clientY - r.top) / r.height - 0.5) / 0.42));
    pb.auto = null;
    const speed = Math.abs(ny) < 0.06 ? 0 : ny * 0.6;
    const heading = Math.abs(nx) < 0.06 ? 0 : (speed < 0 ? -nx : nx);
    pbSend(speed, heading);
    place();
  };
  pad.addEventListener('pointerdown', (e) => { drag = true; pad.setPointerCapture(e.pointerId); move(e); });
  pad.addEventListener('pointermove', (e) => { if (drag) move(e); });
  const up = () => { if (!drag) return; drag = false; pbSend(0, 0); place(); };
  pad.addEventListener('pointerup', up);
  pad.addEventListener('pointercancel', up);
  return pad;
}

function pbOpt(key, opts, label) {
  const row = el('div', { class: 'pc-seg' });
  for (const o of opts) {
    const b = el('button', { type: 'button', class: 'pc-opt' + (String(pb.plan[key]) === String(o) ? ' on' : ''), text: typeof o === 'number' ? (key === 'angle' ? o + '°' : o + ' m') : o });
    b.addEventListener('click', () => { pb.plan[key] = o; pbRender(); });
    row.append(b);
  }
  return el('div', { class: 'pc-f' }, el('span', { class: 'pc-l', text: label }), row);
}

function pbTop() {
  const v = pb.v;
  const attached = !!+v['PUSHBACK ATTACHED'];
  const brake = +v.A32NX_PARK_BRAKE_LEVER_POS >= 0.5;
  const chip = (k, val, cls) => el('div', { class: 'pb-chip' }, el('span', { class: 'pb-ck', text: k }), el('span', { class: 'pb-cv ' + (cls || ''), text: val }));
  const status = el('div', { class: 'pb-status' },
    chip('TUG', attached ? 'CONNECTED' : 'NOT CONNECTED', attached ? 'pc-good' : ''),
    chip('PARK BRK', brake ? 'SET' : 'RELEASED', brake ? 'pc-bad' : 'pc-good'),
    chip('GS', `${Math.abs(+v['GROUND VELOCITY'] || 0).toFixed(1)} kt`),
    chip('HDG', `${Math.round(+v['PLANE HEADING DEGREES TRUE'] || 0).toString().padStart(3, '0')}°`),
    chip('MODE', pb.auto ? `AUTO ${pb.auto.i + 1}/${pb.auto.steps.length}` : pb.speed ? 'MANUAL' : 'IDLE', pb.auto ? 'pc-good' : ''));
  const tug = el('button', { type: 'button', class: 'pc-btn' + (attached ? '' : ' pc-go'), text: attached ? 'RELEASE TUG' : 'CALL TUG' });
  tug.addEventListener('click', () => { pbStop(); pbTug(); });
  const stop = el('button', { type: 'button', class: 'pc-btn pb-stop', text: 'STOP' });
  stop.addEventListener('click', pbStop);
  return el('div', { class: 'pb-top' }, status, el('div', { class: 'pb-btns' }, tug, stop));
}

function pbGoState() {
  const go = document.getElementById('pb-go');
  if (!go) return;
  const attached = !!+pb.v['PUSHBACK ATTACHED'];
  const brake = +pb.v.A32NX_PARK_BRAKE_LEVER_POS >= 0.5;
  go.disabled = !attached || brake || !!pb.auto;
  go.textContent = pb.auto ? 'PUSHING...' : !attached ? 'CALL THE TUG FIRST' : brake ? 'RELEASE PARK BRAKE' : 'START PUSHBACK';
  const note = document.getElementById('pb-note');
  if (note) note.textContent = brake && attached ? 'Release the parking brake to move.' : 'Drag to push or pull and steer; let go to stop.';
}

function pbRender() {
  const c = document.getElementById('pb-content');
  if (!c) return;
  c.innerHTML = '';
  const manual = el('div', { class: 'pc-card' }, el('div', { class: 'pc-h', text: 'MANUAL' }), pbPad(), el('div', { class: 'pb-note', id: 'pb-note' }));
  const auto = el('div', { class: 'pc-card' }, el('div', { class: 'pc-h', text: 'AUTOMATIC PUSHBACK' }),
    el('div', { class: 'pb-auto' },
      el('div', {}, pbOpt('dir', ['LEFT', 'STRAIGHT', 'RIGHT'], 'Tail'), pbOpt('angle', [45, 90, 135, 180], 'Turn'), pbOpt('after', [0, 15, 30, 60], 'Push after the turn'), pbOpt('pull', [0, 10, 20, 40], 'Pull forward'), pbOpt('pace', ['SLOW', 'NORMAL', 'FAST'], 'Speed')),
      el('div', { class: 'pb-preview' })));
  auto.querySelector('.pb-preview').innerHTML = pbPathSvg();
  const go = el('button', { type: 'button', class: 'pc-btn pc-go', id: 'pb-go' });
  go.addEventListener('click', () => { pbStartAuto(); pbGoState(); });
  auto.append(el('div', { class: 'pc-actions' }, go));
  c.append(pbTop(), el('div', { class: 'pb-grid' }, manual, auto));
  if (typeof gcCard === 'function') c.append(gcCard());
  pbGoState();
}

async function pbPoll() {
  const tab = document.getElementById('tab-pushback');
  const shown = tab && tab.classList.contains('active');
  if (!shown && !pb.auto) { clearInterval(pb.timer); pb.timer = null; return; }
  try { pb.v = (await studyGet('/vars?names=' + PB_VARS.map(encodeURIComponent).join(','))).vars || {}; } catch { pb.v = {}; }
  pbAutoTick();
  if (!shown) return;
  if (!pb.map.state) pbLoadMap();
  const top = document.querySelector('#pb-content .pb-top');
  if (top) top.replaceWith(pbTop());
  pbGoState();
}

function pushbackOnShow() {
  if (!pb.paceSet) {
    pb.paceSet = true;
    const pace = typeof settings === 'object' && settings ? settings['xphfbw.pushbackSpeed'] : null;
    if (['SLOW', 'NORMAL', 'FAST'].includes(pace)) pb.plan.pace = pace;
  }
  pbRender();
  if (!pb.timer) pb.timer = setInterval(pbPoll, 200);
  pbPoll();
}

(function () {
  const s = document.createElement('style');
  s.textContent = `
.pb-top{display:flex;justify-content:space-between;align-items:center;gap:12px;margin:0 0 14px;flex-wrap:wrap;}
.pb-status{display:flex;gap:10px;flex-wrap:wrap;}
.pb-chip{background:#26282c;border:1px solid rgba(255,255,255,0.08);border-radius:10px;padding:8px 14px;display:flex;flex-direction:column;gap:2px;min-width:96px;}
.pb-ck{font:800 11px 'Manrope','Segoe UI',sans-serif;color:#9da3aa;letter-spacing:0.08em;}
.pb-cv{font:800 16px 'Manrope','Segoe UI',sans-serif;color:#fff;}
.pb-btns{display:flex;gap:10px;}
button.pb-stop{background:#b3263a !important;border-color:transparent !important;min-width:110px;}
.pb-grid{display:grid;grid-template-columns:0.62fr 1.38fr;gap:16px;align-items:start;}
.pb-pad{position:relative;width:100%;aspect-ratio:1/1;max-height:520px;margin:0 auto;border-radius:18px;background:radial-gradient(circle at 50% 50%,#30333a 0,#202226 70%);border:1px solid rgba(255,255,255,0.1);touch-action:none;cursor:grab;}
.pb-pad::before,.pb-pad::after{content:'';position:absolute;background:rgba(255,255,255,0.12);}
.pb-pad::before{left:50%;top:8%;bottom:8%;width:1px;}
.pb-pad::after{top:50%;left:8%;right:8%;height:1px;}
.pb-knob{position:absolute;width:64px;height:64px;margin:-32px 0 0 -32px;border-radius:50%;background:linear-gradient(135deg,#f0703a,#ff3f7a);box-shadow:0 6px 18px rgba(0,0,0,0.5);}
.pb-pl{position:absolute;font:800 12px 'Manrope','Segoe UI',sans-serif;color:#9da3aa;letter-spacing:0.08em;}
.pb-pl.t{top:10px;left:50%;transform:translateX(-50%);}
.pb-pl.b{bottom:10px;left:50%;transform:translateX(-50%);}
.pb-pl.l{left:12px;top:50%;transform:translateY(-50%);}
.pb-pl.r{right:12px;top:50%;transform:translateY(-50%);}
.pb-note{font:500 13.5px 'Manrope','Segoe UI',sans-serif;color:#9da3aa;margin-top:10px;text-align:center;}
.pb-auto{display:grid;grid-template-columns:1fr 420px;gap:16px;align-items:start;}
.pb-path{width:420px;height:420px;display:block;}
`;
  document.head.append(s);
})();

const FCTL_POWER_COL = { G: '#3fd27f', Y: '#ffd23f', E: '#4fb7ff' };
const FCTL_NAMES = { SLATS: 'SLATS', FLAPS: 'FLAPS', THS: 'THS', RUD_UP: 'UPPER RUDDER', RUD_LO: 'LOWER RUDDER' };

function fctlName(key) {
  if (FCTL_NAMES[key]) return FCTL_NAMES[key];
  let m = key.match(/^SPLR_([LR])(\d)$/);
  if (m) return `${m[1]} SPLR ${m[2]}`;
  m = key.match(/^AIL_([LR])_(IN|MID|OUT)$/);
  if (m) return `${m[1]} ${m[2]} AIL`;
  m = key.match(/^ELEV_([LR])_(IN|OUT)$/);
  if (m) return `${m[1]} ${m[2]} ELEV`;
  return key;
}

function fctlPlanView(m, title) {
  const W = 1000, H = 900, s = 9.0, x0 = 500, y0 = 64;
  const p = (x, z) => [x0 + x * s, y0 + (z + 35.4) * s];
  const parts = [], fg = [];
  FCTL_COMPUTERS.forEach((c, i) => {
    const st = m.get(c) || 'live';
    fg.push(nsBox(20 + i * 88, 14, 80, 28, c, st, { size: 10, r: 7, tag: m.get(c) === 'fail' ? 'FAILED' : '' }));
  });
  const law = fctlLaw(m);
  if (law) fg.push(exT(W - 20, 68, law, { size: 15, anchor: 'end', fill: NS.orange }));
  parts.push(exPath(DX_DATA.sil.top.map(([x, z]) => p(x, z)), { fill: 'rgba(255,255,255,0.03)', stroke: '#4a4f56', sw: 1.2 }));
  const fillOf = (st) => (st === 'fail' ? 'rgba(255,61,99,0.38)' : st === 'warn' ? 'rgba(255,176,0,0.25)' : 'rgba(63,210,127,0.12)');
  const drawPoly = (pts, st, proj = p) => `<path d="M${pts.map(([a, b]) => proj(a, b).map((v) => v.toFixed(1)).join(',')).join(' L')} Z" fill="${fillOf(st)}" stroke="${nsStroke(st)}" stroke-width="${st === 'live' ? 1.2 : 2}" stroke-linejoin="round"/>`;
  const dots = (pts, power, st, proj = p) => {
    const sp = pts.map(([a, b]) => proj(a, b));
    const cx = sp.reduce((a, q) => a + q[0], 0) / sp.length, cy = sp.reduce((a, q) => a + q[1], 0) / sp.length;
    const w = Math.max(...sp.map((q) => q[0])) - Math.min(...sp.map((q) => q[0]));
    const r = Math.max(1.6, Math.min(3, (w - 4) / (power.length * 2.6)));
    power.forEach((pw, j) => fg.push(`<circle cx="${(cx + (j - (power.length - 1) / 2) * r * 2.6).toFixed(1)}" cy="${cy.toFixed(1)}" r="${r.toFixed(1)}" fill="${st === 'fail' ? '#fff' : FCTL_POWER_COL[pw]}"/>`));
    return [cx, cy];
  };
  const affected = [];
  const surf = (key, pts, power, label, labelOff = 0) => {
    const ctl = fctlControl(key, m);
    const st = m.get(key) || ctl.st;
    fg.push(drawPoly(pts, st));
    const [cx, cy] = dots(pts, power, st);
    if (label) fg.push(exT(cx, cy + labelOff, label, { size: 8, fill: NS.greyText, weight: 600 }));
    const tag = m.get(key) === 'fail' ? 'FAILED' : m.get(key) ? 'DEGRADED' : ctl.tag;
    if (tag) affected.push([key, tag, st]);
  };
  const te = (x) => EX_TE[0][1] + ((x - EX_TE[0][0]) * (EX_TE[1][1] - EX_TE[0][1])) / (EX_TE[1][0] - EX_TE[0][0]);
  const le = (x) => EX_LE[0][1] + ((x - EX_LE[0][0]) * (EX_LE[1][1] - EX_LE[0][1])) / (EX_LE[1][0] - EX_LE[0][0]);
  const strip = (k, xa, xb, f, d0, d1) => [[k * xa, f(xa) + d0], [k * xb, f(xb) + d0], [k * xb, f(xb) + d1], [k * xa, f(xa) + d1]];
  for (const side of ['L', 'R']) {
    const k = side === 'R' ? 1 : -1;
    surf('FLAPS', strip(k, 5.4, 26.4, te, -2.6, -0.1), ['G', 'Y'], '');
    surf('SLATS', strip(k, 7.0, 38.6, le, 0.05, 1.3), ['G', 'E'], '');
    for (let n = 1; n <= 8; n++) surf(`SPLR_${side}${n}`, fctlSurfacePoly(`l_spoiler${n}_key`, side), n === 6 ? ['G', 'E'] : [n % 2 ? 'Y' : 'G'], String(n), -9);
    ['IN', 'MID', 'OUT'].forEach((pos, i) => surf(`AIL_${side}_${pos}`, fctlSurfacePoly(`l_aileron${i + 1}_percent_key`, side), FCTL_POWER['AIL_' + pos].map((q) => q[0]), '', 0));
    const thsPts = [[k * 1.0, 23.0], [k * 15.2, 34.5], [k * 13.7, 34.67], [k * 5.6, 31.6], [k * 1.0, 29.73]];
    const thsSt = m.get('THS') || fctlControl('THS', m).st;
    fg.push(drawPoly(thsPts, thsSt));
    if (side === 'L') {
      dots([[-4, 27.6], [-6, 27.6], [-6, 28.6], [-4, 28.6]], ['G', 'Y', 'E'], thsSt);
      const tt = fctlControl('THS', m).tag;
      if (tt || m.get('THS')) affected.push(['THS', m.get('THS') === 'fail' ? 'FAILED' : tt, thsSt]);
    }
    surf(`ELEV_${side}_IN`, fctlSurfacePoly('l_inner_elevator_deflection', side), FCTL_POWER[`ELEV_${side}_IN`].map((q) => q[0]), '');
    surf(`ELEV_${side}_OUT`, fctlSurfacePoly('l_outer_elevator_deflection', side), FCTL_POWER[`ELEV_${side}_OUT`].map((q) => q[0]), '');
  }
  const lbl = (x, z, t) => { const [a, b] = p(x, z); fg.push(exT(a, b, t, { size: 9, fill: NS.greyText, weight: 600 })); };
  lbl(-24, le(24) - 1.6, 'SLATS'); lbl(-16, te(16) + 1.6, 'FLAPS'); lbl(-31.5, te(31.5) + 1.9, 'AILERONS'); lbl(-15.5, 0.0, 'SPOILERS'); lbl(-9, 27.0, 'THS'); lbl(-8, 36.4, 'ELEVATORS');
  const fx = 790, fy = 680, fs = 9.5;
  const fp = (z, y) => [fx + (z - 24) * fs, fy + (21 - y) * fs];
  fg.push(`<rect x="${fx - 10}" y="${fy - 22}" width="${16 * fs}" height="${20 * fs}" rx="10" fill="#1f2226" stroke="#3a3e44"/>`);
  fg.push(exT(fx + 8 * fs - 10, fy - 6, 'FIN · SIDE VIEW', { size: 9, fill: NS.greyText }));
  const clip = `<clipPath id="fctlFin"><rect x="${fx - 10}" y="${fy - 22}" width="${16 * fs}" height="${20 * fs}" rx="10"/></clipPath>`;
  fg.push(clip, `<g clip-path="url(#fctlFin)">${exPath(DX_DATA.sil.side.map(([z, y]) => fp(z, y)), { fill: 'rgba(255,255,255,0.03)', stroke: '#4a4f56', sw: 1.2 })}</g>`);
  const teZ = (y) => 33.18 + (y - 10.68) * 0.446;
  [['RUD_UP', 10.6, 20.0, 3.4], ['RUD_LO', 5.6, 10.5, 3.3]].forEach(([key, ya, yb, chord]) => {
    const pts = [[teZ(ya) - chord, ya], [teZ(ya), ya], [teZ(yb), yb], [teZ(yb) - chord, yb]];
    const ctl = fctlControl(key, m);
    const st = m.get(key) || ctl.st;
    fg.push(drawPoly(pts, st, fp));
    dots(pts, FCTL_POWER[key].map((q) => q[0]), st, fp);
    const tag = m.get(key) === 'fail' ? 'FAILED' : ctl.tag;
    if (tag) affected.push([key, tag, st]);
  });
  if (affected.length) {
    fg.push(exT(24, 84, 'AFFECTED', { size: 10, anchor: 'start', fill: NS.greyText }));
    affected.forEach(([key, tag, st], i) => {
      const col = Math.floor(i / 14), row = i % 14;
      fg.push(exT(24 + col * 150, 104 + row * 16, `${fctlName(key)}  ${tag}`, { size: 9, anchor: 'start', fill: nsStroke(st), weight: 600 }));
    });
  }
  [['G', 'GREEN HYD'], ['Y', 'YELLOW HYD'], ['E', 'ELECTRIC (EHA / EBHA)']].forEach(([k, label], i) => {
    fg.push(`<circle cx="30" cy="${H - 64 + i * 18}" r="5" fill="${FCTL_POWER_COL[k]}"/>`, exT(44, H - 60 + i * 18, label, { size: 10, anchor: 'start', fill: NS.greyText, weight: 600 }));
  });
  if (title) fg.push(exT(W - 16, H - 10, title, { anchor: 'end', size: 12, fill: NS.greyText, weight: 500 }));
  return nsSvg(W, H, [...parts, ...fg]);
}

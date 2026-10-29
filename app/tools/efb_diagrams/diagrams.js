const EX = {
  ink: '#e6e6e6', dim: 'rgba(230,230,230,0.45)', hot: '#e040fb', fire: '#ff4b4b', green: '#3fd27f', yellow: '#ffd23f',
  mono: "'JetBrains Mono','Cascadia Mono','Consolas',monospace",
};
const EX_NOSE = -35.21;
const EX_LE = [[4.5, -14.99], [38.8, 10.98]];
const EX_TE = [[4, 2.0], [39.5, 15.4]];

function exEsc(s) { return String(s).replace(/&/g, '&amp;').replace(/</g, '&lt;'); }
function exT(x, y, s, o = {}) {
  return `<text x="${x}" y="${y}" fill="${o.fill || EX.ink}" font-size="${o.size || 13}" font-weight="${o.weight || 700}" text-anchor="${o.anchor || 'middle'}" font-family="${EX.mono}"${o.rot ? ` transform="rotate(${o.rot} ${x} ${y})"` : ''}>${exEsc(s)}</text>`;
}
function exPath(pts, o = {}) {
  const d = pts.map((p, i) => (i ? 'L' : 'M') + p[0].toFixed(1) + ',' + p[1].toFixed(1)).join('') + (o.open ? '' : 'Z');
  return `<path d="${d}" fill="${o.fill || 'none'}" stroke="${o.stroke || EX.ink}" stroke-width="${o.sw || 1.6}" stroke-linejoin="round" stroke-linecap="round"/>`;
}
function exBox(x, y, w, h, label, on, o = {}) {
  const c = on ? (o.hot || EX.hot) : 'rgba(255,255,255,0.05)';
  return `<rect x="${x}" y="${y}" width="${w}" height="${h}" fill="${c}" stroke="${o.stroke || EX.ink}" stroke-width="1.6"/>` +
    (label ? exT(x + w / 2, y + h / 2 + 5, label, { size: o.size || 13, fill: on ? '#fff' : EX.ink }) : '');
}
function exLine(pts, on, o = {}) {
  return exPath(pts, { open: true, stroke: on ? (o.hot || EX.hot) : (o.stroke || EX.ink), sw: on ? 4 : (o.sw || 2) });
}
function exPump(x, y, on, label) {
  return `<circle cx="${x}" cy="${y}" r="15" fill="${on ? EX.hot : 'rgba(255,255,255,0.05)'}" stroke="${EX.ink}" stroke-width="1.6"/>` +
    `<path d="M${x - 8},${y + 6} L${x},${y - 9} L${x + 8},${y + 6}" fill="none" stroke="${on ? '#fff' : EX.ink}" stroke-width="1.6"/>` +
    (label ? exT(x, y + 32, label, { size: 11.5, fill: on ? EX.hot : EX.ink }) : '');
}
function exValve(x, y, on, label, vertical) {
  const bar = vertical ? `M${x},${y - 11} L${x},${y + 11}` : `M${x - 11},${y} L${x + 11},${y}`;
  return `<circle cx="${x}" cy="${y}" r="13" fill="${on ? EX.hot : '#1a1d21'}" stroke="${EX.ink}" stroke-width="1.6"/><path d="${bar}" stroke="${on ? '#fff' : EX.ink}" stroke-width="2"/>` +
    (label ? exT(x, y + 30, label, { size: 11.5, fill: on ? EX.hot : EX.ink }) : '');
}
function exSvg(w, h, parts) {
  const d = el('div', { class: 'qd-svg' });
  d.innerHTML = `<svg viewBox="0 0 ${w} ${h}" class="qd-phases" font-family="${EX.mono}">${parts.join('')}</svg>`;
  return d;
}

function exPlanView(hl, fire) {
  const W = 980, H = 640, s = 7.9, x0 = 490, y0 = 34;
  const p = (x, z) => [x0 + x * s, y0 + (z + 35.4) * s];
  const hot = fire ? EX.fire : EX.hot;
  const parts = [exPath(DX_DATA.sil.top.map(([x, z]) => p(x, z)), { fill: 'rgba(255,255,255,0.03)', sw: 1.6 })];
  const engs = DX_DATA.parts.top_engines.map((loop) => ({ loop, cx: loop.reduce((a, q) => a + q[0], 0) / loop.length })).sort((a, b) => a.cx - b.cx);
  engs.forEach((e, i) => {
    const on = hl.has('ENG' + (i + 1));
    parts.push(exPath(e.loop.map(([x, z]) => p(x, z)), { fill: on ? hot : 'rgba(255,255,255,0.04)', sw: 1.6 }));
    const [lx, ly] = p(e.cx, -16);
    parts.push(exT(lx, ly, String(i + 1), { size: 15, fill: on ? hot : EX.ink }));
  });
  const S = DX_DATA.model.surfaces;
  const rect = (b, side, on) => {
    const k = side === 'R' ? 1 : -1;
    const [ax, ay] = p(k * Math.abs(b[0]), b[2]), [bx, by] = p(k * Math.abs(b[1]), b[3]);
    return `<rect x="${Math.min(ax, bx)}" y="${Math.min(ay, by)}" width="${Math.abs(bx - ax)}" height="${Math.abs(by - ay)}" fill="${on ? hot : 'rgba(255,255,255,0.04)'}" stroke="${on ? hot : EX.dim}" stroke-width="1.2"/>`;
  };
  for (const side of ['L', 'R']) {
    for (let n = 1; n <= 8; n++) parts.push(rect(S[`l_spoiler${n}_key`], side, hl.has(`SPLR${side}${n}`) || hl.has(`SPLR${n}`) || hl.has('SPLRALL')));
    ['INR', 'MID', 'OUTR'].forEach((part, i) => parts.push(rect(S[`l_aileron${i + 1}_percent_key`], side, hl.has(`AIL${side}${part}`) || hl.has(`AIL${part}`) || hl.has('AILALL') || hl.has('AIL' + side))));
    parts.push(rect(S.l_inner_elevator_deflection, side, hl.has(`ELEV${side}INR`) || hl.has('ELEVALL') || hl.has('ELEV' + side)));
    parts.push(rect(S.l_outer_elevator_deflection, side, hl.has(`ELEV${side}OUTR`) || hl.has('ELEVALL') || hl.has('ELEV' + side)));
    const k = side === 'R' ? 1 : -1;
    const le = EX_LE.map(([x, z]) => p(k * x, z));
    parts.push(exLine(le, hl.has('SLAT') || hl.has('SLAT' + side), { stroke: 'rgba(0,0,0,0)' }));
    const te = [[4, 2.0], [26, 9.3]].map(([x, z]) => p(k * x, z + 0.6));
    parts.push(exLine(te, hl.has('FLAP') || hl.has('FLAP' + side), { stroke: 'rgba(0,0,0,0)' }));
  }
  const [rx, ry] = p(0, 30.2), [, ry2] = p(0, 37.8);
  const rud = hl.has('RUDUPPER') || hl.has('RUDLOWER') || hl.has('RUD');
  parts.push(`<rect x="${rx - 3.5}" y="${ry}" width="7" height="${ry2 - ry}" fill="${rud ? hot : 'rgba(255,255,255,0.1)'}" stroke="${rud ? hot : EX.dim}" stroke-width="1.2"/>`);
  if (hl.has('THS')) {
    const [a, b] = p(-15, 32), [c, d] = p(15, 34.5);
    parts.push(`<rect x="${a}" y="${b}" width="${c - a}" height="${d - b}" fill="none" stroke="${hot}" stroke-width="3"/>`);
  }
  const apu = hl.has('APU');
  const [ax, ay] = p(0, 36.6);
  parts.push(`<rect x="${ax - 9}" y="${ay - 12}" width="18" height="16" fill="${apu ? hot : 'rgba(255,255,255,0.06)'}" stroke="${apu ? hot : EX.dim}" stroke-width="1.2"/>`);
  if (apu) parts.push(exT(ax + 40, ay, 'APU', { fill: hot }));
  const gear = [['NLG', 0, -29.4, 1, 2], ['WLGL', -5.95, -1.1, 2, 2], ['WLGR', 5.95, -1.1, 2, 2], ['BLGL', -2.85, 2.6, 2, 3], ['BLGR', 2.85, 2.6, 2, 3]];
  for (const [id, x, z, cols, rows] of gear) {
    const on = hl.has(id) || hl.has(id.slice(0, 3)) || hl.has('GEAR') || (id[0] !== 'N' && hl.has('MLG'));
    for (let c = 0; c < cols; c++) for (let r = 0; r < rows; r++) {
      const [gx, gy] = p(x + (c - (cols - 1) / 2) * 0.9, z + (r - (rows - 1) / 2) * 1.7);
      parts.push(`<rect x="${gx - 3}" y="${gy - 5.5}" width="6" height="11" rx="1.5" fill="${on ? hot : 'rgba(255,255,255,0.15)'}" stroke="${on ? hot : EX.dim}" stroke-width="1"/>`);
    }
  }
  if (hl.has('LGL') || hl.has('LGR')) {
    for (const side of ['L', 'R']) if (hl.has('LG' + side)) {
      for (const [id] of gear.filter((g) => g[0].endsWith(side))) {
        const g = gear.find((q) => q[0] === id);
        const [gx, gy] = p(g[1], g[2]);
        parts.push(`<circle cx="${gx}" cy="${gy}" r="18" fill="none" stroke="${hot}" stroke-width="2.5"/>`);
      }
    }
  }
  parts.push(exT(14, 22, 'L', { anchor: 'start', size: 15 }), exT(W - 14, 22, 'R', { anchor: 'end', size: 15 }));
  return exSvg(W, H, parts);
}

const EX_DOORS = [['M1', 'ANIM_DOOR_M1L'], ['M2', 'ANIM_DOOR_M2L'], ['M3', 'ANIM_DOOR_M3L'], ['M4', 'ANIM_DOOR_M4L'], ['M5', 'ANIM_DOOR_M5L'], ['U1', 'U1L'], ['U2', 'U2L'], ['U3', 'U3L']];
function exPart(name) {
  const P = DX_DATA.model.parts;
  const k = Object.keys(P).find((x) => x.includes(name));
  return k ? P[k] : null;
}

function exSideView(hl, smoke) {
  const W = 980, H = 380, s = 12.6, x0 = 26, y0 = 40;
  const p = (z, y) => [x0 + (z + 35.4) * s, y0 + (20.4 - y) * s];
  const hot = smoke ? EX.fire : EX.hot;
  const parts = [exPath(DX_DATA.sil.side.map(([z, y]) => p(z, y)), { fill: 'rgba(255,255,255,0.03)', sw: 1.6 })];
  for (const [n, key] of EX_DOORS) {
    const b = exPart(key);
    if (!b) continue;
    const on = hl.has('D' + n) || hl.has('DALL') || (n[0] === 'M' && hl.has('DMAIN')) || (n[0] === 'U' && hl.has('DUPPER'));
    const [ax, ay] = p(b[0][2], b[1][1]), [bx, by] = p(b[1][2], b[0][1]);
    parts.push(`<rect x="${ax}" y="${ay}" width="${bx - ax}" height="${by - ay}" rx="5" fill="${on ? hot : 'rgba(255,255,255,0.04)'}" stroke="${on ? hot : EX.ink}" stroke-width="1.6"/>`);
    parts.push(exT((ax + bx) / 2, n[0] === 'U' ? ay - 10 : by + 22, n, { size: 13, fill: on ? hot : EX.ink }));
  }
  const zone = (id, z0, z1, label) => {
    const on = hl.has(id);
    const [a, t] = p(EX_NOSE + z0, 0.6), [b, bt] = p(EX_NOSE + z1, -2.4);
    parts.push(`<rect x="${a}" y="${t}" width="${b - a}" height="${bt - t}" fill="${on ? hot : 'none'}" fill-opacity="${on ? 0.85 : 0}" stroke="${on ? hot : EX.dim}" stroke-width="1.4" stroke-dasharray="${on ? '' : '6 4'}"/>`);
    parts.push(exT((a + b) / 2, bt + 40, label, { size: 12, fill: on ? hot : EX.dim }));
  };
  zone('CFWD', 10.5, 27.9, 'FWD CARGO');
  zone('CAFT', 37.0, 46.7, 'AFT CARGO');
  zone('CBULK', 47.3, 50.1, 'BULK');
  const av = hl.has('AVNCS');
  const [a1, t1] = p(EX_NOSE + 4, 0.8), [b1, bt1] = p(EX_NOSE + 9.5, -2.2);
  parts.push(`<rect x="${a1}" y="${t1}" width="${b1 - a1}" height="${bt1 - t1}" fill="${av ? hot : 'none'}" stroke="${av ? hot : EX.dim}" stroke-width="1.4" stroke-dasharray="${av ? '' : '6 4'}"/>`);
  parts.push(exT((a1 + b1) / 2, bt1 + 40, 'AVNCS', { size: 12, fill: av ? hot : EX.dim }));
  return exSvg(W, H, parts);
}

function exHyd(hl) {
  const W = 980, H = 430, parts = [];
  const sys = (x, c, name, engs) => {
    const on = hl.has(name);
    parts.push(exT(x + 200, 34, name === 'G' ? 'GREEN' : 'YELLOW', { size: 17, fill: c }));
    engs.forEach((e, i) => {
      ['A', 'B'].forEach((ab, j) => {
        const px = x + 40 + i * 180 + j * 70;
        const id = `EDP${e}${ab}`;
        parts.push(exPump(px, 110, hl.has(id), `${e}${ab}`));
        parts.push(exLine([[px, 125], [px, 230]], false, { stroke: c }));
      });
      parts.push(exT(x + 75 + i * 180, 64, `ENG ${e}`, { size: 13 }));
    });
    ['A', 'B'].forEach((ab, j) => {
      const px = x + 320 + j * 60;
      parts.push(exPump(px, 300, hl.has(`EP${name}${ab}`), `ELEC ${ab}`));
      parts.push(exLine([[px, 285], [px, 230]], false, { stroke: c }));
    });
    parts.push(exLine([[x + 20, 230], [x + 400, 230]], on, { stroke: c, sw: 5 }));
    parts.push(exBox(x + 40, 270, 120, 46, 'RSVR', hl.has('RSVR' + name)));
    parts.push(exT(x + 210, 390, '5000 PSI', { size: 13, fill: c }));
  };
  sys(40, EX.green, 'G', [1, 2]);
  sys(530, EX.yellow, 'Y', [3, 4]);
  return exSvg(W, H, parts);
}

function exElec(hl) {
  const W = 980, H = 470, parts = [];
  for (let i = 1; i <= 4; i++) {
    const x = 30 + (i - 1) * 240;
    parts.push(exBox(x, 30, 100, 44, `GEN ${i}`, hl.has('GEN' + i)));
    parts.push(exBox(x + 112, 30, 92, 44, `EXT ${i}`, hl.has('EXT' + i)));
    parts.push(exLine([[x + 50, 74], [x + 50, 130]], false));
    parts.push(exLine([[x + 158, 74], [x + 158, 110], [x + 50, 110]], false));
    parts.push(exBox(x, 130, 204, 44, `AC ${i}`, hl.has('AC' + i)));
  }
  parts.push(exBox(300, 210, 140, 44, 'APU GEN A', hl.has('APUGENA')), exBox(540, 210, 140, 44, 'APU GEN B', hl.has('APUGENB')));
  parts.push(exLine([[30, 190], [950, 190]], hl.has('BUSTIE'), { sw: 2 }));
  parts.push(exBox(60, 280, 180, 44, 'AC ESS', hl.has('ACESS')), exBox(400, 280, 180, 44, 'AC EHA', hl.has('ACEHA')), exBox(740, 280, 180, 44, 'EMER GEN', hl.has('RAT')));
  [['TR 1', 'DC 1', 'TR1', 'DC1'], ['TR 2', 'DC 2', 'TR2', 'DC2'], ['ESS TR', 'DC ESS', 'TRESS', 'DCESS'], ['APU TR', 'DC APU', 'TRAPU', 'DCAPU']].forEach(([tr, dc, a, b], i) => {
    const x = 30 + i * 240;
    parts.push(exBox(x, 350, 96, 40, tr, hl.has(a)), exBox(x + 108, 350, 96, 40, dc, hl.has(b)));
    parts.push(exLine([[x + 96, 370], [x + 108, 370]], false));
    const bat = ['BAT 1', 'BAT 2', 'ESS BAT', 'APU BAT'][i], bid = ['BAT1', 'BAT2', 'BATESS', 'BATAPU'][i];
    parts.push(exBox(x + 108, 410, 96, 36, bat, hl.has(bid), { size: 12 }));
  });
  return exSvg(W, H, parts);
}

function exFuel(hl) {
  const W = 980, H = 460, parts = [];
  const tanks = [['OUTR', 'L', 'OUTRL'], ['FEED 1', 'L', 'FEED1'], ['MID', 'L', 'MIDL'], ['INR', 'L', 'INRL'], ['FEED 2', 'L', 'FEED2'], ['FEED 3', 'R', 'FEED3'], ['INR', 'R', 'INRR'], ['MID', 'R', 'MIDR'], ['FEED 4', 'R', 'FEED4'], ['OUTR', 'R', 'OUTRR']];
  const any = (id) => hl.has(id) || hl.has(id.replace(/[LR]$/, ''));
  tanks.forEach(([name, , id], i) => {
    const x = 18 + i * 95 + (i >= 5 ? 14 : 0);
    const y = 170 - Math.abs(i - 4.5) * 14;
    parts.push(exBox(x, y, 84, 92, '', any(id)));
    parts.push(exT(x + 42, y + 38, name, { size: 12.5, fill: any(id) ? '#fff' : EX.ink }));
    if (name.startsWith('FEED')) {
      const n = name.slice(-1);
      parts.push(exPump(x + 26, y + 70, hl.has('PUMPMAIN' + n), null), exPump(x + 60, y + 70, hl.has('PUMPSTBY' + n), null));
      parts.push(exLine([[x + 42, y], [x + 42, 60]], hl.has('ENGFEED' + n)));
      parts.push(exBox(x + 6, 24, 72, 36, `ENG ${n}`, hl.has('ENGFEED' + n), { size: 12.5 }));
      parts.push(exValve(x + 42, y - 36, hl.has('XFEED' + n), null, true));
    }
  });
  parts.push(exLine([[60, 300], [920, 300]], hl.has('XFR'), { sw: 2 }));
  parts.push(exBox(410, 340, 160, 70, 'TRIM', any('TRIM')));
  parts.push(exLine([[490, 300], [490, 340]], hl.has('TRIM'), { sw: 2 }));
  parts.push(exBox(780, 360, 120, 44, 'APU', hl.has('APUFEED'), { size: 12.5 }));
  parts.push(exBox(70, 360, 150, 44, 'JETTISON', hl.has('JETT'), { size: 12.5 }));
  return exSvg(W, H, parts);
}

function exBleed(hl) {
  const W = 980, H = 440, parts = [];
  for (let i = 1; i <= 4; i++) {
    const x = 80 + (i - 1) * 230;
    parts.push(exBox(x - 50, 24, 100, 46, `ENG ${i}`, hl.has('ENG' + i) && !hl.has('BLEED' + i)));
    parts.push(exValve(x, 120, hl.has('BLEED' + i), null, true));
    parts.push(exLine([[x, 70], [x, 107]], hl.has('BLEED' + i)), exLine([[x, 133], [x, 200]], hl.has('BLEED' + i)));
    parts.push(exT(x + 30, 125, 'BLEED', { size: 11.5, anchor: 'start', fill: hl.has('BLEED' + i) ? EX.hot : EX.dim }));
  }
  parts.push(exLine([[80, 200], [770, 200]], hl.has('DUCT'), { sw: 2.4 }));
  [[195, 'XBLEED L', 'XBLEEDL'], [425, 'XBLEED C', 'XBLEEDC'], [655, 'XBLEED R', 'XBLEEDR']].forEach(([x, n, id]) => parts.push(exValve(x, 200, hl.has(id), n)));
  parts.push(exBox(375, 260, 100, 44, 'APU', hl.has('APUBLEED')), exLine([[425, 260], [425, 213]], hl.has('APUBLEED')));
  [[160, 'PACK 1', 'PACK1'], [690, 'PACK 2', 'PACK2']].forEach(([x, n, id]) => {
    parts.push(exLine([[x + 40, 200], [x + 40, 330]], hl.has(id)), exBox(x, 330, 80, 44, n, hl.has(id), { size: 12.5 }));
  });
  parts.push(exBox(860, 300, 100, 44, 'HOT AIR', hl.has('HOTAIR'), { size: 12 }));
  return exSvg(W, H, parts);
}

function exHighlights(p) {
  const t = ' ' + p.t.toUpperCase().replace(/[^A-Z0-9/+ ]/g, ' ') + ' ';
  const s = (p.s || '').toUpperCase();
  const hl = new Set();
  const all = (re) => [...t.matchAll(re)];
  for (const m of all(/ENG (\d)(?:\s*\+\s*(\d))?(?:\s*\+\s*(\d))?(?:\s*\+\s*(\d))?/g)) for (const n of m.slice(1)) if (n) hl.add('ENG' + n);
  if (/ ENG 1\+2\+3\+4 | ALL ENG/.test(t)) [1, 2, 3, 4].forEach((n) => hl.add('ENG' + n));
  if (/\bAPU\b/.test(t)) hl.add('APU');
  for (const m of all(/(?:EDP|ENG)\s*(\d)\s*PUMP\s*([AB])/g)) hl.add('EDP' + m[1] + m[2]);
  for (const m of all(/ENG (\d) PUMP ([AB])/g)) hl.add('EDP' + m[1] + m[2]);
  for (const m of all(/\b(G|Y|GREEN|YELLOW)\b/g)) hl.add(m[1][0]);
  for (const m of all(/ELEC PUMP ([AB])/g)) { if (hl.has('G')) hl.add('EPG' + m[1]); if (hl.has('Y')) hl.add('EPY' + m[1]); }
  if (/RSVR/.test(t)) { if (hl.has('G')) hl.add('RSVRG'); if (hl.has('Y')) hl.add('RSVRY'); }
  for (const m of all(/(?:GEN|VFG)\s*(\d)/g)) hl.add('GEN' + m[1]);
  for (const m of all(/APU GEN ([AB])/g)) hl.add('APUGEN' + m[1]);
  for (const m of all(/EXT (?:PWR )?(\d)/g)) hl.add('EXT' + m[1]);
  for (const m of all(/AC (?:BUS )?(\d)/g)) hl.add('AC' + m[1]);
  if (/AC ESS/.test(t)) hl.add('ACESS');
  if (/AC EHA|247XP/.test(t)) hl.add('ACEHA');
  for (const m of all(/DC (?:BUS )?(\d)/g)) hl.add('DC' + m[1]);
  if (/DC ESS/.test(t)) hl.add('DCESS');
  if (/DC APU/.test(t)) hl.add('DCAPU');
  for (const m of all(/TR (\d)/g)) hl.add('TR' + m[1]);
  if (/ESS TR/.test(t)) hl.add('TRESS');
  if (/APU TR/.test(t)) hl.add('TRAPU');
  for (const m of all(/BAT (\d)/g)) hl.add('BAT' + m[1]);
  if (/ESS BAT/.test(t)) hl.add('BATESS');
  if (/APU BAT/.test(t)) hl.add('BATAPU');
  if (/RAT|EMER GEN|EMER CONFIG/.test(t)) hl.add('RAT');
  if (/BUS TIE/.test(t)) hl.add('BUSTIE');
  for (const m of all(/FEED (?:TK )?(\d)/g)) hl.add('FEED' + m[1]);
  for (const m of all(/COLLECTOR CELL (\d)/g)) hl.add('FEED' + m[1]);
  for (const m of all(/(?:L|LEFT) (OUTR|MID|INR)/g)) hl.add(m[1] + 'L');
  for (const m of all(/(?:R|RIGHT) (OUTR|MID|INR)/g)) hl.add(m[1] + 'R');
  for (const m of all(/ (OUTR|MID|INR) TK/g)) { hl.add(m[1] + 'L'); hl.add(m[1] + 'R'); }
  if (/TRIM TK|TRIM TANK/.test(t)) hl.add('TRIM');
  for (const m of all(/(MAIN|STBY) PUMP/g)) for (let n = 1; n <= 4; n++) if (hl.has('FEED' + n)) hl.add('PUMP' + m[1] + n);
  for (const m of all(/XFEED (\d)/g)) hl.add('XFEED' + m[1]);
  if (/XFR/.test(t)) hl.add('XFR');
  if (/JETT/.test(t)) hl.add('JETT');
  if (/APU .*FEED|APU FUEL/.test(t)) hl.add('APUFEED');
  for (const m of all(/BLEED (\d)/g)) hl.add('BLEED' + m[1]);
  for (const m of all(/ENG (\d) BLEED/g)) hl.add('BLEED' + m[1]);
  for (const m of all(/PACK (\d)(?:\+(\d))?/g)) { hl.add('PACK' + m[1]); if (m[2]) hl.add('PACK' + m[2]); }
  if (/XBLEED|X BLEED|CROSSBLEED/.test(t)) ['XBLEEDL', 'XBLEEDC', 'XBLEEDR'].forEach((x) => hl.add(x));
  if (/APU BLEED/.test(t)) hl.add('APUBLEED');
  if (/HOT AIR/.test(t)) hl.add('HOTAIR');
  if (/DUCT|LEAK/.test(t)) hl.add('DUCT');
  for (const m of all(/(?:SPLR|SPOILER)S? (\d)/g)) hl.add('SPLR' + m[1]);
  if (/SPLRS|SPOILERS|SPD BRK|GND SPLR/.test(t)) hl.add('SPLRALL');
  for (const m of all(/\b([LR]) (INR|MID|OUTR) AIL/g)) hl.add('AIL' + m[1] + m[2]);
  for (const m of all(/\b([LR]) AIL/g)) hl.add('AIL' + m[1]);
  if (/AILERONS?|AIL /.test(t) && ![...hl].some((x) => x.startsWith('AIL'))) hl.add('AILALL');
  for (const m of all(/\b([LR]) (INR|OUTR) ELEV/g)) hl.add('ELEV' + m[1] + m[2]);
  for (const m of all(/\b([LR]) ELEV/g)) hl.add('ELEV' + m[1]);
  if (/ELEV/.test(t) && ![...hl].some((x) => x.startsWith('ELEV'))) hl.add('ELEVALL');
  if (/UPPER RUD|UPR RUD/.test(t)) hl.add('RUDUPPER');
  if (/LOWER RUD|LWR RUD/.test(t)) hl.add('RUDLOWER');
  if (/RUD/.test(t)) hl.add('RUD');
  if (/SLAT/.test(t)) hl.add('SLAT');
  if (/FLAP/.test(t)) hl.add('FLAP');
  if (/PITCH TRIM|THS|STAB/.test(t)) hl.add('THS');
  if (/N\/W|NOSE (?:L\/G|GEAR|WHEEL)|NLG/.test(t)) hl.add('NLG');
  const tg = t.replace(/L\/G/g, 'GEAR');
  const sideOf = (a, b) => (/RIGHT|\bR\b|\bRH\b/.test(tg) ? [b] : /LEFT|\bL\b|\bLH\b/.test(tg) ? [a] : [a, b]);
  if (/BODY GEAR|BLG/.test(tg)) sideOf('BLGL', 'BLGR').forEach((x) => hl.add(x));
  if (/WING GEAR|WLG/.test(tg)) sideOf('WLGL', 'WLGR').forEach((x) => hl.add(x));
  if (/L\/G|GEAR|BRAKES|WHEEL|STEER|MLG/.test(t) && ![...hl].some((x) => /LG/.test(x))) hl.add(s === 'STEER' ? 'NLG' : 'GEAR');
  for (const m of all(/(?:DOOR )?(MAIN|UPPER|UPR) (\d)([LR])?/g)) hl.add('D' + (m[1] === 'MAIN' ? 'M' : 'U') + m[2]);
  if (/FWD CARGO/.test(t)) hl.add('CFWD');
  if (/AFT CARGO/.test(t)) hl.add('CAFT');
  if (/BULK/.test(t)) hl.add('CBULK');
  if (/AVNCS/.test(t)) hl.add('AVNCS');
  if (/DOORS?\b/.test(t) && ![...hl].some((x) => x.startsWith('D'))) hl.add('DALL');
  return hl;
}

function ecamDiagram(p) {
  const s = (p.s || '').toUpperCase();
  const t = p.t.toUpperCase();
  const hl = exHighlights(p);
  const fire = /FIRE/.test(t) || /FIRE/.test(s);
  if (/^HYD/.test(s) || /^HYD/.test(t)) return exHyd(hl);
  if (/^ELEC/.test(s) || /^\*?ELEC/.test(t)) return exElec(hl);
  if (/^FUEL/.test(s)) return exFuel(hl);
  if (/^(AIR|COND|VENT)/.test(s) && /BLEED|PACK|XBLEED|HOT AIR|DUCT|LEAK|ENG \d/.test(t)) return exBleed(hl);
  if (/^DOOR/.test(s) || (/^SMOKE/.test(s) && /CARGO|AVNCS|BULK/.test(t))) return exSideView(hl, /^SMOKE/.test(s));
  if (/^(ENG|F\/CTL|BRAKES|L\/G|STEER|MLG BAY|WHEEL|APU)/.test(s) || /FIRE/.test(s) || /^ENG \d/.test(s)) return exPlanView(hl, fire);
  return null;
}

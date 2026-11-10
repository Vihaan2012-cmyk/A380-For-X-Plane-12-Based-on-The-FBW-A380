const EX_UNITS = {
  NAV: [['ADR', ['1', '2', '3']], ['IR', ['1', '2', '3']], ['AOA', ['1', '2', '3']], ['GPS', ['1', '2']], ['ILS', ['1', '2']], ['FLS', ['1', '2']], ['GLS', ['1', '2']], ['HUD', ['']]],
  SURV: [['TCAS', ['1', '2']], ['TAWS', ['1', '2']], ['GPWS', ['1', '2']], ['TERR SYS', ['1', '2']], ['XPDR', ['1', '2']], ['ROW/ROP', ['']]],
  COM: [['VHF', ['1', '2', '3']], ['HF', ['1', '2']], ['SATCOM', ['']], ['RMP', ['1', '2', '3']], ['PTT', ['CAPT', 'F/O', '3RD']], ['DATALINK', ['']]],
  'AUTO FLT': [['AP', ['']], ['A/THR', ['1', '2', '3', '4']], ['FMS', ['1', '2']], ['FMC', ['A', 'B', 'C']], ['AFS CTL PNL', ['', 'CAPT BKUP', 'F/O BKUP']], ['TCAS MODE', ['']]],
  FWS: [['FWS', ['1', '2']], ['FCDC', ['1', '2']], ['ECP', ['']], ['AUDIO', ['']]],
  RECORDER: [['CVR', ['']], ['DFDR', ['']], ['ACCELMTR', ['']]],
  'CAB COM': [['CIDS', ['1', '2', '3']], ['CABIN COM', ['']]],
};

function exUnitHits(t, name, ids) {
  const hits = new Set();
  const esc = name.replace(/\//g, '\\/');
  const re = new RegExp(`${esc}[ -]?((?:[0-9A-C](?:\\+|\\s*&\\s*)?)+)?(?![A-Z])`, 'g');
  for (const m of t.matchAll(re)) {
    if (!m[1]) { ids.forEach((x) => hits.add(x)); continue; }
    m[1].split(/\+|&/).map((x) => x.trim()).filter(Boolean).forEach((x) => hits.add(x));
  }
  if (name === 'A/THR') for (const m of t.matchAll(/ENG (\d) A\/THR/g)) hits.add(m[1]);
  if (name === 'AFS CTL PNL') {
    hits.clear();
    if (/AFS CTL PNL/.test(t)) hits.add('');
    if (/CAPT BKUP/.test(t)) hits.add('CAPT BKUP');
    if (/F\/O BKUP/.test(t)) hits.add('F/O BKUP');
  }
  if (name === 'PTT') {
    hits.clear();
    if (/CAPT PTT/.test(t)) hits.add('CAPT');
    if (/F\/O PTT/.test(t)) hits.add('F/O');
    if (/THIRD/.test(t)) hits.add('3RD');
  }
  return hits;
}

function exUnits(sys, p) {
  const rows = EX_UNITS[sys];
  const t = ' ' + p.t.toUpperCase().replace(/^\S+(?:\s+FLT)?\s/, '') + ' ';
  const W = 980, rowH = 52, H = 30 + rows.length * rowH, parts = [];
  rows.forEach(([name, ids], r) => {
    const y = 16 + r * rowH;
    let hits = exUnitHits(t, name, ids);
    const word = new RegExp(`(^|[^A-Z/])${name.replace(/\//g, '\\/')}([^A-Z]|$)`);
    if (!word.test(t) && !['A/THR', 'AFS CTL PNL', 'PTT'].includes(name)) hits = new Set();
    if (name === 'A/THR' && !/A\/THR/.test(t)) hits = new Set();
    const lit = hits.size > 0;
    parts.push(exT(250, y + 28, name, { anchor: 'end', size: 14, fill: lit ? EX.hot : EX.ink }));
    const bw = ids.length > 3 ? 120 : 150;
    ids.forEach((id, i) => {
      const on = hits.has(id);
      parts.push(exBox(280 + i * (bw + 16), y + 6, bw, 36, id || 'SYS', on, { size: 13 }));
    });
  });
  return exSvg(W, H, parts);
}

function exCds(p) {
  const t = p.t.toUpperCase();
  const W = 980, H = 320, parts = [];
  const both = /CAPT\+F\/O|CAPT F\/O/.test(t);
  const side = (k) => both || (k === 'CAPT' ? /CAPT/.test(t) : /F\/O/.test(t));
  const du = [['CAPT', 'PFD', 20], ['CAPT', 'ND', 150], ['', 'EWD', 300], ['', 'SD', 430], ['', 'MFD', 560], ['F/O', 'ND', 690], ['F/O', 'PFD', 820]];
  for (const [who, n, x] of du) {
    const named = new RegExp(`\\b${n}\\b`).test(t);
    const on = /DISPLAY DISAGREE/.test(t) ? n === 'PFD' || n === 'ND' : /DU/.test(t) && named && (who ? side(who) : true);
    parts.push(exBox(x, 24, 120, 120, '', on));
    parts.push(exT(x + 60, 80, n, { size: 15, fill: on ? '#fff' : EX.ink }), exT(x + 60, 104, who, { size: 11.5, fill: on ? '#fff' : EX.dim }));
  }
  for (const [who, x] of [['CAPT', 40], ['F/O', 740]]) {
    const s = side(who);
    parts.push(exT(x + 100, 172, who, { size: 12, fill: EX.dim }));
    parts.push(exBox(x, 182, 200, 34, 'EFIS CTL PNL', s && /EFIS CTL PNL/.test(t), { size: 12 }));
    parts.push(exBox(x, 224, 200, 34, 'EFIS BKUP CTL', s && /EFIS BKUP/.test(t), { size: 12 }));
    parts.push(exBox(x, 266, 96, 34, 'KCCU', s && /CURSOR/.test(t), { size: 12 }), exBox(x + 104, 266, 96, 34, 'KEYBD', s && /KEYBOARD/.test(t), { size: 12 }));
  }
  if (/MAILBOX/.test(t)) parts.push(exBox(410, 230, 160, 40, 'MAILBOX', true, { size: 12 }));
  return exSvg(W, H, parts);
}

const EX_ZONES = [
  ['MAIN AVNCS', 3.5, 9.5, -2.2, 0.8, /MAIN AVNCS|AVNCS VENT|AVNCS BLOWING|AVNCS EXTRACT|AVNCS OVBD|COOLG/],
  ['UPPER AVNCS', 3.5, 8.0, 4.2, 6.6, /UPPER AVNCS/],
  ['AFT AVNCS', 44.0, 47.0, 4.2, 6.6, /AFT AVNCS/],
  ['FWD LWR REST', 27.9, 33.5, -2.2, 0.8, /FWD LWR CAB REST/],
  ['LWR LAV', 33.5, 36.5, -2.2, 0.8, /LOWER DECK LAV/],
  ['IFE BAY', 30.0, 33.5, 4.2, 6.6, /IFE BAY/],
  ['FWD CARGO', 10.5, 27.9, -2.2, 0.8, /FWD CARGO|FWD\+AFT CARGO/],
  ['AFT CARGO', 37.0, 46.7, -2.2, 0.8, /AFT CARGO|FWD\+AFT CARGO/],
  ['BULK', 47.3, 50.1, -2.2, 0.8, /BULK/],
  ['PACK BAY 1', 22.0, 30.0, -2.6, -1.2, /PACK BAY 1/],
  ['PACK BAY 2', 22.0, 30.0, -1.2, 0.2, /PACK BAY 2|PACK BAY 1\+2/],
  ['MAIN DECK FWD', 6.0, 30.0, 1.2, 3.6, /FWD VENT|PURSER|PRIMARY CABIN FAN|SECONDARY CABIN FAN|TEMP CTL|MIXER/],
  ['MAIN DECK AFT', 30.0, 56.0, 1.2, 3.6, /AFT VENT|TEMP CTL|CABIN AIR EXTRACT|SECONDARY CABIN FAN|PRIMARY CABIN FAN/],
  ['UPPER DECK', 12.0, 52.0, 4.2, 6.6, /TEMP CTL|PURSER|LAV & GALLEYS/],
  ['THS BAY', 58.0, 64.0, 2.0, 4.6, /THS BAY/],
  ['MAIN 5L REST', 54.0, 57.5, 1.2, 3.6, /MAIN 5L CAB REST/],
];

function exCabin(p) {
  const t = p.t.toUpperCase();
  const fire = /SMOKE|FIRE/.test(t) && !/DET FAULT|BOTTLES|BTL|FAULT$/.test(t);
  const hotc = fire ? EX.fire : EX.hot;
  const W = 980, H = 400, s = 12.6, x0 = 26, y0 = 40;
  const p2 = (z, y) => [x0 + (z + 35.4) * s, y0 + (20.4 - y) * s];
  const parts = [exPath(DX_DATA.sil.side.map(([z, y]) => p2(z, y)), { fill: 'rgba(255,255,255,0.03)', sw: 1.6 })];
  for (const [n, a, b, ylo, yhi, re] of EX_ZONES) {
    const on = re.test(t);
    if (!on && /^(PACK BAY|MAIN DECK|UPPER DECK|THS BAY|MAIN 5L)/.test(n)) continue;
    const [ax, ay] = p2(EX_NOSE + a, yhi), [bx, by] = p2(EX_NOSE + b, ylo);
    parts.push(`<rect x="${ax}" y="${ay}" width="${bx - ax}" height="${by - ay}" fill="${on ? hotc : 'none'}" fill-opacity="${on ? 0.85 : 0}" stroke="${on ? hotc : EX.dim}" stroke-width="1.3" stroke-dasharray="${on ? '' : '5 4'}"/>`);
    if (on) parts.push(exT((ax + bx) / 2, ay - 10, n, { size: 12.5, fill: hotc }));
  }
  for (const m of t.matchAll(/MAIN (\d)([LR]) (CWS|RCC)/g)) {
    const b = exPart(`ANIM_DOOR_M${m[1]}${m[2]}`);
    if (!b) continue;
    const [cx, cy] = p2(b[1][2] + 2.0, 2.4);
    parts.push(`<circle cx="${cx}" cy="${cy}" r="14" fill="${hotc}"/>`, exT(cx, cy - 22, `${m[1]}${m[2]} ${m[3]}`, { size: 12.5, fill: hotc }));
  }
  const ofv = [[1, 6.0], [2, 9.0], [3, 52.0], [4, 55.0]];
  for (const [n, z] of ofv) {
    const on = /OUTFLW/.test(t) && (new RegExp(`CTL ${n}\\b`).test(t) || !/CTL \d/.test(t));
    const [cx, cy] = p2(EX_NOSE + z, z < 20 ? -2.6 : -1.4);
    parts.push(`<circle cx="${cx}" cy="${cy}" r="9" fill="${on ? EX.hot : '#1a1d21'}" stroke="${on ? EX.hot : EX.dim}" stroke-width="1.4"/>`, exT(cx, cy + 26, `OFV ${n}`, { size: 11, fill: on ? EX.hot : EX.dim }));
  }
  if (/CAB PRESS/.test(t) && !/OUTFLW/.test(t)) {
    const [ax, ay] = p2(EX_NOSE + 2, 7.2), [bx, by] = p2(EX_NOSE + 60, -3.0);
    parts.push(`<rect x="${ax}" y="${ay}" width="${bx - ax}" height="${by - ay}" rx="40" fill="none" stroke="${EX.hot}" stroke-width="3"/>`);
  }
  if (/EMER EXIT/.test(t)) {
    for (const [n, key] of EX_DOORS) {
      const b = exPart(key);
      if (!b) continue;
      const [ax, ay] = p2(b[0][2], b[1][1]);
      parts.push(`<rect x="${ax}" y="${ay}" width="${(b[1][2] - b[0][2]) * s}" height="${(b[1][1] - b[0][1]) * s}" rx="5" fill="${EX.hot}"/>`);
    }
  }
  return exSvg(W, H, parts);
}

const exBaseDiagram = ecamDiagram;
ecamDiagram = function (p) {
  const s = (p.s || '').toUpperCase().replace(/^\*/, '');
  const t = p.t.toUpperCase();
  if (EX_UNITS[s]) return exUnits(s, p);
  if (s === 'CDS' || s === 'CDS & AUTO FLT') return exCds(p);
  if (/^(SMOKE|VENT|CAB PRESS|CABIN)$/.test(s) || (s === 'COND' && !/HOT AIR|RAM AIR/.test(t))) return exCabin(p);
  if (s === 'COND') return exBleed(exHighlights(p));
  if (s === 'MLG BAY') return exPlanView(new Set(['MLG']), /FIRE$/.test(t));
  if (s === 'CONFIG') {
    const hl = new Set();
    if (/FLAPS/.test(t)) hl.add('FLAP');
    if (/SLATS/.test(t)) hl.add('SLAT');
    if (/PITCH TRIM/.test(t)) hl.add('THS');
    if (/RUDDER TRIM/.test(t)) hl.add('RUD');
    if (/SPD BRK/.test(t)) hl.add('SPLRALL');
    if (/PARK BRK/.test(t)) hl.add('GEAR');
    return hl.size ? exPlanView(hl, false) : null;
  }
  return exBaseDiagram(p);
};

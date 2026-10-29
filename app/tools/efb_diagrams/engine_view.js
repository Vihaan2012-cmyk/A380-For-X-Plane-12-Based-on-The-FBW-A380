const ENG = {
  axis: 220,
  case: 'rgba(255,255,255,0.06)', metal: 'rgba(200,210,220,0.18)', blade: '#cfd6dd', bypass: '#5ad1ff', cold: '#8fd8ff', warm: '#ffd36b', hot: '#ff7a2f', flame: '#ff4b1f',
  bad: '#ff3b3b', warn: '#ffb000', hl: '#e040fb', oil: '#c9a227', fuel: '#ff9de2', bleed: '#7cf0b0',
};

const ENG_PARTS = {
  inlet: { x: 70, label: 'INLET' },
  spinner: { x: 120, label: 'SPINNER' },
  fan: { x: 168, label: 'FAN' },
  fanCase: { x: 250, label: 'FAN CASE' },
  vsv: { x: 232, label: 'IP VSV' },
  ipc: { x: 285, label: 'IP COMPRESSOR' },
  hpc: { x: 425, label: 'HP COMPRESSOR' },
  combustor: { x: 515, label: 'COMBUSTOR' },
  nozzles: { x: 487, label: 'FUEL NOZZLES' },
  igniter: { x: 500, label: 'IGNITERS' },
  hpt: { x: 575, label: 'HP TURBINE' },
  ipt: { x: 608, label: 'IP TURBINE' },
  lpt: { x: 690, label: 'LP TURBINE' },
  exhaust: { x: 840, label: 'EXHAUST' },
  bypassDuct: { x: 420, label: 'BYPASS DUCT' },
  reverser: { x: 470, label: 'THRUST REVERSER' },
  cowl: { x: 300, label: 'FAN COWL' },
  brgFan: { x: 205, label: 'FAN FRONT BRG' },
  brgIp: { x: 228, label: 'IP FRONT BRG' },
  brgHp: { x: 562, label: 'HP TURBINE BRG' },
  brgIpt: { x: 625, label: 'IP TURBINE BRG' },
  brgLp: { x: 772, label: 'LP TURBINE REAR BRG' },
  gearbox: { x: 350, label: 'GEARBOX' },
  starter: { x: 300, label: 'STARTER' },
  oilPump: { x: 335, label: 'OIL PUMP' },
  oilTank: { x: 405, label: 'OIL TANK' },
  fuelPump: { x: 370, label: 'FUEL PUMPS' },
  fmu: { x: 440, label: 'FMU' },
  generator: { x: 270, label: 'VFG' },
  edp: { x: 430, label: 'EDP A/B' },
  eec: { x: 300, label: 'EEC' },
  bleedIp: { x: 330, label: 'IP8 BLEED' },
  bleedHp: { x: 455, label: 'HP6 BLEED' },
  precooler: { x: 470, label: 'PRECOOLER' },
  t25: { x: 352, label: 'T25' },
  p30: { x: 470, label: 'P30' },
  tgt: { x: 600, label: 'TGT' },
  vibFan: { x: 215, label: 'VIB FAN' },
  vibCore: { x: 540, label: 'VIB CORE' },
  speed: { x: 190, label: 'SPEED PICKUP' },
  fanZone: { x: 330, label: 'FAN ZONE' },
  coreZone: { x: 620, label: 'CORE ZONE' },
};

function engFaults(failures, engine) {
  const out = new Map();
  const put = (k, mode, mag) => { const prev = out.get(k); if (!prev || mag > prev.mag) out.set(k, { mode, mag }); };
  for (const f of failures) {
    if (!f.active) continue;
    const id = f.id;
    const mag = f.magnitude > 0 ? f.magnitude : 1;
    const n = f.name.toUpperCase();
    const en = n.match(/\bENG(?:INE)?\s*(\d)/);
    if (id < 100000) {
      const base = id - ((id - 72000) % 4 + 4) % 4;
      const e = id - base + 1;
      if (Math.floor(id / 1000) === 72 || Math.floor(id / 1000) === 54) {
        if (e !== engine && !(en && +en[1] === engine)) continue;
      } else if (!en || +en[1] !== engine) continue;
      if (base === 72000) put('brgHp', 'wear', mag);
      else if (base === 72004) put('ipc', 'stall', mag), put('hpc', 'stall', mag);
      else if (base === 72008) put('hpt', 'damage', mag);
      else if (base === 72012) put('hpc', 'destroyed', mag);
      else if (base === 72016) put('hpt', 'released', mag);
      else if (base === 72020) put('brgHp', 'seized', mag);
      else if (base === 72024) put('inlet', 'fod', mag), put('hpc', 'destroyed', mag);
      else if (id >= 54010 && id < 54014) put('cowl', 'separated', 1);
      else engKeywords(n, put, mag);
      continue;
    }
    if (!en || +en[1] !== engine) continue;
    engKeywords(n, put, mag);
  }
  return out;
}

function engKeywords(n, put, mag) {
  const rules = [
    [/\bFAIL$|SHUT ?DOWN|FLAMEOUT|FLAME OUT|RELIGHT/, 'combustor', 'out'], [/\bSTART\b|STARTER|IGN START/, 'starter', 'fault'], [/OVERLIMIT|OVER LIMIT|OVERSPEED/, 'tgt', 'fault'],
    [/FAN FRONT BRG/, 'brgFan', 'wear'], [/IP FRONT BRG/, 'brgIp', 'wear'], [/HP TURBINE BRG/, 'brgHp', 'wear'], [/IP TURBINE BRG/, 'brgIpt', 'wear'], [/LP TURBINE REAR BRG/, 'brgLp', 'wear'],
    [/BEARING/, 'brgHp', 'wear'], [/BLADE LOSS|FAN BLADE|BIRD STRIKE FAN/, 'fan', 'damage'], [/BIRD STRIKE CORE|INGESTION|FOD/, 'inlet', 'fod'],
    [/VSV/, 'vsv', 'jam'], [/T25/, 't25', 'fault'], [/P30/, 'p30', 'fault'], [/TGT|EGT/, 'tgt', 'fault'], [/VIBRATION PICKUP \(FAN\)|VIB.*FAN/, 'vibFan', 'fault'],
    [/VIBRATION PICKUP|VIB/, 'vibCore', 'fault'], [/SPEED PICKUP|TACHOMETER|N1 |N2 |N3 /, 'speed', 'fault'], [/NOZZLE|COKING/, 'nozzles', 'coking'],
    [/METERING|FMU/, 'fmu', 'stuck'], [/FUEL PUMP|LP FUEL|HP FUEL|STRAINER|FUEL FILTER|FUEL FLOW/, 'fuelPump', 'fault'], [/OIL PUMP|OIL PRESS|SCAVENGE/, 'oilPump', 'fault'],
    [/OIL (?:LEAK|QTY|QUANTITY|TANK)|OIL FILTER|CHIP/, 'oilTank', 'fault'], [/OIL TEMP|OIL COOLER|FOHE/, 'oilTank', 'hot'], [/STARTER/, 'starter', 'fault'], [/IGNIT/, 'igniter', 'fault'],
    [/FADEC|EEC/, 'eec', 'fault'], [/REVERSER|REV /, 'reverser', 'fault'], [/PRECOOLER/, 'precooler', 'fault'], [/HP BLEED|HP VALVE/, 'bleedHp', 'stuck'],
    [/BLEED|IP CHECK/, 'bleedIp', 'fault'], [/GEN(?:ERATOR)?|VFG|IDG/, 'generator', 'fault'], [/EDP|ENGINE DRIVEN PUMP|PUMP [AB]/, 'edp', 'fault'],
    [/FIRE.*FAN ZONE|FAN ZONE/, 'fanZone', 'fire'], [/FIRE|LOOP/, 'coreZone', /DETECT|LOOP|OPEN|SHORT|FALSE/.test(n) ? 'loop' : 'fire'], [/GEARBOX|AGB/, 'gearbox', 'fault'],
    [/COMPRESSOR STALL|SURGE/, 'hpc', 'stall'], [/COMPRESSOR EROSION|ASH.*COMPRESSOR/, 'hpc', 'erosion'], [/NGV|GLASSING/, 'hpt', 'damage'], [/ICE CRYSTAL|ACCRETION/, 'ipc', 'ice'],
    [/FLAMEOUT|ROLL-?BACK/, 'combustor', 'out'], [/CASE|CONTAINMENT/, 'fanCase', 'damage'],
  ];
  for (const [re, k, mode] of rules) if (re.test(n)) { put(k, mode, mag); return; }
  put('combustor', 'fault', mag);
}

function engView(o) {
  const W = 1000, H = 490, A = ENG.axis;
  const faults = o.faults || new Map();
  const f = (k) => faults.get(k);
  const n1 = Math.max(0, o.n1 || 0), n3 = Math.max(0, o.n3 || 0);
  const lit = (o.running ?? n3 > 50) && !(f('combustor') && f('combustor').mode === 'out') && !(f('hpc') && f('hpc').mode === 'destroyed') && !(f('brgHp') && f('brgHp').mode === 'seized');
  const hpStopped = f('brgHp') && f('brgHp').mode === 'seized';
  const surge = (f('hpc') && f('hpc').mode === 'stall') || (f('ipc') && f('ipc').mode === 'stall');
  const shattered = f('hpc') && f('hpc').mode === 'destroyed';
  const shake = Math.max(...[...faults.entries()].filter(([k]) => /^brg|fan|hpt/.test(k)).map(([, v]) => (v.mode === 'released' || v.mode === 'damage' ? 3 : v.mode === 'seized' ? 0 : 1.2) * v.mag), 0);
  const parts = [];
  const sym = (pts) => [...pts.map(([x, r]) => [x, A - r]), ...pts.slice().reverse().map(([x, r]) => [x, A + r])];
  const poly = (pts, fill, stroke, sw = 1.4, extra = '') => `<path d="${pts.map((p, i) => (i ? 'L' : 'M') + p[0].toFixed(1) + ',' + p[1].toFixed(1)).join('')}Z" fill="${fill}" stroke="${stroke}" stroke-width="${sw}" stroke-linejoin="round"${extra}/>`;
  const band = (outer, inner, fill, stroke) => {
    const top = [...outer.map(([x, r]) => [x, A - r]), ...inner.slice().reverse().map(([x, r]) => [x, A - r])];
    const bot = [...outer.map(([x, r]) => [x, A + r]), ...inner.slice().reverse().map(([x, r]) => [x, A + r])];
    return poly(top, fill, stroke) + poly(bot, fill, stroke);
  };
  const hl = (k) => (f(k) ? (f(k).mode === 'fire' || /destroyed|released|seized|separated/.test(f(k).mode) ? ENG.bad : ENG.warn) : null);
  const glow = (k) => (f(k) ? ` filter="url(#engGlow)"` : '');

  parts.push(`<defs><filter id="engGlow" x="-50%" y="-50%" width="200%" height="200%"><feGaussianBlur stdDeviation="4" result="b"/><feMerge><feMergeNode in="b"/><feMergeNode in="SourceGraphic"/></feMerge></filter>
<linearGradient id="engCore" x1="0" x2="1" y1="0" y2="0"><stop offset="0" stop-color="${ENG.cold}"/><stop offset="0.42" stop-color="${ENG.warm}"/><stop offset="0.52" stop-color="${lit ? ENG.flame : ENG.warm}"/><stop offset="0.75" stop-color="${lit ? ENG.hot : ENG.warm}"/><stop offset="1" stop-color="${lit ? '#ffb46b' : ENG.cold}"/></linearGradient>
<style>.engflow{fill:none;stroke-linecap:round;animation:engdash linear infinite}.engrev{animation:engrev 0.9s ease-in-out infinite}.engshake{animation:engshake 0.09s linear infinite}.engpulse{animation:engpulse 0.8s ease-in-out infinite}.engspark{animation:engspark 1.2s ease-out infinite}
@keyframes engdash{to{stroke-dashoffset:-60}}@keyframes engrev{0%,60%{stroke-dashoffset:0}80%{stroke-dashoffset:40}100%{stroke-dashoffset:-60}}@keyframes engshake{0%{transform:translate(0,0)}25%{transform:translate(var(--sx),0)}50%{transform:translate(0,var(--sy))}75%{transform:translate(calc(var(--sx) * -1),0)}}
@keyframes engpulse{0%,100%{opacity:0.25}50%{opacity:1}}@keyframes engspark{0%{opacity:1;transform:translate(0,0)}100%{opacity:0;transform:translate(var(--dx),var(--dy))}}</style></defs>`);

  const cowlGone = f('cowl');
  const nacOuter = [[40, 176], [60, 186], [120, 190], [240, 192], [420, 186], [500, 178], [600, 150]];
  const nacInner = [[40, 168], [70, 158], [150, 156], [200, 158], [420, 160], [520, 152], [600, 144]];
  if (cowlGone) parts.push(band(nacOuter.filter(([x]) => x >= 420), nacInner.filter(([x]) => x >= 420), ENG.case, hl('cowl')));
  else parts.push(band(nacOuter, nacInner, ENG.case, f('fanZone') ? ENG.bad : '#9aa6b2'));
  parts.push(band([[200, 156], [260, 158], [420, 158]], [[200, 150], [260, 150], [420, 152]], f('fanCase') ? 'rgba(255,59,59,0.35)' : ENG.metal, hl('fanCase') || '#9aa6b2'));
  const coreCowl = [[200, 96], [300, 104], [420, 100], [560, 92], [640, 100], [780, 104], [800, 96]];
  const coreCase = [[205, 90], [300, 86], [370, 70], [470, 62], [560, 80], [640, 92], [780, 100]];
  parts.push(band(coreCowl, coreCase, f('coreZone') ? 'rgba(255,59,59,0.25)' : ENG.case, f('coreZone') ? ENG.bad : '#7f8a95'));
  parts.push(poly(sym([[780, 100], [880, 80], [905, 72]]).concat([]), 'none', '#7f8a95'));
  parts.push(poly(sym([[760, 46], [840, 36], [930, 4]]), 'rgba(255,255,255,0.08)', '#9aa6b2'));
  const hub = [[200, 44], [300, 46], [370, 40], [470, 40], [560, 48], [640, 52], [760, 46]];
  parts.push(band(coreCase, hub, 'rgba(0,0,0,0)', 'rgba(255,255,255,0.15)'));

  const flowW = 0.5 + 1.8 * Math.min(1.2, n1 / 90);
  const bypassDur = n1 > 2 ? (2.2 / Math.max(0.15, n1 / 85)).toFixed(2) : 0;
  const coreDur = n3 > 5 && !hpStopped ? (1.6 / Math.max(0.2, n3 / 95)).toFixed(2) : 0;
  const stream = (d, stroke, dur, w, cls = '', dash = '10 20') => (dur ? `<path d="${d}" class="engflow ${cls}" stroke="${stroke}" stroke-width="${w}" stroke-dasharray="${dash}" style="animation-duration:${dur}s" opacity="0.85"/>` : `<path d="${d}" fill="none" stroke="${stroke}" stroke-width="${w * 0.6}" stroke-dasharray="2 14" opacity="0.35"/>`);
  for (const s of [-1, 1]) {
    for (const r of [112, 126, 140]) parts.push(stream(`M20,${A + s * (r + 30)} L190,${A + s * r} L420,${A + s * (r + 2)} L620,${A + s * (r - 6)} L720,${A + s * (r - 14)}`, ENG.bypass, bypassDur, flowW));
    const turb = shattered || surge;
    for (const r of [55, 68, 80]) {
      const wob = turb ? (k) => (k % 2 ? 9 : -9) * (f('hpc') ? f('hpc').mag : 1) : () => 0;
      const pts = [[20, r + 6], [190, r + 8], [230, r - 4 + wob(1)], [330, r - 14 + wob(2)], [420, r - 26 + wob(3)], [470, r - 22 + wob(4)], [520, r - 12 + wob(5)], [580, r - 6], [700, r + 6], [800, r - 10], [960, r - 30]];
      const d = 'M' + pts.map(([x, rr]) => `${x},${A + s * Math.max(6, rr)}`).join(' L');
      if (shattered) parts.push(stream(d, 'url(#engCore)', coreDur ? (coreDur * 3).toFixed(2) : 2.5, flowW * 0.8, '', '3 9 14 26'));
      else parts.push(stream(d, 'url(#engCore)', coreDur, flowW, surge ? 'engrev' : ''));
    }
  }

  const shakeStyle = shake > 0 ? ` class="engshake" style="--sx:${shake.toFixed(1)}px;--sy:${(shake * 0.7).toFixed(1)}px"` : '';
  parts.push(`<g${shakeStyle}>`);
  parts.push(poly(sym([[90, 0], [120, 26], [160, 42], [170, 44]]), 'rgba(255,255,255,0.12)', hl('spinner') || '#cfd6dd'));
  const fanCol = hl('fan') || ENG.blade;
  for (const s of [-1, 1]) {
    for (let k = 0; k < 5; k++) {
      const y0 = A + s * 44, y1 = A + s * (150 - k * 0.5);
      const broken = f('fan') && k === 2;
      parts.push(`<path d="M${162 + k * 4},${y0} Q${176 + k * 4},${(y0 + y1) / 2} ${168 + k * 4},${broken ? A + s * 95 : y1}" fill="none" stroke="${fanCol}" stroke-width="${k === 2 ? 3 : 1.4}" opacity="${k === 2 ? 1 : 0.55}"/>`);
    }
  }
  const stage = (x0, n, gap, rIn0, rIn1, rOut0, rOut1, key, w = 4) => {
    const c = hl(key) || ENG.blade;
    const out = [];
    for (let k = 0; k < n; k++) {
      const t = n > 1 ? k / (n - 1) : 0;
      const x = x0 + k * gap;
      const ri = rIn0 + (rIn1 - rIn0) * t, ro = rOut0 + (rOut1 - rOut0) * t;
      for (const s of [-1, 1]) {
        const brokenAt = key === 'hpc' && shattered && k % 2 === 0 ? ri + (ro - ri) * 0.35 : key === 'hpt' && f('hpt') && f('hpt').mode === 'released' ? ri + (ro - ri) * 0.4 : ro;
        out.push(`<rect x="${x}" y="${Math.min(A + s * ri, A + s * brokenAt)}" width="${w}" height="${Math.abs(brokenAt - ri)}" fill="${c}" opacity="0.9"/>`);
        if (k < n - 1) out.push(`<rect x="${x + gap / 2}" y="${Math.min(A + s * (ri + 4), A + s * ro)}" width="${w * 0.6}" height="${Math.abs(ro - ri - 4)}" fill="${c}" opacity="0.35"/>`);
        if (brokenAt < ro) out.push(`<path d="M${x - 2},${A + s * brokenAt} l3,${s * 5} l3,${-s * 4} l3,${s * 6}" stroke="${ENG.bad}" stroke-width="1.6" fill="none"/>`);
      }
    }
    return `<g${glow(key)}>${out.join('')}</g>`;
  };
  parts.push(stage(222, 2, 10, 50, 50, 88, 86, 'vsv', 3));
  parts.push(stage(250, 7, 14, 48, 46, 86, 72, 'ipc'));
  parts.push(stage(378, 6, 15, 42, 44, 66, 60, 'hpc'));
  parts.push(stage(568, 1, 0, 50, 50, 82, 82, 'hpt', 7));
  parts.push(stage(602, 1, 0, 52, 52, 92, 92, 'ipt', 7));
  parts.push(stage(632, 5, 26, 50, 46, 98, 108, 'lpt', 6));
  parts.push('</g>');

  const flame = lit ? ENG.flame : 'rgba(120,120,120,0.4)';
  for (const s of [-1, 1]) {
    parts.push(poly([[470, A + s * 48], [490, A + s * 44], [560, A + s * 52], [560, A + s * 80], [490, A + s * 80], [470, A + s * 70]], f('combustor') ? 'rgba(255,59,59,0.2)' : 'rgba(255,255,255,0.04)', hl('combustor') || '#9aa6b2'));
    parts.push(`<ellipse cx="${520}" cy="${A + s * 64}" rx="${lit ? 26 : 10}" ry="${lit ? 9 : 4}" fill="${flame}" opacity="${lit ? 0.75 : 0.5}"${lit ? ' filter="url(#engGlow)"' : ''}/>`);
    parts.push(`<circle cx="487" cy="${A + s * 63}" r="5" fill="${hl('nozzles') || ENG.fuel}"${glow('nozzles')}/>`);
    parts.push(`<path d="M500,${A + s * 80} l0,${s * 10}" stroke="${hl('igniter') || ENG.warm}" stroke-width="3"${glow('igniter')}/>`);
  }
  if (shattered || (f('hpt') && f('hpt').mode === 'released')) {
    const cx = shattered ? 430 : 580;
    for (let k = 0; k < 9; k++) {
      const ang = (k / 9) * Math.PI * 2;
      parts.push(`<circle cx="${cx}" cy="${A - 60}" r="2.5" fill="${ENG.warm}" class="engspark" style="--dx:${(Math.cos(ang) * 120).toFixed(0)}px;--dy:${(Math.sin(ang) * 120).toFixed(0)}px;animation-delay:${(k * 0.13).toFixed(2)}s"/>`);
    }
  }
  if (surge) for (const s of [-1, 1]) parts.push(`<path d="M190,${A + s * 70} l-40,${s * 6}" stroke="${ENG.flame}" stroke-width="5" class="engpulse" filter="url(#engGlow)"/>`);
  if (f('inlet')) parts.push(`<path d="M60,${A - 40} l14,-10 l10,12 l-8,14 z" fill="${ENG.warn}" stroke="${ENG.bad}"/>`);

  const shaft = (x0, x1, r, col) => `<path d="M${x0},${A - r} L${x1},${A - r} M${x0},${A + r} L${x1},${A + r}" stroke="${col}" stroke-width="2"/>`;
  parts.push(shaft(165, 780, 8, '#7d8a96'), shaft(240, 620, 15, '#8f9caa'), shaft(380, 580, 22, hpStopped ? ENG.bad : '#a8b4c0'));
  const brg = (k, r) => {
    const c = hl(k);
    const x = ENG_PARTS[k].x;
    return [-1, 1].map((s) => `<circle cx="${x}" cy="${A + s * r}" r="${c ? 8 : 5}" fill="${c ? c : 'rgba(0,0,0,0.5)'}" stroke="${c || '#cfd6dd'}" stroke-width="1.6"${c ? ' class="engpulse" filter="url(#engGlow)"' : ''}/>`).join('');
  };
  parts.push(brg('brgFan', 8), brg('brgIp', 15), brg('brgHp', 22), brg('brgIpt', 15), brg('brgLp', 8));

  const box = (k, x, y, w, h) => {
    const c = hl(k);
    return `<rect x="${x}" y="${y}" width="${w}" height="${h}" rx="4" fill="${c ? c : 'rgba(255,255,255,0.05)'}" fill-opacity="${c ? 0.35 : 1}" stroke="${c || '#9aa6b2'}" stroke-width="1.4"${glow(k)}/>`;
  };
  parts.push(`<path d="M360,${A + 40} L340,${A + 215}" stroke="${hpStopped ? ENG.bad : '#7f8a95'}" stroke-width="3"/>`);
  parts.push(box('gearbox', 250, A + 205, 230, 26));
  parts.push(box('generator', 255, A + 180, 34, 24), box('starter', 292, A + 180, 28, 24), box('oilPump', 323, A + 180, 26, 24), box('fuelPump', 352, A + 180, 40, 24), box('oilTank', 395, A + 176, 28, 28), box('edp', 426, A + 180, 34, 24));
  parts.push(box('fmu', 430, A + 120, 30, 20));
  parts.push(`<path d="M372,${A + 180} L372,${A + 130} L430,${A + 130}" fill="none" stroke="${hl('fuelPump') || hl('fmu') || ENG.fuel}" stroke-width="2"/><path d="M460,${A + 130} L487,${A + 130} L487,${A + 70}" fill="none" stroke="${hl('fmu') || hl('nozzles') || ENG.fuel}" stroke-width="2"/>`);
  parts.push(`<path d="M409,${A + 176} L409,${A + 30} L205,${A + 14} M409,${A + 30} L562,${A + 26}" fill="none" stroke="${hl('oilPump') || hl('oilTank') || ENG.oil}" stroke-width="1.6" stroke-dasharray="5 4"/>`);
  parts.push(box('eec', 280, A - 214, 46, 22));
  parts.push(`<path d="M330,${A - 90} L330,${A - 200} L470,${A - 200}" fill="none" stroke="${hl('bleedIp') || ENG.bleed}" stroke-width="2.4"/><path d="M455,${A - 66} L455,${A - 200}" fill="none" stroke="${hl('bleedHp') || ENG.bleed}" stroke-width="2.4"/>`);
  parts.push(box('precooler', 470, A - 212, 60, 24));
  if (f('reverser')) parts.push(`<rect x="440" y="${A - 186}" width="70" height="12" fill="${ENG.warn}" opacity="0.6"/><rect x="440" y="${A + 174}" width="70" height="12" fill="${ENG.warn}" opacity="0.6"/>`);
  const probe = (k, x, r) => { const c = hl(k); return `<path d="M${x},${A - r} l0,-14" stroke="${c || '#cfd6dd'}" stroke-width="${c ? 4 : 2}"${glow(k)}/><circle cx="${x}" cy="${A - r - 16}" r="${c ? 5 : 3}" fill="${c || '#cfd6dd'}"${glow(k)}/>`; };
  parts.push(probe('t25', 352, 88), probe('p30', 470, 66), probe('tgt', 600, 98), probe('vibFan', 215, 158), probe('vibCore', 540, 94), probe('speed', 190, 152));
  const dim = (x, y, t) => exT(x, y, t, { size: 10, fill: 'rgba(230,230,230,0.55)', weight: 600 });
  parts.push(dim(130, A + 70, 'FAN'), dim(285, A + 37, 'IPC'), dim(425, A + 37, 'HPC'), dim(515, A + 37, 'COMB'), dim(575, A + 37, 'HPT'), dim(612, A + 37, 'IPT'), dim(690, A + 37, 'LPT'), dim(860, A + 37, 'EXHAUST'));
  parts.push(dim(272, A + 196, 'VFG'), dim(306, A + 196, 'STRT'), dim(336, A + 196, 'OIL'), dim(372, A + 196, 'FUEL'), dim(409, A + 194, 'TANK'), dim(443, A + 196, 'EDP'), dim(445, A + 134, 'FMU'), dim(365, A + 224, 'GEARBOX'));
  parts.push(dim(303, A - 199, 'EEC'), dim(500, A - 196, 'PRECOOLER'), dim(316, A - 150, 'IP8'), dim(442, A - 150, 'HP6'), dim(420, A - 168, 'BYPASS'));
  if (f('fanZone') && f('fanZone').mode === 'fire') parts.push(`<path d="M260,${A - 162} q20,-30 40,0 q20,-26 40,0" fill="${ENG.flame}" class="engpulse" filter="url(#engGlow)"/>`);
  if (f('coreZone') && f('coreZone').mode === 'fire') parts.push(`<path d="M600,${A - 100} q20,-30 40,0 q20,-26 40,0" fill="${ENG.flame}" class="engpulse" filter="url(#engGlow)"/>`);
  if (f('coreZone') && f('coreZone').mode === 'loop') parts.push(`<path d="M300,${A - 104} L780,${A - 104}" stroke="${ENG.warn}" stroke-width="3" stroke-dasharray="8 6"/>`);
  const labels = [...faults.keys()].filter((k) => ENG_PARTS[k]);
  labels.forEach((k, i) => {
    const x = ENG_PARTS[k].x;
    parts.push(`<path d="M${x},${A + 236} L${x},${A + 206}" stroke="${hl(k)}" stroke-width="1" stroke-dasharray="3 3"/>`);
    parts.push(exT(Math.min(W - 70, Math.max(70, x)), H - 4 - (i % 2) * 14, ENG_PARTS[k].label, { fill: hl(k), size: 12 }));
  });
  if (o.title) parts.push(exT(24, 24, o.title, { anchor: 'start', size: 15 }));
  if (o.readout) parts.push(exT(W - 20, 24, o.readout, { anchor: 'end', size: 13, fill: EX.dim }));
  return parts;
}

function engineSvg(o) {
  return exSvg(1000, 490, engView(o));
}

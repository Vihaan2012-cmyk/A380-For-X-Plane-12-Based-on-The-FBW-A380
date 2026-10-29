function slMerge(list) {
  const elec = { dead: new Set(), degraded: new Set(), edge: new Map() };
  const hyd = { G: new Map(), Y: new Map() };
  const fuel = new Map(), air = new Map(), fctl = new Map();
  const names = list.filter((f) => f.active).map((f) => f.name || f.title || '');
  const is = (n, re) => re.test(n.toUpperCase());
  for (const n of names) {
    if (is(n, /FIRE|SMOKE/) || !is(n, /ELEC|\bGEN\b|GENERATOR|\bVFG|\bBUS\b|\bTR\b|\bBAT|INV|CONTACTOR|EXT PWR|\bRAT\b|\bAC \d|\bDC \d|\bAC ESS|\bDC ESS/)) continue;
    const e = elecTargets(n);
    e.dead.forEach((k) => elec.dead.add(k));
    e.degraded.forEach((k) => elec.degraded.add(k));
    e.edge.forEach((v, k) => elec.edge.set(k, v));
  }
  for (const n of names) {
    if (is(n, /FIRE|SMOKE/)) continue;
    if (is(n, /HYD|EDP|PUMP|RSVR|RESERVOIR|ACCU|PRIORITY|HX|HEAT EXCH/) && !is(n, /FUEL|BOOST|XFR/)) {
      const h = hydTargets(n);
      if (h) h.m.forEach((v, k) => hyd[h.sys].set(k, v));
    }
    if (is(n, /FUEL|TANK|XFR|JETT|BOOST|CROSSFEED|X ?FEED|REFUEL/)) {
      const f = fuelTargets(n);
      if (f) f.forEach((v, k) => fuel.set(k, v));
    }
    if (is(n, /AIR|BLEED|PACK|COND|VENT|PRESS|OUTFL|DUCT|PRECOOLER|AVNCS|CAB FAN|TRIM AIR|HP VALVE|IP CHECK|FAV|XBLEED/)) {
      const a = airTargets(n);
      if (a) a.forEach((v, k) => air.set(k, v));
    }
    if (is(n, /F\/CTL|FCTL|AILERON|\bAIL\b|ELEV|RUDDER|\bRUD\b|SPLR|SPOILER|SLAT|FLAP|\bTHS\b|\bPRIM|\bSEC\b|\bEHA|EBHA|SIDESTICK/)) {
      const c = fctlTargets(n);
      if (c) c.forEach((v, k) => fctl.set(k, v));
    }
  }
  return { names, elec, hyd, fuel, air, fctl };
}

function slLive(draw) {
  const host = el('div', { class: 'sl-live' });
  let key = null;
  const tick = async () => {
    if (!host.isConnected && key !== null) return;
    let list = [];
    try { list = (await studyGet('/study/failures')).failures || []; } catch { /* offline: nominal */ }
    const t = slMerge(list);
    const k = t.names.join('|');
    if (k !== key) {
      key = k;
      const d = draw(t);
      host.replaceChildren(d, el('div', { class: 'sl-foot', text: t.names.length ? 'ACTIVE: ' + t.names.slice(0, 12).join('  /  ') + (t.names.length > 12 ? `  +${t.names.length - 12}` : '') : 'LIVE  /  NO ACTIVE FAILURES' }));
    }
    if (host.isConnected || key === k) setTimeout(tick, 2000);
  };
  tick();
  return host;
}

function slHits(names, re) { return names.some((n) => re.test(n.toUpperCase())); }

function slState(names, re, warnRe) {
  const u = names.map((n) => n.toUpperCase());
  if (u.some((n) => re.test(n) && !(warnRe && warnRe.test(n)))) return 'fail';
  if (warnRe && u.some((n) => re.test(n) && warnRe.test(n))) return 'warn';
  return 'live';
}

const SL_SOFT = /DEGRAD|WEAR|DRIFT|LEAK|LOW|LO |FAULT|HOT|OVERHEAT|DET/;

function slGear(t) {
  const n = t.names;
  const W = 1000, H = 760, parts = [], fg = [];
  const gears = [['NLG', 500, 90, 2, /NOSE|NLG/], ['WLG L', 330, 380, 4, /(WING|WLG).*(L|LEFT)\b|LEFT WING GEAR|WLG ?L/], ['WLG R', 670, 380, 4, /(WING|WLG).*(R|RIGHT)\b|RIGHT WING GEAR|WLG ?R/], ['BLG L', 420, 520, 6, /(BODY|BLG).*(L|LEFT)\b|BLG ?L/], ['BLG R', 580, 520, 6, /(BODY|BLG).*(R|RIGHT)\b|BLG ?R/]];
  for (const [label, x, y, wheels, re] of gears) {
    const st = slState(n, re, SL_SOFT);
    const cols = wheels === 2 ? 2 : 2, rows = wheels / cols;
    for (let r = 0; r < rows; r++) for (let c = 0; c < cols; c++) {
      const wx = x - 40 + c * 50, wy = y + r * 44;
      const braked = label !== 'NLG' && !(label.startsWith('BLG') && r === 1);
      const tyre = slState(n, new RegExp(`(TIRE|TYRE).*${label.replace(' ', '.*')}`), SL_SOFT);
      fg.push(nsBox(wx, wy, 30, 36, '', tyre === 'live' ? st : tyre, { r: 7 }));
      if (braked) fg.push(`<rect x="${wx + 9}" y="${wy + 12}" width="12" height="12" rx="3" fill="${slState(n, /BRAKE/, SL_SOFT) === 'live' ? NS.green : NS.orange}" opacity="0.85"/>`);
    }
    fg.push(exT(x - 15, y - 14, label, { size: 13, fill: nsStroke(st) }));
  }
  const steer = slState(n, /STEER|NWS|BWS/, SL_SOFT);
  fg.push(nsBox(600, 70, 150, 40, 'NOSE STEERING', steer));
  fg.push(nsBox(720, 560, 170, 40, 'BODY WHEEL STEER', steer));
  const doors = slState(n, /GEAR DOOR|L\/G DOOR/, SL_SOFT);
  fg.push(nsBox(80, 200, 160, 40, 'GEAR DOORS', doors));
  fg.push(nsBox(80, 260, 160, 40, 'L/G CTL (LGCIU)', slState(n, /LGCIU|L\/G CTL|GEAR CTL|PROXIMITY/, SL_SOFT)));
  fg.push(nsBox(80, 320, 160, 40, 'GRAVITY EXT', slState(n, /GRAVITY|FREEFALL/, SL_SOFT)));
  const g = t.hyd.G.size ? 'warn' : 'live', y = t.hyd.Y.size ? 'warn' : 'live';
  fg.push(nsBox(80, 470, 160, 40, 'GREEN HYD', g), nsBox(80, 530, 160, 40, 'YELLOW HYD', y));
  fg.push(nsBox(260, 650, 150, 40, 'NORMAL BRAKES', slState(n, /NORM(AL)? BRAKE|BRAKE.*NORM|BCS/, SL_SOFT)), nsBox(430, 650, 150, 40, 'ALTN BRAKES', slState(n, /ALTN BRAKE|ALTERNATE BRAKE/, SL_SOFT)), nsBox(600, 650, 150, 40, 'BRAKE ACCU', slState(n, /ACCU/, SL_SOFT)), nsBox(770, 650, 150, 40, 'ANTI SKID', slState(n, /SKID/, SL_SOFT)));
  parts.push(nsElbow([240, 490], [260, 670], g), nsElbow([240, 550], [430, 670], y));
  return nsSvg(W, H, [...parts, ...fg]);
}

function slApu(t) {
  const n = t.names;
  const S = (re) => slState(n, re, SL_SOFT);
  const parts = [], fg = [];
  fg.push(nsBox(390, 250, 220, 110, 'APU\nPW980', S(/APU(?! GEN| BLEED| FIRE| BAT| TR)/), { size: 16 }));
  fg.push(nsBox(120, 120, 160, 44, 'AIR INTAKE FLAP', S(/INTAKE|FLAP/)), nsBox(120, 200, 160, 44, 'ECB', S(/ECB|APU CTL|FADEC/)));
  fg.push(nsBox(120, 280, 160, 44, 'START MOTOR 1', S(/STARTER ?1|START MOTOR ?1/)), nsBox(120, 340, 160, 44, 'START MOTOR 2', S(/STARTER ?2|START MOTOR ?2/)));
  fg.push(nsBox(120, 440, 160, 44, 'FUEL LP VALVE', S(/APU.*(LP|FUEL)|FUEL.*APU/)), nsBox(120, 520, 160, 44, 'APU BAT', S(/APU BAT|BAT APU/)));
  fg.push(nsBox(720, 120, 170, 44, 'APU GEN A', S(/APU GEN ?(A|1)/)), nsBox(720, 200, 170, 44, 'APU GEN B', S(/APU GEN ?(B|2)/)));
  fg.push(nsBox(720, 300, 170, 44, 'BLEED VALVE', S(/APU BLEED/)), nsBox(720, 380, 170, 44, 'OIL', S(/APU OIL/)));
  fg.push(nsBox(720, 480, 170, 44, 'FIRE LOOP', S(/APU FIRE|APU.*LOOP/)), nsBox(720, 560, 170, 44, 'FIRE BOTTLE', S(/APU.*(BOTTLE|AGENT|SQUIB)/)));
  for (const y of [142, 222, 302, 362, 462, 542]) parts.push(nsElbow([280, y], [390, 305], 'live'));
  for (const y of [142, 222, 322, 402, 502, 582]) parts.push(nsElbow([610, 305], [720, y], 'live'));
  return nsSvg(1000, 640, [...parts, ...fg]);
}

function slFire(t) {
  const n = t.names;
  const fg = [];
  for (let e = 1; e <= 4; e++) {
    const x = 40 + (e - 1) * 240;
    const zone = slHits(n, new RegExp(`ENG(INE)? ?${e}.*FIRE(?!.*(DET|LOOP))`)) ? 'fail' : 'live';
    fg.push(nsBox(x, 60, 200, 60, `ENG ${e}`, zone, { size: 16 }));
    fg.push(nsBox(x, 140, 95, 40, 'LOOP A', slState(n, new RegExp(`ENG(INE)? ?${e}.*LOOP ?A`), SL_SOFT)), nsBox(x + 105, 140, 95, 40, 'LOOP B', slState(n, new RegExp(`ENG(INE)? ?${e}.*LOOP ?B`), SL_SOFT)));
    fg.push(nsBox(x, 200, 95, 40, 'BTL 1', slState(n, new RegExp(`ENG(INE)? ?${e}.*(BTL|BOTTLE|AGENT) ?1`), SL_SOFT)), nsBox(x + 105, 200, 95, 40, 'BTL 2', slState(n, new RegExp(`ENG(INE)? ?${e}.*(BTL|BOTTLE|AGENT) ?2`), SL_SOFT)));
  }
  fg.push(nsBox(40, 300, 200, 60, 'APU', slHits(n, /APU FIRE(?!.*(DET|LOOP))/) ? 'fail' : 'live', { size: 16 }), nsBox(40, 380, 95, 40, 'LOOP', slState(n, /APU.*LOOP|APU FIRE DET/, SL_SOFT)), nsBox(145, 380, 95, 40, 'BOTTLE', slState(n, /APU.*(BTL|BOTTLE|AGENT)/, SL_SOFT)));
  const cargo = [['FWD CARGO', /FWD CARGO|FWD\+AFT CARGO/], ['AFT CARGO', /AFT CARGO|FWD\+AFT CARGO/], ['BULK', /BULK/]];
  cargo.forEach(([lab, re], i) => fg.push(nsBox(280 + i * 170, 300, 150, 60, lab, slHits(n, new RegExp('SMOKE.*(' + re.source + ')|(' + re.source + ').*SMOKE')) ? 'fail' : slState(n, new RegExp('(' + re.source + ').*(DET|SDF)'), SL_SOFT))));
  fg.push(nsBox(280, 380, 490, 40, 'CARGO FIRE BOTTLES (2 DISCHARGE, 3 METERING)', slState(n, /CARGO.*(BTL|BOTTLE|AGENT)/, SL_SOFT), { size: 12 }));
  fg.push(nsBox(800, 300, 160, 60, 'AVIONICS\nSMOKE', slHits(n, /AVNCS SMOKE|AVIONICS SMOKE/) ? 'fail' : 'live'), nsBox(800, 380, 160, 40, 'LAV SMOKE', slHits(n, /LAV.*SMOKE/) ? 'fail' : 'live'));
  fg.push(nsBox(40, 470, 920, 40, 'FIRE DETECTION UNITS  FDU 1 / FDU 2  /  SDF CIDS', slState(n, /FDU|FIRE DET(ECTION)? UNIT|SDF/, SL_SOFT), { size: 12.5 }));
  return nsSvg(1000, 540, fg);
}

function slOxygen(t) {
  const n = t.names;
  const S = (re) => slState(n, re, SL_SOFT);
  const parts = [], fg = [];
  fg.push(exT(160, 40, 'CREW', { size: 15, fill: NS.greyText }), exT(700, 40, 'PASSENGERS', { size: 15, fill: NS.greyText }));
  for (let b = 1; b <= 3; b++) fg.push(nsBox(60 + (b - 1) * 110, 70, 95, 50, `BTL ${b}`, S(new RegExp(`CREW (OXY|O2).*(BTL|BOTTLE|CYL) ?${b}|CREW OXY LO PR`))));
  fg.push(nsBox(110, 170, 200, 44, 'CREW SUPPLY VALVE', S(/CREW (OXY|O2).*VALVE|CREW SUPPLY/)));
  ['CAPT', 'F/O', '3RD', '4TH', '5TH'].forEach((m, i) => fg.push(nsBox(30 + i * 62, 270, 56, 40, m, S(new RegExp(`MASK.*${m.replace('/', '.')}|${m.replace('/', '.')}.*MASK`)), { size: 11 })));
  parts.push(nsElbow([210, 120], [210, 170], 'live'));
  for (let i = 0; i < 5; i++) parts.push(nsElbow([210, 214], [58 + i * 62, 270], 'live'));
  for (let b = 0; b < 6; b++) fg.push(nsBox(480 + b * 80, 70, 70, 50, `CYL ${b + 1}`, S(/PAX (OXY|O2).*(BTL|BOTTLE|CYL|LO PR)/), { size: 12 }));
  fg.push(nsBox(560, 170, 320, 44, 'PAX HIGH / LOW PRESSURE REGULATORS', S(/PAX (OXY|O2).*(REG|PRESS)/), { size: 12 }));
  fg.push(nsBox(480, 270, 230, 50, 'MAIN DECK MASKS', S(/MAIN DECK.*MASK|PAX MASK|PAX OXY/)), nsBox(730, 270, 230, 50, 'UPPER DECK MASKS', S(/UPPER DECK.*MASK|PAX MASK|PAX OXY/)));
  fg.push(nsBox(560, 360, 320, 40, 'PAX SYS ON  /  MASK MAN ON', S(/MASK MAN|PAX SYS/), { size: 12 }));
  parts.push(nsElbow([720, 120], [720, 170], 'live'), nsElbow([720, 214], [595, 270], 'live'), nsElbow([720, 214], [845, 270], 'live'));
  return nsSvg(1000, 430, [...parts, ...fg]);
}

function slDoors(t) {
  const hl = new Set();
  for (const nm of t.names) {
    const u = nm.toUpperCase();
    for (const m of u.matchAll(/\b([MU]\d[LR])\b/g)) hl.add('D' + m[1]);
    if (/CARGO DOOR|FWD CARGO|AFT CARGO/.test(u)) hl.add(/AFT/.test(u) ? 'CAFT' : 'CFWD');
    if (/BULK/.test(u)) hl.add('CBULK');
    if (/AVNCS/.test(u)) hl.add('AVNCS');
  }
  return exSideView(hl, false);
}

function slAntiIce(t) {
  const n = t.names;
  const S = (re) => slState(n, re, SL_SOFT);
  const fg = [], parts = [];
  for (let e = 1; e <= 4; e++) fg.push(nsBox(40 + (e - 1) * 240, 60, 200, 50, `ENG ${e} NAI`, S(new RegExp(`ENG(INE)? ?${e}.*(NAI|ANTI ?ICE|A\\.ICE)`))));
  fg.push(nsBox(120, 170, 300, 50, 'WING A-ICE L', S(/WING.*(ANTI ?ICE|A\.ICE).*(L|LEFT)|WING A-ICE/)), nsBox(580, 170, 300, 50, 'WING A-ICE R', S(/WING.*(ANTI ?ICE|A\.ICE).*(R|RIGHT)|WING A-ICE/)));
  ['CAPT', 'F/O', 'STBY'].forEach((s, i) => fg.push(nsBox(120 + i * 260, 290, 240, 50, `${s} PROBES HEAT`, S(new RegExp(`${s.replace('/', '.')}.*(PROBE|PITOT|AOA|TAT|STATIC)|PROBE.*${s.replace('/', '.')}|PHC ?${i + 1}`)))));
  fg.push(nsBox(120, 380, 360, 50, 'WINDSHIELD / WINDOWS HEAT', S(/WINDSHIELD|WINDOW HEAT|WSHLD|WHC/)), nsBox(520, 380, 360, 50, 'RAIN REPELLENT / WIPERS', S(/WIPER|RAIN/)));
  fg.push(nsBox(120, 470, 760, 44, 'ICE DETECTORS 1 / 2', S(/ICE DET/)));
  parts.push(nsWire([[140, 85], [860, 85]], 'off', true));
  return nsSvg(1000, 540, [...parts, ...fg]);
}

function slUnits(sys) { return (t) => exUnits(sys, { t: t.names.join(' '), s: sys }); }

function slEngines(t) {
  const host = el('div', { class: 'sl-engs' });
  for (let e = 1; e <= 4; e++) {
    const list = t.names.map((name, i) => ({ id: i + 1, name, active: true, magnitude: 1 }));
    host.append(engineSvg({ n1: 85, n3: 96, faults: engFaults(list, e), title: `ENGINE ${e}` }));
  }
  return host;
}

(function () {
  const live = {
    ELECTRICAL: (t) => elecView(t.elec, { title: 'ELECTRICAL NETWORK', full: true, loads: elecLoadsLost(t.elec) }),
    HYDRAULICS: (t) => hydView(t.hyd, 'HYDRAULIC SYSTEMS'),
    FUEL: (t) => fuelView(t.fuel, 'FUEL SYSTEM'),
    'FLIGHT CONTROLS': (t) => fctlPlanView(t.fctl, 'FLIGHT CONTROLS'),
    'BLEED AND PACKS': (t) => bleedView(t.air, 'PNEUMATIC SYSTEM'),
    'CABIN AIR AND VENTILATION': (t) => ventView(t.air, 'VENTILATION'),
    ENGINE: slEngines,
  };
  for (const row of SYSTEM_MAPS) if (live[row[0]]) { const f = live[row[0]]; row[1] = () => slLive(f); }
  SYSTEM_MAPS.push(
    ['LANDING GEAR AND BRAKES', () => slLive(slGear)],
    ['APU', () => slLive(slApu)],
    ['FIRE PROTECTION', () => slLive(slFire)],
    ['OXYGEN', () => slLive(slOxygen)],
    ['DOORS', () => slLive(slDoors)],
    ['ANTI ICE AND RAIN', () => slLive(slAntiIce)],
    ['NAVIGATION', () => slLive(slUnits('NAV'))],
    ['SURVEILLANCE', () => slLive(slUnits('SURV'))],
    ['COMMUNICATIONS', () => slLive(slUnits('COM'))],
    ['AUTO FLIGHT', () => slLive(slUnits('AUTO FLT'))],
    ['WARNING AND RECORDING', () => slLive(slUnits('FWS'))],
  );
  const s = document.createElement('style');
  s.textContent = '.sl-live{display:flex;flex-direction:column;gap:8px;}.sl-foot{font:700 12px "JetBrains Mono",monospace;color:#8b9097;letter-spacing:0.04em;padding:4px 2px;}.sl-engs{display:grid;grid-template-columns:1fr 1fr;gap:10px;}';
  document.head.append(s);
})();

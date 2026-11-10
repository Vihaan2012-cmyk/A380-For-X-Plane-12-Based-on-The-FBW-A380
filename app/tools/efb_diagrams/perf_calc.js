const PC_G = 9.80665, PC_S = 845, PC_KT = 0.514444, PC_FT = 0.3048;
const PC_RATINGS = { GP7270: 332.44, GP7270E: 332.44, GP7277: 357.15, '970': 334.29, '972': 341.41, '972E': 341.41, '970B': 348.31, '972B': 356.81, '977': 359.33, '977B': 372.92, '980': 374.09 };
const PC_WV = { WV000: [560, 386], WV001: [510, 394], WV002: [569, 391], WV003: [510, 395], WV004: [560, 391], WV005: [560, 386], WV006: [573, 393], WV007: [490, 395], WV008: [575, 394], WV009: [510, 386], WV010: [480, 386], WV011: [575, 395], WV012: [569, 395], WV013: [492, 386], WV014: [572, 391] };
const PC_VLS = {
  '1+F': [[123, 133, 144, 154, 163, 172, 180, 188], [123, 130, 140, 150, 159, 167, 176, 183]],
  '2': [[123, 128, 138, 148, 157, 165, 173, 181], [123, 125, 135, 144, 153, 161, 169, 176]],
  '3': [[123, 124, 134, 144, 152, 161, 169, 176], [123, 123, 131, 140, 148, 156, 164, 171]],
  FULL: [[123, 123, 131, 140, 148, 157, 165, 173], [123, 123, 127, 136, 144, 152, 160, 168]],
};
const PC_LD = { '1+F': [0.074, 13.2], '2': [0.081, 12.1], '3': [0.092, 10.6] };
const PC_TO_COND = {
  DRY: { mu: 0.42, roll: 0.015, wet: false, rev: false },
  WET: { mu: 0.25, roll: 0.02, wet: true, rev: true },
  'WATER 6 MM': { mu: 0.15, roll: 0.045, wet: true, rev: true },
  'WATER 13 MM': { mu: 0.13, roll: 0.07, wet: true, rev: true },
  'SLUSH 6 MM': { mu: 0.14, roll: 0.05, wet: true, rev: true },
  'SLUSH 13 MM': { mu: 0.12, roll: 0.08, wet: true, rev: true },
  'COMPACTED SNOW': { mu: 0.2, roll: 0.02, wet: true, rev: true },
  'WET SNOW 5 MM': { mu: 0.17, roll: 0.035, wet: true, rev: true },
  'WET SNOW 15 MM': { mu: 0.16, roll: 0.05, wet: true, rev: true },
  'DRY SNOW 10 MM': { mu: 0.2, roll: 0.035, wet: true, rev: true },
  'DRY SNOW 100 MM': { mu: 0.17, roll: 0.09, wet: true, rev: true },
};
const PC_LDG_COND = { '6 DRY': 0.38, '5 GOOD': 0.28, '4 GOOD/MEDIUM': 0.22, '3 MEDIUM': 0.17, '2 MEDIUM/POOR': 0.12, '1 POOR': 0.08 };
const PC_AUTOBRAKE = { LO: 1.7, '2': 2.2, '3': 3.0, HI: 3.5, MAX: 99 };
const PC_LINEUP = { '0': [0, 0], '90': [24, 45], '180': [35, 56] };

let pcTab = 'to';
let pcCal = { thrust: 1, roll: 1, brake: 1, takeoffs: 0, landings: 0 };

async function pcLoadCal() {
  try {
    const r = await (await fetch(PLUGIN_ORIGIN + '/perf/cal')).json();
    const f = (r && r.factors) || {};
    const c = (v) => (Number.isFinite(v) ? Math.min(1.3, Math.max(0.7, v)) : 1);
    pcCal = { thrust: c(f.thrust), roll: c(f.roll), brake: c(f.brake), takeoffs: f.takeoffs || 0, landings: f.landings || 0 };
  } catch {}
}

function pcCalText() {
  return pcCal.takeoffs + pcCal.landings === 0
    ? 'X-Plane calibration: none yet. Each take-off and maximum-braking landing you fly tunes these numbers to the X-Plane flight model.'
    : `X-Plane calibration from ${pcCal.takeoffs} take-off(s) and ${pcCal.landings} landing(s): thrust x${pcCal.thrust.toFixed(2)}, ground roll x${pcCal.roll.toFixed(2)}, braking x${pcCal.brake.toFixed(2)}.`;
}
let pcAirport = { to: null, ldg: null };
let pcOut = { to: null, ldg: null };
const pcIn = {
  to: { icao: '', rwy: '', tora: '', toda: '', asda: '', slope: '0', lineup: '90', wdir: '', wspd: '0', oat: '15', qnh: '1013', cond: 'DRY', tow: '', cg: '33', conf: 'AUTO', thrust: 'FLEX', drt: 'D01', packs: 'ON', ai: 'OFF' },
  ldg: { icao: '', rwy: '', lda: '', slope: '0', wdir: '', wspd: '0', oat: '15', qnh: '1013', cond: '6 DRY', lw: '', cg: '33', conf: 'FULL', ab: 'ALL', rev: 'ON', autoland: 'NO', ice: 'NO' },
};
const PC_INITIAL = JSON.parse(JSON.stringify(pcIn));
const PC_OPERATOR = [['to', 'conf', 'xphfbw.opToConf'], ['to', 'thrust', 'xphfbw.opToThrust'], ['to', 'packs', 'xphfbw.opToPacks'], ['ldg', 'conf', 'xphfbw.opLdgConf'], ['ldg', 'ab', 'xphfbw.opLdgAutobrake'], ['ldg', 'rev', 'xphfbw.opLdgReversers']];
const pcOpApplied = {};
function pcOperatorDefaults() {
  for (const [kind, field, key] of PC_OPERATOR) {
    const want = typeof settings === 'object' && settings[key] != null && settings[key] !== '' ? String(settings[key]) : (typeof DEFAULTS === 'object' && DEFAULTS[key] != null ? String(DEFAULTS[key]) : null);
    if (want === null) continue;
    const prev = pcOpApplied[kind + field] ?? PC_INITIAL[kind][field];
    if (pcIn[kind][field] === prev && prev !== want) pcIn[kind][field] = want;
    if (pcIn[kind][field] === want) pcOpApplied[kind + field] = want;
  }
}

function pcNum(v, d = 0) { const n = parseFloat(String(v).replace(',', '.')); return Number.isFinite(n) ? n : d; }

function pcAtmos(elevFt, qnh, oat) {
  const pa = elevFt + (1013.25 - qnh) * 27.3;
  const delta = Math.pow(Math.max(0.2, 1 - 6.8756e-6 * pa), 5.2559);
  const isa = 15 - 1.98 * pa / 1000;
  const sigma = delta * 288.15 / (oat + 273.15);
  return { pa, delta, isa, sigma, rho: 1.225 * sigma };
}

function pcVls(conf, tons, cg) {
  const t = PC_VLS[conf];
  const at = (row) => {
    const x = Math.max(350, Math.min(600, tons));
    const i = Math.min(6, Math.floor((x - 250) / 50));
    const v = row[i] + (row[i + 1] - row[i]) * ((x - 250) / 50 - i);
    return tons >= 350 ? v : v * Math.sqrt(tons / 350);
  };
  const k = Math.max(0, Math.min(1, (cg - 29) / 14));
  return at(t[0]) * (1 - k) + at(t[1]) * k;
}

function pcVmcg(pa) { return 118 - 0.6 * pa / 1000; }
function pcVmca(pa) { return 123 - 0.8 * pa / 1000; }

function pcEngineThrust(rating, atm, tempEff, packs, ai) {
  const flat = atm.isa + 15;
  const temp = tempEff <= flat ? 1 : Math.max(0.35, 1 - 0.0095 * (tempEff - flat));
  const bleed = (packs ? 0.988 : 1) * (ai === 'ENG+WING' ? 0.975 : ai === 'ENG' ? 0.99 : 1);
  return rating * 1000 * Math.pow(atm.delta, 0.8) * temp * bleed * pcCal.thrust;
}

function pcThrustAt(f0, tasKt) { return f0 * (1 - 0.00165 * tasKt + 1.3e-6 * tasKt * tasKt); }

function pcGround(m, cd, mu, slope, atm, f0, engines, v0, v1, hw, dt = 0.05) {
  let v = v0, x = 0, t = 0;
  const sin = Math.sin(Math.atan(slope / 100));
  while (v < v1 && t < 200) {
    const q = 0.5 * atm.rho * v * v;
    const lift = q * PC_S * 0.45;
    const a = (engines * pcThrustAt(f0, v / PC_KT) - q * PC_S * cd - mu * Math.max(0, m * PC_G - lift) - m * PC_G * sin) / m;
    if (a <= 0.01) return { x: Infinity, t: Infinity };
    v += a * dt; x += (v - hw) * dt; t += dt;
  }
  return { x: x * pcCal.roll, t };
}

function pcAir(m, conf, atm, f0, engines, vlof, v2, hw, screenFt) {
  const [, ld] = PC_LD[conf];
  const h = screenFt * PC_FT;
  const v = (vlof + v2) / 2;
  const T = engines * pcThrustAt(f0, v / PC_KT) - (engines < 4 ? 0.012 * m * PC_G : 0);
  const gamma = T / (m * PC_G) - 1 / (ld - 3.2);
  if (gamma <= 0.002) return Infinity;
  return (h / (0.5 * gamma)) * (v - hw) / v;
}

function pcTakeoffCase(p, tempEff, conf, v1Fixed) {
  const atm = p.atm;
  const m = p.tow * 1000;
  const f0 = pcEngineThrust(p.rating, atm, tempEff, p.packs, p.ai) * (1 - (p.deratePct || 0) / 100);
  const [cd, ld] = PC_LD[conf];
  const vsCas = pcVls(conf, p.tow, p.cg) / 1.23;
  const toTas = (cas) => cas / Math.sqrt(atm.sigma);
  const vmcRatio = Math.sqrt(1 - (p.deratePct || 0) / 100);
  const vmcg = pcVmcg(atm.pa) * vmcRatio, vmca = pcVmca(atm.pa) * vmcRatio;
  const v2 = Math.ceil(Math.max(1.13 * vsCas, 1.1 * vmca));
  const vr = Math.ceil(Math.max(1.05 * vmca, v2 - 6));
  const hw = p.hw >= 0 ? p.hw * 0.5 : p.hw * 1.5;
  const hwM = hw * PC_KT;
  const vrT = toTas(vr) * PC_KT, v2T = toTas(v2) * PC_KT, vlofT = toTas(vr + 4) * PC_KT;
  const gnd = pcGround(m, cd, p.c.roll, p.slope, atm, f0, 4, 0, vrT, hwM);
  const rot = pcGround(m, cd * 1.15, p.c.roll * 0.4, p.slope, atm, f0, 4, vrT, vlofT, hwM);
  const todAeo = 1.15 * (gnd.x + rot.x + pcAir(m, conf, atm, f0, 4, vlofT, v2T * 1.04, hwM, 35));
  const grad2 = (3 * pcThrustAt(f0, toTas(v2)) - 0.012 * m * PC_G) / (m * PC_G) - 1 / ld;
  const vlofGs = toTas(vr + 4) - hw;
  const keBrake = 0.5 * m * Math.pow(v1Fixed ? v1Fixed * PC_KT : vrT, 2);
  const caseFor = (v1) => {
    const v1T = toTas(v1) * PC_KT;
    const vefT = Math.max(1, toTas(v1 - 3) * PC_KT);
    const a1 = pcGround(m, cd, p.c.roll, p.slope, atm, f0, 4, 0, vefT, hwM);
    const a2 = pcGround(m, cd, p.c.roll, p.slope, atm, f0, 3, vefT, vrT, hwM);
    const a3 = pcGround(m, cd * 1.15, p.c.roll * 0.4, p.slope, atm, f0, 3, vrT, toTas(vr + 3) * PC_KT, hwM);
    const todOei = a1.x + a2.x + a3.x + pcAir(m, conf, atm, f0, 3, toTas(vr + 3) * PC_KT, v2T, hwM, p.c.wet ? 15 : 35);
    const acc = pcGround(m, cd, p.c.roll, p.slope, atm, f0, 4, 0, v1T, hwM);
    let v = v1T, x = acc.x + 2 * (v1T - hwM);
    const sin = Math.sin(Math.atan(p.slope / 100));
    for (let t = 0; v > 0.5 && t < 120; t += 0.05) {
      const q = 0.5 * atm.rho * v * v;
      const rev = p.c.rev ? 2 * 0.18 * pcThrustAt(f0, v / PC_KT) * Math.min(1, v / (60 * PC_KT)) : 0;
      const dec = p.c.mu * pcCal.brake * PC_G * Math.max(0.2, 1 - (q * PC_S * 0.2) / (m * PC_G)) + (q * PC_S * (cd + 0.06) + rev - 4 * 0.06 * f0) / m + PC_G * sin;
      v -= Math.max(0.3, dec) * 0.05;
      x += Math.max(0, v - hwM) * 0.05;
    }
    return { v1, todOei, asd: x, v1x: acc.x, ke: 0.5 * m * v1T * v1T };
  };
  const toda = p.toda - p.lineup[0], asda = p.asda - p.lineup[1];
  let best = null;
  const lo = Math.ceil(vmcg), hi = vr;
  for (let v1 = v1Fixed || lo; v1 <= (v1Fixed || hi); v1++) {
    const c = caseFor(v1);
    const margin = Math.min(toda - c.todOei, asda - c.asd);
    if (!best || margin > best.margin) best = { ...c, margin };
  }
  const limits = [];
  if (todAeo > toda) limits.push('RUNWAY (TOD AEO)');
  if (best.todOei > toda) limits.push('RUNWAY (TOD OEI)');
  if (best.asd > asda) limits.push('RUNWAY (ASD)');
  if (grad2 < 0.03) limits.push('SECOND SEGMENT');
  if (vlofGs > 204) limits.push('TYRE SPEED');
  if (best.ke > 16 * 130e6) limits.push('BRAKE ENERGY');
  if (vr < vmcg) limits.push('VMCG');
  if (p.c.wet && !p.dryCheck) {
    const dry = pcTakeoffCase({ ...p, c: PC_TO_COND.DRY, dryCheck: true }, tempEff, conf);
    for (const l of dry.limits) if (!limits.includes(l)) limits.push(l + ' (DRY)');
  }
  return { ok: !limits.length, limits, v1: best.v1, v1x: best.v1x, vr, v2, conf, todAeo, todOei: best.todOei, asd: best.asd, toda, asda, grad2, vlofGs, vs: vsCas, tempEff, f0, keBrake };
}

function pcTakeoff(p) {
  const confs = p.conf === 'AUTO' ? ['1+F', '2', '3'] : [p.conf];
  const oat = p.oat;
  const flat = p.atm.isa + 15, tmax = p.atm.isa + 70;
  let best = null;
  for (const conf of confs) {
    const toga = pcTakeoffCase(p, oat, conf);
    if (!toga.ok) { if (!best || (!best.ok && toga.todAeo < best.todAeo)) best = { ...toga, flex: null }; continue; }
    let flex = null;
    if (p.thrust === 'FLEX') {
      const minFlex = Math.max(oat + 1, Math.ceil(flat));
      const fogaF0 = pcEngineThrust(p.rating, p.atm, oat, p.packs, p.ai);
      let lo = minFlex, hi = Math.floor(tmax);
      const usable = (t) => pcEngineThrust(p.rating, p.atm, t, p.packs, p.ai) >= 0.75 * fogaF0 && pcTakeoffCase(p, t, conf).ok;
      if (lo <= hi && usable(lo)) {
        while (lo < hi) { const mid = Math.ceil((lo + hi + 1) / 2); if (usable(mid)) lo = mid; else hi = mid - 1; }
        flex = lo;
      }
    }
    const r = flex ? { ...pcTakeoffCase(p, flex, conf), flex } : { ...toga, flex: null };
    const score = (x) => (x.ok ? 1000 + (x.flex || 0) * 10 + (x.conf === '2' ? 0.5 : 0) : -1);
    if (!best || score(r) > score(best)) best = r;
  }
  best.trim = pcTrim(p.cg);
  best.limitTow = pcLimitWeight(p, best.conf);
  return best;
}

function pcLimitWeight(p, conf) {
  let lo = 250, hi = 620;
  for (let i = 0; i < 18; i++) { const mid = (lo + hi) / 2; if (pcTakeoffCase({ ...p, tow: mid }, p.oat, conf).ok) lo = mid; else hi = mid; }
  return lo;
}

function pcTrim(cg) {
  if (cg < 29) return 5.8;
  if (cg <= 34.5) return 5.8 - ((cg - 29) * 56) / 275;
  if (cg <= 43) return 4.68 - ((cg - 34.5) * 243) / 425;
  return -0.18;
}

function pcLanding(p) {
  const atm = p.atm;
  const m = p.lw * 1000;
  const vls = Math.max(pcVls(p.conf, p.lw, p.cg), 123) + (p.ice ? 5 : 0);
  const windAdd = Math.min(15, Math.max(5, p.hw / 3)) + (p.autoland ? 0 : 0);
  const vapp = Math.round(vls + windAdd);
  const hwF = p.hw >= 0 ? p.hw * 0.5 : p.hw * 1.5;
  const tas = (cas) => cas / Math.sqrt(atm.sigma);
  const gsTd = (tas(vapp - 5) - hwF) * PC_KT;
  const air = 430 * ((tas(vapp) - hwF) / 140);
  const mu = p.mu * pcCal.brake;
  const sin = Math.sin(Math.atan(p.slope / 100));
  const run = (target) => {
    let v = tas(vapp - 5) * PC_KT, x = air + 2.5 * gsTd;
    for (let t = 0; v > 0.5 && t < 200; t += 0.05) {
      const q = 0.5 * atm.rho * v * v;
      const aero = (q * PC_S * 0.16) / m;
      const rev = p.rev ? (2 * 0.2 * 350e3 * atm.delta * Math.min(1, Math.max(0, (v / PC_KT - 70) / 70))) / m : 0;
      const brakeMax = mu * PC_G * Math.max(0.3, 1 - (q * PC_S * 0.15) / (m * PC_G));
      const dec = Math.min(target, brakeMax + aero + rev) + PC_G * sin;
      v -= Math.max(0.3, dec) * 0.05;
      x += Math.max(0, v - hwF * PC_KT) * 0.05;
    }
    return x;
  };
  const modes = p.ab === 'ALL' ? ['LO', '2', '3', 'HI', 'MAX'] : [p.ab];
  const rows = modes.map((ab) => {
    const d = run(PC_AUTOBRAKE[ab]);
    return { ab, actual: d, inflight: d * 1.15, dispatch: d / 0.6 * (mu < 0.3 ? 1.15 : 1) };
  });
  return { vls: Math.round(vls), vapp, rows, lda: p.lda, mlwOver: p.lw > p.mlw };
}

function pcParams(kind) {
  const i = pcIn[kind];
  const ap = pcAirport[kind];
  const rw = ap && ap.runways ? ap.runways.find((r) => r.id === i.rwy) : null;
  const elev = ap ? ap.elevation_ft : 0;
  const atm = pcAtmos(elev, pcNum(i.qnh, 1013), pcNum(i.oat, 15));
  const hdg = rw ? rw.heading : 0;
  const wdir = i.wdir === '' ? hdg : pcNum(i.wdir);
  const hw = pcNum(i.wspd) * Math.cos(((wdir - hdg) * Math.PI) / 180);
  const xw = pcNum(i.wspd) * Math.sin(((wdir - hdg) * Math.PI) / 180);
  const s = typeof settings === 'object' && settings ? settings : {};
  const derates = String(s['xphfbw.takeoffDerates'] || '4,8,12,16,20').split(',').map((x) => parseFloat(x) || 0);
  const wv = PC_WV[s['xphfbw.weightVariant'] || 'WV003'] || PC_WV.WV003;
  const rating = PC_RATINGS[s['xphfbw.engineRating'] || '972B'] || PC_RATINGS['972B'];
  if (kind === 'to') {
    return {
      atm, hw, xw, rw, rating, mtow: wv[0], oat: pcNum(i.oat, 15), tow: pcNum(i.tow), cg: pcNum(i.cg, 33), conf: i.conf, thrust: i.thrust,
      deratePct: i.thrust === 'DERATE' ? (derates[+i.drt.slice(1) - 1] || 0) : 0,
      packs: i.packs === 'ON', ai: i.ai, slope: pcNum(i.slope), c: PC_TO_COND[i.cond] || PC_TO_COND.DRY,
      tora: pcNum(i.tora, rw ? rw.tora_m : 0), toda: pcNum(i.toda, rw ? rw.toda_m : 0), asda: pcNum(i.asda, rw ? rw.asda_m : 0), lineup: PC_LINEUP[i.lineup] || PC_LINEUP['90'],
    };
  }
  return {
    atm, hw, xw, rw, mlw: wv[1], lw: pcNum(i.lw), cg: pcNum(i.cg, 33), conf: i.conf, ab: i.ab, rev: i.rev === 'ON', autoland: i.autoland === 'YES', ice: i.ice === 'YES',
    slope: pcNum(i.slope), mu: PC_LDG_COND[i.cond] || 0.42, lda: pcNum(i.lda, rw ? rw.lda_m : 0),
  };
}

async function pcLoadAirport(kind, icao) {
  icao = (icao || '').trim().toUpperCase();
  pcIn[kind].icao = icao;
  pcAirport[kind] = null;
  if (icao.length < 3) { pcRender(); return; }
  try {
    const r = await (await fetch('/api/runways?icao=' + encodeURIComponent(icao))).json();
    pcAirport[kind] = r.error ? { error: r.error } : r;
    if (!r.error && r.runways.length && !r.runways.some((x) => x.id === pcIn[kind].rwy)) pcIn[kind].rwy = r.runways[0].id;
  } catch { pcAirport[kind] = { error: 'Runway data needs X-Plane running.' }; }
  pcRender();
}

function pcMetar(raw, into) {
  if (!raw) return;
  const s = [].concat(raw).join(' ');
  const w = s.match(/\b(\d{3}|VRB)(\d{2,3})(?:G\d{2,3})?KT\b/);
  if (w) { into.wdir = w[1] === 'VRB' ? '' : String(+w[1]); into.wspd = String(+w[2]); }
  const t = s.match(/\s(M?\d{2})\/(M?\d{2})?\s/);
  if (t) into.oat = String(+t[1].replace('M', '-'));
  const q = s.match(/\bQ(\d{4})\b/);
  const a = s.match(/\bA(\d{4})\b/);
  if (q) into.qnh = String(+q[1]);
  else if (a) into.qnh = String(Math.round(+a[1] / 100 * 33.8639));
}

async function pcFromMetar(kind) {
  const icao = (pcIn[kind].icao || '').trim().toUpperCase();
  if (icao.length !== 4) { show('Enter the airport first'); return; }
  let j;
  try { j = await (await fetch(`${PLUGIN_ORIGIN}/wx/metar/${encodeURIComponent(icao)}?source=vatsim`)).json(); } catch { show('METAR service not reachable'); return; }
  if (!j || !j.metar) { show(`No METAR for ${icao}`); return; }
  pcMetar(j.metar, pcIn[kind]);
  const wet = /(^|\s)[-+]?(VC)?(SH|TS)?(RA|DZ)/.test(j.metar);
  if (wet && kind === 'to' && pcIn.to.cond === 'DRY') pcIn.to.cond = 'WET';
  if (wet && kind === 'ldg' && pcIn.ldg.cond === '6 DRY') pcIn.ldg.cond = '5 GOOD';
  show(`${icao} ${j.metar.split(' ').slice(1, 3).join(' ')}: wind, OAT and QNH set${wet ? ', runway wet' : ''}`);
  pcRender();
}

function pcFromOfp() {
  const o = typeof lastOfp !== 'undefined' ? lastOfp : null;
  if (!o) return false;
  const kg = !o.params || /kg/i.test(o.params.units || 'kgs');
  const t = (v) => (v == null ? '' : String(Math.round((kg ? +v : +v * 0.453592) / 100) / 10));
  if (o.origin) { pcIn.to.icao = o.origin.icao_code || ''; if (o.origin.plan_rwy) pcIn.to.rwy = o.origin.plan_rwy; }
  if (o.destination) { pcIn.ldg.icao = o.destination.icao_code || ''; if (o.destination.plan_rwy) pcIn.ldg.rwy = o.destination.plan_rwy; }
  if (o.weights) { pcIn.to.tow = t(o.weights.est_tow); pcIn.ldg.lw = t(o.weights.est_ldw); }
  if (o.weather) { pcMetar(o.weather.orig_metar, pcIn.to); pcMetar(o.weather.dest_metar, pcIn.ldg); }
  return true;
}

async function pcFromAircraft(kind) {
  const names = ['A32NX_AIRFRAME_GW', 'A32NX_AIRFRAME_GW_CG_PERCENT_MAC', 'AMBIENT TEMPERATURE', 'SEA LEVEL PRESSURE', 'AMBIENT WIND DIRECTION', 'AMBIENT WIND VELOCITY'];
  try {
    const r = await studyGet('/vars?names=' + names.map(encodeURIComponent).join(','));
    const v = (r && r.vars) || {};
    const i = pcIn[kind];
    if (+v.A32NX_AIRFRAME_GW > 1000) { const t = String(Math.round(+v.A32NX_AIRFRAME_GW / 100) / 10); if (kind === 'to') i.tow = t; else i.lw = t; }
    if (+v.A32NX_AIRFRAME_GW_CG_PERCENT_MAC > 0) i.cg = String(Math.round(+v.A32NX_AIRFRAME_GW_CG_PERCENT_MAC * 10) / 10);
    if (v['AMBIENT TEMPERATURE'] != null) i.oat = String(Math.round(+v['AMBIENT TEMPERATURE']));
    if (+v['SEA LEVEL PRESSURE'] > 800) i.qnh = String(Math.round(+v['SEA LEVEL PRESSURE']));
    if (v['AMBIENT WIND VELOCITY'] != null) { i.wspd = String(Math.round(+v['AMBIENT WIND VELOCITY'])); i.wdir = String(Math.round(+v['AMBIENT WIND DIRECTION'] || 0)); }
  } catch { /* offline */ }
  pcRender();
}

function pcField(kind, key, label, opts, unit) {
  const i = pcIn[kind];
  let input;
  if (Array.isArray(opts)) {
    input = el('div', { class: 'pc-seg' });
    for (const o of opts) {
      const b = el('button', { type: 'button', class: 'pc-opt' + (String(i[key]) === String(o) ? ' on' : ''), text: o });
      b.addEventListener('click', () => { i[key] = String(o); pcOut[kind] = null; pcRender(); });
      input.append(b);
    }
  } else if (opts && opts.select) {
    input = el('select', { class: 'pc-in' });
    for (const o of opts.select) { const op = el('option', { value: o, text: o }); if (o === i[key]) op.selected = true; input.append(op); }
    input.addEventListener('change', () => { i[key] = input.value; pcOut[kind] = null; pcRender(); });
  } else {
    input = el('input', { type: 'text', class: 'pc-in', value: i[key] ?? '', placeholder: opts && opts.ph ? opts.ph : '' });
    input.addEventListener('change', () => {
      i[key] = input.value.trim();
      pcOut[kind] = null;
      if (key === 'icao') pcLoadAirport(kind, input.value); else pcRender();
    });
  }
  return el('label', { class: 'pc-f' }, el('span', { class: 'pc-l', text: label }), el('span', { class: 'pc-row' }, input, unit ? el('span', { class: 'pc-u', text: unit }) : ''));
}

function pcRunwayField(kind) {
  const ap = pcAirport[kind];
  const i = pcIn[kind];
  if (!ap || ap.error || !ap.runways) return el('label', { class: 'pc-f' }, el('span', { class: 'pc-l', text: 'Runway' }), el('span', { class: 'pc-note', text: ap && ap.error ? ap.error : 'Enter an airport' }));
  return pcField(kind, 'rwy', 'Runway', { select: ap.runways.map((r) => r.id) });
}

function pcRunwayStrip(len, marks) {
  const W = 760, x0 = 10, w = W - 20;
  const sx = (m) => x0 + Math.max(0, Math.min(1, m / len)) * w;
  const P = [`<svg viewBox="0 0 ${W} 86" class="pc-strip"><rect x="${x0}" y="30" width="${w}" height="26" rx="3" fill="#2b2e33" stroke="#8a8f96" stroke-width="1.4"/>`];
  for (let k = 0; k < 14; k++) P.push(`<rect x="${x0 + 30 + k * (w - 60) / 14}" y="42" width="${(w - 60) / 28}" height="2" fill="#8a8f96"/>`);
  for (const [m, label, color, up] of marks) {
    if (!Number.isFinite(m)) continue;
    const x = sx(m);
    P.push(`<line x1="${x}" y1="${up ? 18 : 30}" x2="${x}" y2="${up ? 56 : 68}" stroke="${color}" stroke-width="2.4"/>`);
    const anchor = x < 60 ? 'start' : x > W - 60 ? 'end' : 'middle';
    P.push(`<text x="${anchor === 'start' ? Math.max(2, x - 4) : anchor === 'end' ? Math.min(W - 2, x + 4) : x}" y="${up ? 13 : 82}" text-anchor="${anchor}" fill="${color}" font-size="12.5" font-weight="700" font-family="Manrope, Segoe UI">${label}</text>`);
  }
  P.push('</svg>');
  const d = el('div', { class: 'pc-stripbox' });
  d.innerHTML = P.join('');
  return d;
}

function pcBig(label, value, cls) { return el('div', { class: 'pc-big' }, el('div', { class: 'pc-bl', text: label }), el('div', { class: 'pc-bv' + (cls ? ' ' + cls : ''), text: value })); }

function pcKv(rows) {
  const t = el('div', { class: 'pc-kv' });
  for (const [k, v, cls] of rows) t.append(el('span', { class: 'pc-k', text: k }), el('span', { class: 'pc-v' + (cls ? ' ' + cls : ''), text: v }));
  return t;
}

function pcTakeoffView(c) {
  const p = pcParams('to');
  const inputs = el('div', { class: 'pc-card pc-inputs' },
    el('div', { class: 'pc-h', text: 'RUNWAY' }),
    el('div', { class: 'pc-grid' }, pcField('to', 'icao', 'Airport', { ph: 'ICAO' }), pcRunwayField('to'), pcField('to', 'tora', 'TORA override', { ph: p.rw ? String(p.rw.tora_m) : '' }, 'm'), pcField('to', 'slope', 'Slope', null, '%')),
    pcField('to', 'lineup', 'Line-up angle', ['0', '90', '180']),
    pcField('to', 'cond', 'Runway condition', { select: Object.keys(PC_TO_COND) }),
    el('div', { class: 'pc-h', text: 'CONDITIONS' }),
    el('div', { class: 'pc-grid' }, pcField('to', 'wdir', 'Wind dir', { ph: 'calm' }, '°'), pcField('to', 'wspd', 'Wind speed', null, 'kt'), pcField('to', 'oat', 'OAT', null, '°C'), pcField('to', 'qnh', 'QNH', null, 'hPa')),
    el('div', { class: 'pc-h', text: 'AIRCRAFT' }),
    el('div', { class: 'pc-grid' }, pcField('to', 'tow', 'TOW', { ph: 'tonnes' }, 't'), pcField('to', 'cg', 'CG', null, '% MAC')),
    pcField('to', 'conf', 'Config', ['AUTO', '1+F', '2', '3']),
    pcField('to', 'thrust', 'Thrust', ['FLEX', 'TOGA', 'DERATE']),
    ...(pcIn.to.thrust === 'DERATE' ? [pcField('to', 'drt', 'Derate level', ['D01', 'D02', 'D03', 'D04', 'D05'])] : []),
    el('div', { class: 'pc-grid' }, pcField('to', 'packs', 'Packs', ['ON', 'OFF']), pcField('to', 'ai', 'Anti-ice', ['OFF', 'ENG', 'ENG+WING'])));
  const calc = el('button', { type: 'button', class: 'pc-btn pc-go', text: 'CALCULATE' });
  calc.addEventListener('click', () => {
    const q = pcParams('to');
    if (!q.rw && !pcNum(pcIn.to.tora)) { pcOut.to = { error: 'Pick an airport and runway, or enter TORA.' }; pcRender(); return; }
    if (!q.tow) { pcOut.to = { error: 'Enter the takeoff weight.' }; pcRender(); return; }
    if (!q.rw) { q.toda = q.toda || q.tora; q.asda = q.asda || q.tora; }
    pcOut.to = { r: pcTakeoff(q), p: q };
    const flaps = { '1+F': 1, '2': 2, '3': 3 }[pcOut.to.r && pcOut.to.r.conf];
    if (flaps && !FIXTURE) studyPost('/study/action', { kind: 'copilotTakeoffFlaps', index: flaps }).catch(() => {});
    pcRender();
  });
  const ofp = el('button', { type: 'button', class: 'pc-btn', text: 'FROM OFP' });
  ofp.addEventListener('click', () => { if (pcFromOfp()) { pcLoadAirport('to', pcIn.to.icao); pcLoadAirport('ldg', pcIn.ldg.icao); } });
  const live = el('button', { type: 'button', class: 'pc-btn', text: 'FROM AIRCRAFT' });
  live.addEventListener('click', () => pcFromAircraft('to'));
  const wx = el('button', { type: 'button', class: 'pc-btn', text: 'FROM METAR' });
  wx.addEventListener('click', () => pcFromMetar('to'));
  inputs.append(el('div', { class: 'pc-actions' }, ofp, live, wx, calc));
  const out = el('div', { class: 'pc-card pc-out' });
  const o = pcOut.to;
  if (!o) out.append(el('div', { class: 'pc-empty', text: 'Enter the runway, conditions and weight, then CALCULATE.' }));
  else if (o.error) out.append(el('div', { class: 'pc-empty pc-bad', text: o.error }));
  else {
    const r = o.r, q = o.p;
    out.append(el('div', { class: 'pc-h', text: `${pcIn.to.icao} ${pcIn.to.rwy}  •  CONF ${r.conf}` }));
    out.append(el('div', { class: 'pc-bigs' }, pcBig('V1', String(r.v1)), pcBig('VR', String(r.vr)), pcBig('V2', String(r.v2)), pcBig(r.flex ? 'FLEX' : 'THRUST', r.flex ? String(r.flex) : q.deratePct ? pcIn.to.drt : 'TOGA', r.ok ? '' : 'pc-bad'), pcBig('THS', (r.trim >= 0 ? 'UP ' : 'DN ') + Math.abs(r.trim).toFixed(1))));
    if (!r.ok) out.append(el('div', { class: 'pc-warn', text: 'NOT PERMITTED: ' + r.limits.join(', ') + `. Max TOW for this runway ${r.limitTow.toFixed(1)} t.` }));
    if (q.tow > q.mtow) out.append(el('div', { class: 'pc-warn', text: `TOW above structural MTOW (${q.mtow} t).` }));
    out.append(pcRunwayStrip(Math.max(q.toda, q.asda), [[q.lineup[0], 'LINE-UP', '#9aa0a6', true], [r.v1x, 'V1', '#3fb0ff', true], [r.asd, 'ASD', '#f5a623', false], [r.todOei, 'TOD OEI', '#ff4fd8', false], [r.todAeo, 'TOD', '#4cd964', true]]));
    out.append(pcKv([
      ['TODA / TOD AEO (x1.15)', `${Math.round(r.toda)} / ${Math.round(r.todAeo)} m`, r.todAeo > r.toda ? 'pc-bad' : 'pc-good'],
      ['TODA / TOD OEI', `${Math.round(r.toda)} / ${Math.round(r.todOei)} m`, r.todOei > r.toda ? 'pc-bad' : 'pc-good'],
      ['ASDA / ASD', `${Math.round(r.asda)} / ${Math.round(r.asd)} m`, r.asd > r.asda ? 'pc-bad' : 'pc-good'],
      ['2nd segment gradient', `${(r.grad2 * 100).toFixed(1)} % (min 3.0)`, r.grad2 < 0.03 ? 'pc-bad' : 'pc-good'],
      ['Lift-off ground speed', `${Math.round(r.vlofGs)} kt (tyres 204)`, r.vlofGs > 204 ? 'pc-bad' : ''],
      ['Runway limit TOW', `${r.limitTow.toFixed(1)} t`],
      ['Head / cross wind', `${Math.round(q.hw)} / ${Math.abs(Math.round(q.xw))} kt`, Math.abs(q.xw) > 35 ? 'pc-bad' : ''],
      ['Pressure altitude', `${Math.round(q.atm.pa)} ft`],
      ['Thrust per engine', `${(r.f0 / 1000).toFixed(0)} kN static`],
    ]));
  }
  c.append(el('div', { class: 'pc-wrap' }, inputs, out));
}

function pcLandingView(c) {
  const p = pcParams('ldg');
  const inputs = el('div', { class: 'pc-card pc-inputs' },
    el('div', { class: 'pc-h', text: 'RUNWAY' }),
    el('div', { class: 'pc-grid' }, pcField('ldg', 'icao', 'Airport', { ph: 'ICAO' }), pcRunwayField('ldg'), pcField('ldg', 'lda', 'LDA override', { ph: p.rw ? String(p.rw.lda_m) : '' }, 'm'), pcField('ldg', 'slope', 'Slope', null, '%')),
    pcField('ldg', 'cond', 'Runway condition code', { select: Object.keys(PC_LDG_COND) }),
    el('div', { class: 'pc-h', text: 'CONDITIONS' }),
    el('div', { class: 'pc-grid' }, pcField('ldg', 'wdir', 'Wind dir', { ph: 'calm' }, '°'), pcField('ldg', 'wspd', 'Wind speed', null, 'kt'), pcField('ldg', 'oat', 'OAT', null, '°C'), pcField('ldg', 'qnh', 'QNH', null, 'hPa')),
    el('div', { class: 'pc-h', text: 'AIRCRAFT' }),
    el('div', { class: 'pc-grid' }, pcField('ldg', 'lw', 'Landing weight', { ph: 'tonnes' }, 't'), pcField('ldg', 'cg', 'CG', null, '% MAC')),
    pcField('ldg', 'conf', 'Config', ['3', 'FULL']),
    pcField('ldg', 'ab', 'Autobrake', ['ALL', 'LO', '2', '3', 'HI', 'MAX']),
    el('div', { class: 'pc-grid' }, pcField('ldg', 'rev', 'Reversers', ['ON', 'OFF']), pcField('ldg', 'autoland', 'Autoland', ['NO', 'YES']), pcField('ldg', 'ice', 'Ice accretion', ['NO', 'YES'])));
  const calc = el('button', { type: 'button', class: 'pc-btn pc-go', text: 'CALCULATE' });
  calc.addEventListener('click', () => {
    const q = pcParams('ldg');
    if (!q.lda) { pcOut.ldg = { error: 'Pick an airport and runway, or enter LDA.' }; pcRender(); return; }
    if (!q.lw) { pcOut.ldg = { error: 'Enter the landing weight.' }; pcRender(); return; }
    pcOut.ldg = { r: pcLanding(q), p: q };
    pcRender();
  });
  const ofp = el('button', { type: 'button', class: 'pc-btn', text: 'FROM OFP' });
  ofp.addEventListener('click', () => { if (pcFromOfp()) { pcLoadAirport('to', pcIn.to.icao); pcLoadAirport('ldg', pcIn.ldg.icao); } });
  const live = el('button', { type: 'button', class: 'pc-btn', text: 'FROM AIRCRAFT' });
  live.addEventListener('click', () => pcFromAircraft('ldg'));
  const wx = el('button', { type: 'button', class: 'pc-btn', text: 'FROM METAR' });
  wx.addEventListener('click', () => pcFromMetar('ldg'));
  inputs.append(el('div', { class: 'pc-actions' }, ofp, live, wx, calc));
  const out = el('div', { class: 'pc-card pc-out' });
  const o = pcOut.ldg;
  if (!o) out.append(el('div', { class: 'pc-empty', text: 'Enter the runway, conditions and weight, then CALCULATE.' }));
  else if (o.error) out.append(el('div', { class: 'pc-empty pc-bad', text: o.error }));
  else {
    const r = o.r, q = o.p;
    out.append(el('div', { class: 'pc-h', text: `${pcIn.ldg.icao} ${pcIn.ldg.rwy}  •  CONF ${q.conf}` }));
    out.append(el('div', { class: 'pc-bigs' }, pcBig('VLS', String(r.vls)), pcBig('VAPP', String(r.vapp)), pcBig('LDA', `${Math.round(r.lda)} m`), pcBig('WIND', `${Math.round(q.hw)} HW`)));
    if (r.mlwOver) out.append(el('div', { class: 'pc-warn', text: `Landing weight above MLW (${q.mlw} t): overweight landing.` }));
    const bars = el('div', { class: 'pc-bars' });
    const max = Math.max(r.lda, ...r.rows.map((x) => x.dispatch));
    for (const row of r.rows) {
      const ok = row.inflight <= r.lda;
      const bar = el('div', { class: 'pc-bar' });
      bar.append(el('span', { class: 'pc-barl', text: row.ab === 'MAX' ? 'MAX MAN' : 'AUTO ' + row.ab }));
      const track = el('span', { class: 'pc-track' });
      track.append(el('span', { class: 'pc-fill' + (ok ? '' : ' bad'), style: `width:${(row.actual / max) * 100}%` }), el('span', { class: 'pc-fact', style: `left:${(row.inflight / max) * 100}%` }), el('span', { class: 'pc-lda', style: `left:${(r.lda / max) * 100}%` }));
      bar.append(track, el('span', { class: 'pc-barv' + (ok ? '' : ' pc-bad'), text: `${Math.round(row.actual)} / ${Math.round(row.inflight)} m` }));
      bars.append(bar);
    }
    out.append(el('div', { class: 'pc-h', text: 'LANDING DISTANCE  (ACTUAL / IN-FLIGHT x1.15)' }), bars);
    out.append(pcKv([
      ...r.rows.map((x) => [`Dispatch required, ${x.ab}`, `${Math.round(x.dispatch)} m`, x.dispatch > r.lda ? 'pc-bad' : 'pc-good']),
      ['Head / cross wind', `${Math.round(q.hw)} / ${Math.abs(Math.round(q.xw))} kt`, Math.abs(q.xw) > 40 ? 'pc-bad' : ''],
      ['Pressure altitude', `${Math.round(q.atm.pa)} ft`],
    ]));
  }
  c.append(el('div', { class: 'pc-wrap' }, inputs, out));
}

function pcRender() {
  const c = document.getElementById('perf-content');
  if (!c) return;
  pcOperatorDefaults();
  const keep = c.querySelector('.pc-inputs') ? c.querySelector('.pc-inputs').scrollTop : 0;
  c.innerHTML = '';
  const tabs = el('div', { class: 'pc-tabs' });
  for (const [k, t] of [['to', 'TAKEOFF'], ['ldg', 'LANDING']]) {
    const b = el('button', { type: 'button', class: 'pc-tab' + (pcTab === k ? ' on' : ''), text: t });
    b.addEventListener('click', () => { pcTab = k; pcRender(); });
    tabs.append(b);
  }
  c.append(tabs);
  c.append(el('div', { class: 'pc-note', text: pcCalText() }));
  if (pcTab === 'to') pcTakeoffView(c); else pcLandingView(c);
  const inp = c.querySelector('.pc-inputs'); if (inp) inp.scrollTop = keep;
}

(function () {
  const css = `
.pc-tabs{display:flex;gap:8px;margin:0 0 14px;}
button.pc-tab{flex:none;height:auto;padding:9px 22px;border-radius:8px;border:1px solid rgba(255,255,255,0.14) !important;background:#2a2c30;color:#cfd2d6;font:800 13px 'Manrope','Segoe UI',sans-serif;letter-spacing:0.06em;}
button.pc-tab.on{background:#3a3d42;color:#fff;border-color:#ff4d6a !important;box-shadow:inset 0 0 0 1px #ff4d6a;}
.pc-wrap{display:grid;grid-template-columns:minmax(380px,0.9fr) 1.3fr;gap:16px;align-items:start;min-height:0;}
.pc-card{background:rgba(40,42,46,0.94);border:1px solid rgba(255,255,255,0.08);border-radius:14px;padding:16px 18px;}
.pc-inputs{max-height:calc(100vh - 230px);overflow-y:auto;}
.pc-h{font:800 13px 'Manrope','Segoe UI',sans-serif;color:#9da3aa;letter-spacing:0.08em;margin:6px 0 10px;}
.pc-grid{display:grid;grid-template-columns:1fr 1fr;gap:10px 12px;}
.pc-f{display:flex;flex-direction:column;gap:5px;margin:0 0 10px;}
.pc-l{font:600 12.5px 'Manrope','Segoe UI',sans-serif;color:#c9ccd0;}
.pc-row{display:flex;align-items:center;gap:6px;}
.pc-in{flex:1;min-width:0;height:36px;box-sizing:border-box;border:1px solid rgba(255,255,255,0.14);border-radius:8px;background:#1f2124;color:#fff;padding:0 10px;font:700 14px 'Manrope','Segoe UI',sans-serif;text-transform:uppercase;}
.pc-u{font:600 12px 'Manrope','Segoe UI',sans-serif;color:#8d939a;min-width:24px;}
.pc-note{font:500 13px 'Manrope','Segoe UI',sans-serif;color:#8d939a;padding:8px 0;}
.pc-seg{display:flex;flex-wrap:wrap;gap:6px;}
button.pc-opt{flex:none;height:auto;padding:7px 13px;border-radius:8px;border:1px solid rgba(255,255,255,0.14) !important;background:#33363b;color:#dfe1e4;font:700 13px 'Manrope','Segoe UI',sans-serif;letter-spacing:0;text-transform:none;}
button.pc-opt.on{border-color:#ff4d6a !important;box-shadow:inset 0 0 0 1px #ff4d6a;color:#fff;}
.pc-actions{display:flex;gap:8px;justify-content:flex-end;margin-top:8px;flex-wrap:wrap;}
button.pc-btn{flex:none;height:auto;padding:10px 16px;border-radius:9px;border:1px solid rgba(255,255,255,0.16) !important;background:#33363b;color:#fff;font:800 12.5px 'Manrope','Segoe UI',sans-serif;letter-spacing:0.05em;}
button.pc-btn.pc-go{background:linear-gradient(90deg,#ff4d6a,#e2457a);border-color:transparent !important;}
.pc-empty{font:500 15px 'Manrope','Segoe UI',sans-serif;color:#9da3aa;padding:30px 6px;text-align:center;}
.pc-bigs{display:grid;grid-template-columns:repeat(auto-fit,minmax(100px,1fr));gap:10px;margin:0 0 12px;}
.pc-big{background:#1f2124;border-radius:10px;padding:10px 12px;border:1px solid rgba(255,255,255,0.07);}
.pc-bl{font:700 12px 'Manrope','Segoe UI',sans-serif;color:#9da3aa;letter-spacing:0.06em;}
.pc-bv{font:800 28px 'Manrope','Segoe UI',sans-serif;color:#29d3e6;margin-top:2px;white-space:nowrap;}
.pc-kv{display:grid;grid-template-columns:1fr auto;gap:6px 18px;margin-top:10px;}
.pc-k{font:600 13.5px 'Manrope','Segoe UI',sans-serif;color:#c9ccd0;}
.pc-v{font:800 13.5px 'Manrope','Segoe UI',sans-serif;color:#fff;text-align:right;}
.pc-good{color:#4cd964 !important;}
.pc-bad{color:#ff5a52 !important;}
.pc-warn{background:rgba(229,72,77,0.16);color:#ff6b63;font:700 13.5px 'Manrope','Segoe UI',sans-serif;border-radius:8px;padding:9px 12px;margin:0 0 10px;}
.pc-stripbox{margin:6px 0 4px;}
.pc-strip{width:100%;height:auto;display:block;}
.pc-bars{display:flex;flex-direction:column;gap:8px;}
.pc-bar{display:grid;grid-template-columns:86px 1fr 130px;align-items:center;gap:10px;}
.pc-barl{font:800 12.5px 'Manrope','Segoe UI',sans-serif;color:#dfe1e4;}
.pc-barv{font:700 12.5px 'Manrope','Segoe UI',sans-serif;color:#dfe1e4;text-align:right;}
.pc-track{position:relative;height:16px;background:#1f2124;border-radius:4px;}
.pc-fill{position:absolute;left:0;top:0;bottom:0;background:#4cd964;border-radius:4px;}
.pc-fill.bad{background:#ff5a52;}
.pc-fact{position:absolute;top:-3px;bottom:-3px;width:2px;background:#f5a623;}
.pc-lda{position:absolute;top:-6px;bottom:-6px;width:2px;background:#fff;}
`;
  const s = document.createElement('style');
  s.textContent = css;
  document.head.append(s);
})();

renderPerformance = function () {
  pcLoadCal().then(() => { if (document.getElementById("perf-content")) pcRender(); });
  if (!pcIn.to.icao && typeof lastOfp !== 'undefined' && lastOfp && pcFromOfp()) { pcLoadAirport('to', pcIn.to.icao); pcLoadAirport('ldg', pcIn.ldg.icao); return; }
  pcRender();
};

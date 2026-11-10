const AF_ZONES_MAIN = ['MAIN_DECK_1', 'MAIN_DECK_2', 'MAIN_DECK_3', 'MAIN_DECK_4', 'MAIN_DECK_5', 'MAIN_DECK_6', 'MAIN_DECK_7', 'MAIN_DECK_8'];
const AF_ZONES_UPPER = ['UPPER_DECK_1', 'UPPER_DECK_2', 'UPPER_DECK_3', 'UPPER_DECK_4', 'UPPER_DECK_5', 'UPPER_DECK_6', 'UPPER_DECK_7'];

function airflowNames() {
  const n = [];
  for (let e = 1; e <= 4; e++) for (const k of ['BLEED_EXTRACTION_FLOW', 'PR_VALVE_OPEN', 'HP_VALVE_OPEN', 'IP_VALVE_OPEN', 'REGULATED_TRANSDUCER_PRESSURE', 'PRECOOLER_OUTLET_TEMPERATURE']) n.push(`A32NX_PNEU_ENG_${e}_${k}`);
  for (let p = 1; p <= 2; p++) {
    for (let v = 1; v <= 2; v++) n.push(`A32NX_PNEU_PACK_${p}_FLOW_VALVE_${v}_FLOW_RATE`, `A32NX_COND_PACK_${p}_FLOW_VALVE_${v}_IS_OPEN`);
    n.push(`A32NX_COND_PACK_${p}_OUTLET_TEMPERATURE`, `A32NX_COND_PACK_${p}_RAM_AIR_FLOW`);
  }
  for (const z of ['CKPT', ...AF_ZONES_MAIN, ...AF_ZONES_UPPER]) n.push(`A32NX_COND_${z}_TEMP`);
  for (let o = 1; o <= 4; o++) n.push(`A32NX_PRESS_MAN_OUTFLOW_VALVE_${o}_OPEN_PERCENTAGE`);
  n.push('A32NX_PRESS_CABIN_ALTITUDE_1', 'A32NX_PRESS_CABIN_DELTA_PRESSURE_1', 'A32NX_PRESS_CABIN_VS_1', 'A32NX_VENT_CAB_FANS', 'A32NX_PNEU_APU_BLEED_CONTAINER_PRESSURE', 'A32NX_PRESS_SAFETY_VALVE_OPEN_PERCENTAGE');
  return n;
}

function airflowNominal() {
  const v = {};
  for (let e = 1; e <= 4; e++) Object.assign(v, { [`A32NX_PNEU_ENG_${e}_BLEED_EXTRACTION_FLOW`]: 1.05, [`A32NX_PNEU_ENG_${e}_PR_VALVE_OPEN`]: 1, [`A32NX_PNEU_ENG_${e}_HP_VALVE_OPEN`]: 0, [`A32NX_PNEU_ENG_${e}_IP_VALVE_OPEN`]: 1, [`A32NX_PNEU_ENG_${e}_REGULATED_TRANSDUCER_PRESSURE`]: 42, [`A32NX_PNEU_ENG_${e}_PRECOOLER_OUTLET_TEMPERATURE`]: 195 });
  for (let p = 1; p <= 2; p++) {
    for (let k = 1; k <= 2; k++) Object.assign(v, { [`A32NX_PNEU_PACK_${p}_FLOW_VALVE_${k}_FLOW_RATE`]: 0.62, [`A32NX_COND_PACK_${p}_FLOW_VALVE_${k}_IS_OPEN`]: 1 });
    Object.assign(v, { [`A32NX_COND_PACK_${p}_OUTLET_TEMPERATURE`]: 8, [`A32NX_COND_PACK_${p}_RAM_AIR_FLOW`]: 1.4 });
  }
  for (const z of ['CKPT', ...AF_ZONES_MAIN, ...AF_ZONES_UPPER]) v[`A32NX_COND_${z}_TEMP`] = 23;
  for (let o = 1; o <= 4; o++) v[`A32NX_PRESS_MAN_OUTFLOW_VALVE_${o}_OPEN_PERCENTAGE`] = 18;
  Object.assign(v, { A32NX_PRESS_CABIN_ALTITUDE_1: 6500, A32NX_PRESS_CABIN_DELTA_PRESSURE_1: 8.4, A32NX_PRESS_CABIN_VS_1: 0, A32NX_VENT_CAB_FANS: 1 });
  return v;
}

function airflowView(v, failed, opts = {}) {
  const W = 1000, H = 780;
  const n = (k, d = 0) => { const x = +v[k]; return Number.isFinite(x) ? x : d; };
  const fail = failed || new Map();
  const wires = [], fg = [];
  const flowState = (f, nominal, failKey) => (failKey && fail.get(failKey) === 'fail' ? 'fail' : f <= 0.02 ? 'off' : f < nominal * 0.4 ? 'warn' : 'live');
  const stream = (pts, f, nominal, failKey) => {
    const st = flowState(f, nominal, failKey);
    const d = pts.map((q, i) => (i ? 'L' : 'M') + q[0].toFixed(1) + ',' + q[1].toFixed(1)).join('');
    const w = st === 'off' ? 1.4 : 2 + Math.min(5, Math.sqrt(Math.max(0, f)) * 2.6);
    wires.push(`<path d="${d}" fill="none" stroke="${nsStroke(st)}" stroke-width="${w.toFixed(1)}" stroke-linejoin="round" opacity="${st === 'off' ? 0.6 : 0.35}"/>`);
    if (st !== 'off' && st !== 'fail') {
      const dur = Math.max(0.35, 2.6 / Math.max(0.15, f / nominal));
      wires.push(`<path d="${d}" fill="none" stroke="${nsStroke(st)}" stroke-width="${(w * 0.55).toFixed(1)}" stroke-linecap="round" stroke-dasharray="4 14" class="af-flow" style="animation-duration:${dur.toFixed(2)}s"/>`);
    }
    return st;
  };
  const val = (x, y, t, st = 'live', anchor = 'middle') => fg.push(exT(x, y, t, { size: 9, anchor, fill: st === 'live' ? NS.greyText : nsStroke(st), weight: 600 }));
  const engX = [110, 310, 690, 890];
  const manY = 250;
  let bleedTotal = 0;
  engX.forEach((x, i) => {
    const e = i + 1;
    const f = n(`A32NX_PNEU_ENG_${e}_BLEED_EXTRACTION_FLOW`);
    bleedTotal += f;
    const prv = n(`A32NX_PNEU_ENG_${e}_PR_VALVE_OPEN`) > 0.5;
    const hp = n(`A32NX_PNEU_ENG_${e}_HP_VALVE_OPEN`) > 0.5;
    const ip = n(`A32NX_PNEU_ENG_${e}_IP_VALVE_OPEN`) > 0.5;
    fg.push(nsBox(x - 50, 24, 100, 32, `ENG ${e}`, 'live', { size: 12 }));
    stream([[x - 22, 56], [x - 22, 96], [x, 96]], ip ? f : 0, 1, 'IPC' + e);
    stream([[x + 22, 56], [x + 22, 96], [x, 96]], hp ? f : 0, 1, 'HP' + e);
    fg.push(nsValve(x - 22, 76, fail.get('IPC' + e) || (ip ? 'live' : 'off'), ip, 'IP', { left: true }), nsValve(x + 22, 76, fail.get('HP' + e) || (hp ? 'live' : 'off'), hp, 'HP'));
    const st = stream([[x, 96], [x, 150]], prv ? f : 0, 1, 'PRV' + e);
    fg.push(nsValve(x, 120, fail.get('PRV' + e) || (prv ? 'live' : 'off'), prv, 'PRV'));
    fg.push(nsBox(x - 44, 150, 88, 30, 'PRECOOLER', fail.get('PCLR' + e) || (st === 'off' ? 'off' : 'live'), { size: 10 }));
    stream([[x, 180], [x, manY]], prv ? f : 0, 1, 'DUCT' + e);
    val(x + 8, 206, `${f.toFixed(2)} KG/S`, st, 'start');
    val(x + 8, 220, `${n(`A32NX_PNEU_ENG_${e}_REGULATED_TRANSDUCER_PRESSURE`).toFixed(0)} PSI  ${n(`A32NX_PNEU_ENG_${e}_PRECOOLER_OUTLET_TEMPERATURE`).toFixed(0)}°C`, st, 'start');
  });
  stream([[40, manY], [960, manY]], bleedTotal / 2, 1, null);
  val(500, manY - 8, `BLEED MANIFOLD  ${bleedTotal.toFixed(2)} KG/S`);
  const packX = [300, 700];
  let packTotal = 0;
  packX.forEach((x, i) => {
    const p = i + 1;
    let pf = 0;
    [-34, 34].forEach((dx, j) => {
      const k = j + 1;
      const f = n(`A32NX_PNEU_PACK_${p}_FLOW_VALVE_${k}_FLOW_RATE`);
      const open = n(`A32NX_COND_PACK_${p}_FLOW_VALVE_${k}_IS_OPEN`, f > 0.02 ? 1 : 0) > 0.5;
      pf += f;
      stream([[x + dx, manY], [x + dx, 340]], f, 0.6, `FCV${p}${k}`);
      fg.push(nsValve(x + dx, 296, fail.get(`FCV${p}${k}`) || (open ? 'live' : 'off'), open, `FCV ${p}-${k}`, { left: dx < 0 }));
    });
    packTotal += pf;
    const pst = fail.get('PACK' + p) || (pf <= 0.02 ? 'off' : 'live');
    fg.push(nsBox(x - 74, 340, 148, 44, `PACK ${p}\n${n(`A32NX_COND_PACK_${p}_OUTLET_TEMPERATURE`).toFixed(0)}°C  ${pf.toFixed(2)} KG/S`, pst, { size: 10 }));
    stream([[x + 74, 362], [x + 130, 362]], n(`A32NX_COND_PACK_${p}_RAM_AIR_FLOW`), 1, 'RAM' + p);
    val(x + 102, 354, 'RAM AIR');
    stream([[x, 384], [x, 420], [500, 420], [500, 440]], pf, 1.2, 'PACK' + p);
  });
  const fans = n('A32NX_VENT_CAB_FANS', 1) > 0.5;
  const recirc = fans ? packTotal * 0.65 : 0;
  fg.push(nsBox(430, 440, 140, 36, `MIXER UNIT\n${(packTotal + recirc).toFixed(2)} KG/S`, fail.get('MIXER') || (packTotal > 0.02 ? 'live' : 'off'), { size: 10 }));
  const zones = [['CKPT', 'CKPT'], ...AF_ZONES_UPPER.map((z, i) => [z, `U${i + 1}`]), ...AF_ZONES_MAIN.map((z, i) => [z, `M${i + 1}`])];
  const perZone = (packTotal + recirc) / zones.length;
  stream([[500, 476], [500, 500], [60, 500], [60, 520]], packTotal + recirc, 2.4, 'TRIM');
  stream([[60, 500], [940, 500]], packTotal + recirc, 2.4, 'TRIM');
  fg.push(exT(500, 494, 'TRIM AIR / ZONE DISTRIBUTION', { size: 9, fill: NS.greyText }));
  const zoneBox = (z, label, x, y, w, busY = 500) => {
    const t = n(`A32NX_COND_${z}_TEMP`, 23);
    stream([[x + w / 2, busY], [x + w / 2, y]], perZone, 0.15, 'TRIM');
    const st = t > 30 || t < 15 ? 'warn' : 'live';
    fg.push(nsBox(x, y, w, 38, `${label}\n${t.toFixed(0)}°C`, perZone <= 0.005 ? 'warn' : st, { size: 9, r: 7 }));
  };
  zoneBox('CKPT', 'CKPT', 20, 532, 70);
  AF_ZONES_UPPER.forEach((z, i) => zoneBox(z, `UPPER ${i + 1}`, 104 + i * 126, 532, 112));
  stream([[60, 500], [8, 500], [8, 580], [960, 580]], perZone * 8, 1.2, 'TRIM');
  AF_ZONES_MAIN.forEach((z, i) => zoneBox(z, `MAIN ${i + 1}`, 20 + i * 120, 590, 108, 580));
  if (fans) stream([[900, 628], [900, 646], [980, 646], [980, 458], [570, 458]], recirc, 1, null);
  fg.push(nsBox(930, 470, 60, 30, 'CAB FANS', fans ? 'live' : 'off', { size: 8, r: 6 }));
  const cabinOut = packTotal;
  let ofvSum = 0;
  for (let o = 1; o <= 4; o++) ofvSum += n(`A32NX_PRESS_MAN_OUTFLOW_VALVE_${o}_OPEN_PERCENTAGE`);
  for (let o = 1; o <= 4; o++) {
    const x = 140 + (o - 1) * 200;
    const pct = n(`A32NX_PRESS_MAN_OUTFLOW_VALVE_${o}_OPEN_PERCENTAGE`);
    const f = ofvSum > 0 ? (cabinOut * pct) / ofvSum : 0;
    stream([[x, 646], [x, 720]], f, 0.4, 'OFV' + o);
    fg.push(nsValve(x, 684, fail.get('OFV' + o) || (pct > 0.5 ? 'live' : 'off'), pct > 0.5, `OFV ${o}  ${pct.toFixed(0)}%`));
  }
  stream([[20, 646], [980, 646]], cabinOut, 2.4, 'CPC');
  val(500, 742, `OVERBOARD  ${cabinOut.toFixed(2)} KG/S`);
  const alt = n('A32NX_PRESS_CABIN_ALTITUDE_1'), dp = n('A32NX_PRESS_CABIN_DELTA_PRESSURE_1'), vs = n('A32NX_PRESS_CABIN_VS_1');
  fg.push(exT(20, 764, `CABIN ALT ${alt.toFixed(0)} FT   ΔP ${dp.toFixed(1)} PSI   V/S ${vs.toFixed(0)} FT/MIN`, { size: 11, anchor: 'start', fill: alt > 9550 ? NS.red : NS.green }));
  if (opts.note) fg.push(exT(W - 16, 764, opts.note, { size: 10, anchor: 'end', fill: NS.orange }));
  const style = '<style>.af-flow{animation:afmove linear infinite}@keyframes afmove{to{stroke-dashoffset:-36}}</style>';
  return nsSvg(W, H, [style, ...wires, ...fg]);
}

function airflowFailures(list) {
  const m = new Map();
  for (const f of list || []) {
    if (!f.active) continue;
    const t = typeof airTargets === 'function' ? airTargets(f.name) : null;
    if (t) for (const [k, s] of t) m.set(k, s);
  }
  return m;
}

function airflowPage(body) {
  const host = el('div', { class: 'af-live' });
  body.append(host);
  let failures = new Map();
  let failAt = 0;
  const live = typeof studyGet === 'function' && location.protocol.startsWith('http') && !(typeof FIXTURE !== 'undefined' && FIXTURE);
  const draw = (v, note) => host.replaceChildren(airflowView(v, failures, { note }));
  const tick = async () => {
    if (!host.isConnected) { clearInterval(timer); return; }
    if (!live) { draw(airflowNominal(), 'NOT CONNECTED · NOMINAL CRUISE VALUES'); return; }
    try {
      if (performance.now() > failAt) {
        failAt = performance.now() + 3000;
        try { failures = airflowFailures((await studyGet('/study/failures')).failures); } catch { /* keep the last */ }
      }
      const q = '?names=' + airflowNames().map(encodeURIComponent).join(',');
      const data = await studyGet('/vars' + q);
      draw(data.vars || {}, data.running === false ? 'SIM NOT RUNNING' : '');
    } catch {
      draw(airflowNominal(), 'NOT CONNECTED · NOMINAL CRUISE VALUES');
    }
  };
  const timer = setInterval(tick, 1000);
  tick();
}

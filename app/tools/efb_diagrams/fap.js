const FAP_PAGES = [
  ['status', 'CABIN STATUS'],
  ['audio', 'AUDIO'],
  ['lights', 'LIGHTS'],
  ['doors', 'DOORS / SLIDES'],
  ['temp', 'TEMP'],
  ['water', 'WATER / WASTE'],
  ['smoke', 'SMOKE DETECT'],
  ['seats', 'SEAT SETTING'],
  ['sysinfo', 'SYSTEM INFO'],
];
const FAP_DOORS = ['M1L', 'M1R', 'M2L', 'M2R', 'M3L', 'M3R', 'M4L', 'M4R', 'M5L', 'M5R', 'U1L', 'U1R', 'U2L', 'U2R', 'U3L', 'U3R', 'CARGO FWD', 'CARGO AFT'];
const FAP_ZONES = ['FWD', 'MID', 'AFT'];
const fap = { page: 'status', v: {}, ok: false, timer: null };

function fapNames() {
  const n = ['A32NX_COND_CKPT_TEMP', 'A32NX_COND_CARGO_FWD_TEMP', 'A32NX_COND_CARGO_BULK_TEMP', 'A32NX_COND_PURS_SEL_TEMPERATURE',
    'A32NX_CABIN_WATER_QTY_PERCENT', 'A32NX_CABIN_WATER_QTY_KG', 'A32NX_CABIN_WATER_PRESS_PSI', 'A32NX_CABIN_WATER_COMPRESSOR_CMD', 'A32NX_CABIN_WATER_FLOW_FRACTION', 'A32NX_CABIN_WATER_LEAK_L_S',
    'A32NX_CABIN_WASTE_GENERATOR_RUNNING', 'A32NX_CABIN_SLIDE_PRESSURE_LOW:1', 'A32NX_CABIN_SLIDE_DEPLOYED:1', 'A32NX_CABIN_SLIDE_BOTTLE_PA:1', 'A32NX_CABIN_DOOR_LATCHED:1', 'A32NX_CABIN_CARGO_DOOR_JAMMED:1',
    'A32NX_CARGO_FWD_SMOKE_DETECTED', 'A32NX_CARGO_AFT_SMOKE_DETECTED', 'A32NX_CABIN_IFE_CONTENT_AVAILABLE', 'A32NX_CABIN_SEAT_POWER_CMD', 'A32NX_CABIN_GALLEY_TOTAL_POWER_W',
    'A32NX_CABIN_CREW_CALL_NEW_COUNT', 'A32NX_CABIN_CREW_CALL_PRIORITY', 'A32NX_DEEP_CABIN_PURSER_TEMP_SEL_FAULT', 'A32NX_DEEP_CABIN_LAV_GALLEY_EXTRACT_FAULT', 'A32NX_DEEP_CABIN_SECONDARY_FANS_FAILED_COUNT',
    'A32NX_DEEP_CABIN_IFE_BAY_ISOL_FAULT', 'A32NX_DEEP_CABIN_IFE_BAY_VENT_FAULT', 'A32NX_DEEP_COM_CIDS_PA_MAIN_MAGNITUDE', 'A32NX_DEEP_COM_CIDS_PA_UPPER_MAGNITUDE', 'A32NX_DEEP_COM_CIDS_PA_LOWER_MAGNITUDE', 'A32NX_DEEP_COM_CIDS_INTERPHONE_MAGNITUDE'];
  for (let i = 1; i <= 8; i++) n.push(`A32NX_COND_MAIN_DECK_${i}_TEMP`);
  for (let i = 1; i <= 7; i++) n.push(`A32NX_COND_UPPER_DECK_${i}_TEMP`);
  for (let i = 0; i < 18; i++) n.push(`INTERACTIVE POINT OPEN:${i}`);
  for (let i = 0; i < 16; i++) n.push(`ANIM_DOOR_${FAP_DOORS[i]}_DISARMED`);
  for (let i = 1; i <= 3; i++) {
    n.push(`A32NX_DEEP_COM_CIDS_${i}_FAULT`, `A32NX_CABIN_WASTE_LEVEL_PERCENT:${i}`, `A32NX_CABIN_WASTE_TANK_FULL:${i}`, `A32NX_CABIN_LAV_INOPERATIVE:${i}`, `A32NX_CABIN_WATER_HEATER_TEMP_C:${i}`,
      `A32NX_CABIN_IFE_ZONE_SMOKE:${i}`, `A32NX_CABIN_IFE_ZONE_OVERHEAT:${i}`, `A32NX_CABIN_IFE_ZONE_TRIPPED:${i}`, `A32NX_CABIN_IFE_ZONE_TEMP_C:${i}`, `A32NX_CABIN_GALLEY_OVEN_SMOKE:${i}`,
      `A32NX_CABIN_GALLEY_OVEN_TEMP_C:${i}`, `A32NX_CABIN_GALLEY_CHILLER_TEMP_C:${i}`, `A32NX_CABIN_GALLEY_BOILER_TEMP_C:${i}`, `A32NX_CABIN_GALLEY_BUS_FAULT:${i}`);
  }
  for (let i = 1; i <= 2; i++) n.push(`A32NX_CABIN_IFE_SERVER_FAIL:${i}`, `A32NX_CABIN_MAST_BLOCKED:${i}`, `A32NX_CABIN_MAST_ICE_KG:${i}`);
  return n;
}
const FAP_NAMES = fapNames();

async function fapPoll() {
  try {
    const r = await studyGet('/vars?names=' + FAP_NAMES.map(encodeURIComponent).join(','));
    fap.ok = !!(r && r.running && r.vars);
    fap.v = (r && r.vars) || {};
  } catch {
    fap.ok = false;
  }
  fapRender();
}

const fv = (k) => (k in fap.v && Number.isFinite(fap.v[k]) ? fap.v[k] : null);
const fon = (k) => (fv(k) || 0) >= 0.5;
const ftxt = (x, digits = 0, unit = '') => (x === null ? '--' : `${x.toFixed(digits)}${unit}`);

function fapCautions() {
  const out = [];
  if (fon('A32NX_CARGO_FWD_SMOKE_DETECTED') || fon('A32NX_CARGO_AFT_SMOKE_DETECTED')) out.push(['red', 'CARGO SMOKE']);
  for (let i = 1; i <= 3; i++) {
    if (fon(`A32NX_CABIN_IFE_ZONE_SMOKE:${i}`)) out.push(['red', `SMOKE IFE ZONE ${FAP_ZONES[i - 1]}`]);
    if (fon(`A32NX_CABIN_GALLEY_OVEN_SMOKE:${i}`)) out.push(['red', `SMOKE GALLEY ${FAP_ZONES[i - 1]}`]);
    if (fon(`A32NX_CABIN_WASTE_TANK_FULL:${i}`)) out.push(['amber', `WASTE TANK ${FAP_ZONES[i - 1]} FULL`]);
    if (fon(`A32NX_DEEP_COM_CIDS_${i}_FAULT`)) out.push(['amber', `CIDS DIR ${i} FAULT`]);
  }
  if (fon('A32NX_CABIN_SLIDE_PRESSURE_LOW:1')) out.push(['amber', 'SLIDE BOTTLE PRESSURE LOW']);
  if (fon('A32NX_CABIN_SLIDE_DEPLOYED:1')) out.push(['amber', 'SLIDE DEPLOYED']);
  const water = fv('A32NX_CABIN_WATER_QTY_PERCENT');
  if (water !== null && water < 10) out.push(['amber', 'POTABLE WATER LOW']);
  const calls = fv('A32NX_CABIN_CREW_CALL_NEW_COUNT') || 0;
  if (calls > 0) out.push(['cyan', `${calls} CREW CALL${calls > 1 ? 'S' : ''}`]);
  return out;
}

function fapTempClass(t) {
  if (t === null) return 'na';
  return t < 20 ? 'cold' : t > 26 ? 'hot' : 'ok';
}

function fapZone(label, key) {
  const t = fv(key);
  return el('div', { class: 'fap-zone ' + fapTempClass(t) }, el('span', { class: 'fap-zl', text: label }), el('span', { class: 'fap-zt', text: ftxt(t, 1, '°') }));
}

function fapRow(k, v, cls = '') {
  return el('div', { class: 'fap-row' }, el('span', { text: k }), el('span', { class: 'fap-val ' + cls, text: v }));
}

function fapBox(title, ...kids) {
  return el('div', { class: 'fap-box' }, el('div', { class: 'fap-boxh', text: title }), ...kids);
}

function fapBtn(text, on, cb, cls = '') {
  const b = el('button', { type: 'button', class: 'fap-btn ' + cls + (on ? ' on' : ''), text });
  b.addEventListener('click', cb);
  return b;
}

async function fapSetting(key, value) {
  const r = await bridge.setSettings({ [key]: value });
  if (r && r.ok === false) { show(r.message || 'Could not save'); return; }
  settings = { ...settings, [key]: value };
  fill();
  fapRender();
}

function fapDoorState(i) {
  const p = fv(`INTERACTIVE POINT OPEN:${i}`);
  if (p === null) return ['--', 'na'];
  if (p > 98) return ['OPEN', 'amber'];
  if (p > 2) return [`${Math.round(p)} %`, 'amber'];
  return ['CLOSED', 'green'];
}

function fapPage(host) {
  const p = fap.page;
  if (p === 'status') {
    const open = FAP_DOORS.filter((_, i) => fapDoorState(i)[0] !== 'CLOSED' && fapDoorState(i)[0] !== '--').length;
    const temps = [];
    for (let i = 1; i <= 8; i++) temps.push(fv(`A32NX_COND_MAIN_DECK_${i}_TEMP`));
    for (let i = 1; i <= 7; i++) temps.push(fv(`A32NX_COND_UPPER_DECK_${i}_TEMP`));
    const known = temps.filter((t) => t !== null);
    const avg = known.length ? known.reduce((a, b) => a + b, 0) / known.length : null;
    const waste = Math.max(...[1, 2, 3].map((i) => fv(`A32NX_CABIN_WASTE_LEVEL_PERCENT:${i}`) || 0));
    const lavs = [1, 2, 3].filter((i) => fon(`A32NX_CABIN_LAV_INOPERATIVE:${i}`)).length;
    host.append(el('div', { class: 'fap-grid' },
      fapBox('DOORS', fapRow('Open doors', String(open), open ? 'amber' : 'green'), fapRow('Slides', fon('A32NX_CABIN_SLIDE_PRESSURE_LOW:1') ? 'CHECK' : 'NORMAL', fon('A32NX_CABIN_SLIDE_PRESSURE_LOW:1') ? 'amber' : 'green')),
      fapBox('TEMPERATURE', fapRow('Cabin average', ftxt(avg, 1, ' °C')), fapRow('Selected (purser)', ftxt(fv('A32NX_COND_PURS_SEL_TEMPERATURE'), 1, ' °C'))),
      fapBox('WATER / WASTE', fapRow('Potable water', ftxt(fv('A32NX_CABIN_WATER_QTY_PERCENT'), 0, ' %')), fapRow('Waste (highest)', `${Math.round(waste)} %`, waste > 90 ? 'amber' : ''), fapRow('Lavatories inop', String(lavs), lavs ? 'amber' : '')),
      fapBox('CABIN', fapRow('IFE content', fon('A32NX_CABIN_IFE_CONTENT_AVAILABLE') ? 'AVAILABLE' : 'NOT AVAILABLE', fon('A32NX_CABIN_IFE_CONTENT_AVAILABLE') ? 'green' : 'amber'), fapRow('Seat power', fon('A32NX_CABIN_SEAT_POWER_CMD') ? 'ON' : 'OFF'), fapRow('Galley load', ftxt((fv('A32NX_CABIN_GALLEY_TOTAL_POWER_W') || 0) / 1000, 1, ' kW')), fapRow('Crew calls', String(fv('A32NX_CABIN_CREW_CALL_NEW_COUNT') || 0)))));
    return;
  }
  if (p === 'temp') {
    const sel = fv('A32NX_COND_PURS_SEL_TEMPERATURE');
    const step = (d) => studyPost('/study/action', { kind: 'fapSet', item: 'pursTemp', value: Math.round(((sel ?? 24) + d) * 2) / 2 }).then(() => setTimeout(fapPoll, 400));
    const upper = el('div', { class: 'fap-deck' }, el('div', { class: 'fap-deckl', text: 'UPPER DECK' }), el('div', { class: 'fap-zones' }, ...[1, 2, 3, 4, 5, 6, 7].map((i) => fapZone(`U${i}`, `A32NX_COND_UPPER_DECK_${i}_TEMP`))));
    const main = el('div', { class: 'fap-deck' }, el('div', { class: 'fap-deckl', text: 'MAIN DECK' }), el('div', { class: 'fap-zones' }, fapZone('CKPT', 'A32NX_COND_CKPT_TEMP'), ...[1, 2, 3, 4, 5, 6, 7, 8].map((i) => fapZone(`M${i}`, `A32NX_COND_MAIN_DECK_${i}_TEMP`))));
    const cargo = el('div', { class: 'fap-deck' }, el('div', { class: 'fap-deckl', text: 'CARGO' }), el('div', { class: 'fap-zones' }, fapZone('FWD', 'A32NX_COND_CARGO_FWD_TEMP'), fapZone('BULK', 'A32NX_COND_CARGO_BULK_TEMP')));
    const ctl = el('div', { class: 'fap-sel' }, el('span', { text: 'CABIN TEMP SELECTED' }), fapBtn('▼', false, () => step(-0.5)), el('span', { class: 'fap-selv', text: ftxt(sel, 1, ' °C') }), fapBtn('▲', false, () => step(0.5)));
    host.append(el('div', { class: 'fap-plane' }, upper, main, cargo), ctl, el('div', { class: 'fap-note', text: 'The cabin zones follow this temperature when the cockpit CABIN temperature selector is set to PURS SEL; otherwise the selector sets them. Range 18 to 30 °C.' }));
    if (fon('A32NX_DEEP_CABIN_PURSER_TEMP_SEL_FAULT')) host.append(el('div', { class: 'fap-note amber', text: 'PURSER TEMP SELECTION FAULT: the cockpit CABIN selector controls the cabin.' }));
    return;
  }
  if (p === 'doors') {
    const rows = FAP_DOORS.map((n, i) => {
      const [t, c] = fapDoorState(i);
      const lever = i < 16 ? fv(`ANIM_DOOR_${n}_DISARMED`) : null;
      const slide = lever === null ? '' : lever < 0.5 ? ' · ARMED' : ' · DISARMED';
      return el('div', { class: 'fap-door ' + c + (lever !== null && lever < 0.5 ? ' armed' : '') }, el('span', { text: n }), el('span', { text: t + slide }));
    });
    const psi = fv('A32NX_CABIN_SLIDE_BOTTLE_PA:1');
    host.append(el('div', { class: 'fap-doors' }, ...rows), fapBox('SLIDES',
      fapRow('Bottle pressure', psi === null ? '--' : `${Math.round(psi / 6894.757)} psi`, fon('A32NX_CABIN_SLIDE_PRESSURE_LOW:1') ? 'amber' : 'green'),
      fapRow('Deployed', fon('A32NX_CABIN_SLIDE_DEPLOYED:1') ? 'YES' : 'NO', fon('A32NX_CABIN_SLIDE_DEPLOYED:1') ? 'amber' : ''),
      fapRow('Door latch', fon('A32NX_CABIN_DOOR_LATCHED:1') ? 'LATCHED' : 'UNLATCHED'),
      fapRow('Cargo door', fon('A32NX_CABIN_CARGO_DOOR_JAMMED:1') ? 'JAMMED' : 'NORMAL', fon('A32NX_CABIN_CARGO_DOOR_JAMMED:1') ? 'amber' : '')));
    return;
  }
  if (p === 'water') {
    const tank = fv('A32NX_CABIN_WATER_QTY_PERCENT');
    const bar = (pct, cls) => el('div', { class: 'fap-bar' }, el('div', { class: 'fap-fill ' + cls, style: `height:${Math.max(0, Math.min(100, pct || 0))}%` }));
    const water = fapBox('POTABLE WATER', el('div', { class: 'fap-tanks' }, el('div', { class: 'fap-tank' }, bar(tank, 'water'), el('div', { text: ftxt(tank, 0, ' %') }), el('div', { class: 'fap-small', text: ftxt(fv('A32NX_CABIN_WATER_QTY_KG'), 0, ' kg') }))),
      fapRow('Pressure', ftxt(fv('A32NX_CABIN_WATER_PRESS_PSI'), 1, ' psi')), fapRow('Compressor', fon('A32NX_CABIN_WATER_COMPRESSOR_CMD') ? 'ON' : 'OFF'),
      fapRow('Leak', (fv('A32NX_CABIN_WATER_LEAK_L_S') || 0) > 0.001 ? 'DETECTED' : 'NONE', (fv('A32NX_CABIN_WATER_LEAK_L_S') || 0) > 0.001 ? 'amber' : ''),
      ...[1, 2, 3].map((i) => fapRow(`Heater ${FAP_ZONES[i - 1]}`, ftxt(fv(`A32NX_CABIN_WATER_HEATER_TEMP_C:${i}`), 0, ' °C'))),
      ...[1, 2].map((i) => fapRow(`Drain mast ${i}`, fon(`A32NX_CABIN_MAST_BLOCKED:${i}`) ? 'BLOCKED' : 'CLEAR', fon(`A32NX_CABIN_MAST_BLOCKED:${i}`) ? 'amber' : '')));
    const waste = fapBox('WASTE', el('div', { class: 'fap-tanks' }, ...[1, 2, 3].map((i) => { const w = fv(`A32NX_CABIN_WASTE_LEVEL_PERCENT:${i}`); return el('div', { class: 'fap-tank' }, bar(w, w > 90 ? 'full' : 'waste'), el('div', { text: ftxt(w, 0, ' %') }), el('div', { class: 'fap-small', text: FAP_ZONES[i - 1] })); })),
      fapRow('Vacuum generator', fon('A32NX_CABIN_WASTE_GENERATOR_RUNNING') ? 'RUNNING' : 'OFF'),
      ...[1, 2, 3].map((i) => fapRow(`Lavatories ${FAP_ZONES[i - 1]}`, fon(`A32NX_CABIN_LAV_INOPERATIVE:${i}`) ? 'INOP' : 'NORMAL', fon(`A32NX_CABIN_LAV_INOPERATIVE:${i}`) ? 'amber' : 'green')));
    host.append(el('div', { class: 'fap-grid two' }, water, waste));
    return;
  }
  if (p === 'smoke') {
    const det = [['CARGO FWD', 'A32NX_CARGO_FWD_SMOKE_DETECTED'], ['CARGO AFT', 'A32NX_CARGO_AFT_SMOKE_DETECTED'], ...[1, 2, 3].map((i) => [`IFE ZONE ${FAP_ZONES[i - 1]}`, `A32NX_CABIN_IFE_ZONE_SMOKE:${i}`]), ...[1, 2, 3].map((i) => [`GALLEY ${FAP_ZONES[i - 1]} OVEN`, `A32NX_CABIN_GALLEY_OVEN_SMOKE:${i}`])];
    host.append(el('div', { class: 'fap-doors' }, ...det.map(([n, k]) => el('div', { class: 'fap-door ' + (fv(k) === null ? 'na' : fon(k) ? 'red' : 'green') }, el('span', { text: n }), el('span', { text: fv(k) === null ? '--' : fon(k) ? 'SMOKE' : 'NORMAL' })))));
    return;
  }
  if (p === 'audio') {
    const ann = String(settings.SOUND_ANNOUNCEMENTS_ENABLED ?? DEFAULTS.SOUND_ANNOUNCEMENTS_ENABLED) === '1';
    const vol = Number(settings.SOUND_LEVEL_CABIN ?? DEFAULTS.SOUND_LEVEL_CABIN ?? 100);
    const slider = el('input', { type: 'range', min: '0', max: '100', step: '5', value: String(vol), class: 'fap-range' });
    slider.addEventListener('change', () => fapSetting('SOUND_LEVEL_CABIN', slider.value));
    host.append(el('div', { class: 'fap-grid two' },
      fapBox('ANNOUNCEMENTS', el('div', { class: 'fap-btns' }, fapBtn('PRE-RECORDED ON', ann, () => fapSetting('SOUND_ANNOUNCEMENTS_ENABLED', '1')), fapBtn('OFF', !ann, () => fapSetting('SOUND_ANNOUNCEMENTS_ENABLED', '0'))),
        el('div', { class: 'fap-note', text: 'The pre-recorded cabin announcements play by flight phase.' })),
      fapBox('CABIN VOLUME', slider, el('div', { class: 'fap-selv', text: `${vol} %` })),
      fapBox('PA / INTERPHONE', fapRow('PA main deck', ftxt(fv('A32NX_DEEP_COM_CIDS_PA_MAIN_MAGNITUDE'), 2)), fapRow('PA upper deck', ftxt(fv('A32NX_DEEP_COM_CIDS_PA_UPPER_MAGNITUDE'), 2)), fapRow('PA lower deck', ftxt(fv('A32NX_DEEP_COM_CIDS_PA_LOWER_MAGNITUDE'), 2)), fapRow('Interphone', ftxt(fv('A32NX_DEEP_COM_CIDS_INTERPHONE_MAGNITUDE'), 2))),
      fapBox('IFE', fapRow('Content', fon('A32NX_CABIN_IFE_CONTENT_AVAILABLE') ? 'AVAILABLE' : 'NOT AVAILABLE', fon('A32NX_CABIN_IFE_CONTENT_AVAILABLE') ? 'green' : 'amber'), ...[1, 2].map((i) => fapRow(`Server ${i}`, fon(`A32NX_CABIN_IFE_SERVER_FAIL:${i}`) ? 'FAULT' : 'NORMAL', fon(`A32NX_CABIN_IFE_SERVER_FAIL:${i}`) ? 'amber' : 'green')))));
    return;
  }
  if (p === 'lights') {
    const def = SETTINGS_PAGE.flatMap((s) => s[2]).find((it) => it.key === 'xphfbw.cabinScene');
    const opts = def ? def.sel || def.seg || [] : [];
    const cur = String(settings['xphfbw.cabinScene'] ?? DEFAULTS['xphfbw.cabinScene'] ?? '0');
    const bright = Number(settings['xphfbw.cabinBrightness'] ?? DEFAULTS['xphfbw.cabinBrightness'] ?? 1);
    const slider = el('input', { type: 'range', min: '0', max: '1', step: '0.05', value: String(bright), class: 'fap-range' });
    slider.addEventListener('change', () => fapSetting('xphfbw.cabinBrightness', slider.value));
    host.append(fapBox('CABIN LIGHTING SCENE', el('div', { class: 'fap-btns wrap' }, ...opts.map(([v, label]) => fapBtn(String(label).toUpperCase(), String(v) === cur, () => fapSetting('xphfbw.cabinScene', String(v)))))),
      fapBox('DIMMING', slider, el('div', { class: 'fap-selv', text: `${Math.round(bright * 100)} %` })));
    return;
  }
  if (p === 'seats') {
    host.append(el('div', { class: 'fap-grid' }, ...[1, 2, 3].map((i) => fapBox(`SEAT / IFE ZONE ${FAP_ZONES[i - 1]}`,
      fapRow('In-seat power', fon(`A32NX_CABIN_IFE_ZONE_TRIPPED:${i}`) ? 'TRIPPED' : fon('A32NX_CABIN_SEAT_POWER_CMD') ? 'ON' : 'OFF', fon(`A32NX_CABIN_IFE_ZONE_TRIPPED:${i}`) ? 'amber' : ''),
      fapRow('Equipment temp', ftxt(fv(`A32NX_CABIN_IFE_ZONE_TEMP_C:${i}`), 0, ' °C'), fon(`A32NX_CABIN_IFE_ZONE_OVERHEAT:${i}`) ? 'amber' : ''),
      fapRow('Overheat', fon(`A32NX_CABIN_IFE_ZONE_OVERHEAT:${i}`) ? 'YES' : 'NO', fon(`A32NX_CABIN_IFE_ZONE_OVERHEAT:${i}`) ? 'amber' : '')))),
      el('div', { class: 'fap-note', text: 'In-seat power follows the commercial electrical supply: it is shed with the galleys and when a zone trips.' }));
    return;
  }
  if (p === 'sysinfo') {
    host.append(el('div', { class: 'fap-grid' },
      fapBox('CIDS DIRECTORS', ...[1, 2, 3].map((i) => fapRow(`Director ${i}`, fon(`A32NX_DEEP_COM_CIDS_${i}_FAULT`) ? 'FAULT' : 'NORMAL', fon(`A32NX_DEEP_COM_CIDS_${i}_FAULT`) ? 'amber' : 'green'))),
      fapBox('VENTILATION', fapRow('Lav / galley extract', fon('A32NX_DEEP_CABIN_LAV_GALLEY_EXTRACT_FAULT') ? 'FAULT' : 'NORMAL', fon('A32NX_DEEP_CABIN_LAV_GALLEY_EXTRACT_FAULT') ? 'amber' : 'green'), fapRow('Secondary fans failed', String(fv('A32NX_DEEP_CABIN_SECONDARY_FANS_FAILED_COUNT') || 0)), fapRow('IFE bay isolation', fon('A32NX_DEEP_CABIN_IFE_BAY_ISOL_FAULT') ? 'FAULT' : 'NORMAL'), fapRow('IFE bay ventilation', fon('A32NX_DEEP_CABIN_IFE_BAY_VENT_FAULT') ? 'FAULT' : 'NORMAL')),
      ...[1, 2, 3].map((i) => fapBox(`GALLEY ${FAP_ZONES[i - 1]}`, fapRow('Supply', fon(`A32NX_CABIN_GALLEY_BUS_FAULT:${i}`) ? 'FAULT' : 'NORMAL', fon(`A32NX_CABIN_GALLEY_BUS_FAULT:${i}`) ? 'amber' : 'green'), fapRow('Oven', ftxt(fv(`A32NX_CABIN_GALLEY_OVEN_TEMP_C:${i}`), 0, ' °C')), fapRow('Chiller', ftxt(fv(`A32NX_CABIN_GALLEY_CHILLER_TEMP_C:${i}`), 1, ' °C')), fapRow('Boiler', ftxt(fv(`A32NX_CABIN_GALLEY_BOILER_TEMP_C:${i}`), 0, ' °C'))))));
  }
}

function fapRender() {
  const c = document.getElementById('fap-content');
  if (!c || !document.getElementById('tab-fap').classList.contains('active')) return;
  const scroll = c.querySelector('.fap-main') ? c.querySelector('.fap-main').scrollTop : 0;
  c.innerHTML = '';
  const now = new Date();
  const z = `${String(now.getUTCHours()).padStart(2, '0')}:${String(now.getUTCMinutes()).padStart(2, '0')} UTC`;
  const o = typeof lastOfp !== 'undefined' && lastOfp ? lastOfp : null;
  const flt = o && o.general ? `${o.general.icao_airline || ''}${o.general.flight_number || ''}  ${o.origin ? o.origin.icao_code : ''}-${o.destination ? o.destination.icao_code : ''}` : 'NO FLIGHT DATA';
  const title = (FAP_PAGES.find((x) => x[0] === fap.page) || FAP_PAGES[0])[1];
  const nav = el('div', { class: 'fap-nav' }, ...FAP_PAGES.map(([k, t]) => fapBtn(t, fap.page === k, () => { fap.page = k; fapRender(); }, 'fap-navb')));
  const main = el('div', { class: 'fap-main' });
  if (!fap.ok) main.append(el('div', { class: 'fap-note', text: 'CIDS NOT CONNECTED: start X-Plane with the A380 to use the FAP.' }));
  fapPage(main);
  const cautions = fapCautions();
  const foot = el('div', { class: 'fap-foot' }, ...(cautions.length ? cautions.map(([c2, t]) => el('span', { class: 'fap-caut ' + c2, text: t })) : [el('span', { class: 'fap-caut none', text: 'NO CIDS CAUTION' })]));
  const screen = el('div', { class: 'fap-screen' }, el('div', { class: 'fap-top' }, el('span', { class: 'fap-title', text: title }), el('span', { text: flt }), el('span', { text: z })), el('div', { class: 'fap-body' }, nav, main), foot);
  c.append(el('div', { class: 'fap-bezel' }, el('div', { class: 'fap-brand', text: 'FLIGHT ATTENDANT PANEL' }), screen));
  main.scrollTop = scroll;
}

function fapOnShow() {
  fapRender();
  fapPoll();
  if (!fap.timer) fap.timer = setInterval(() => { if (document.getElementById('tab-fap').classList.contains('active')) fapPoll(); }, 1000);
}

(function () {
  const s = document.createElement('style');
  s.textContent = `
#tab-fap>.page-main{padding:12px 20px 16px;display:flex;}
#fap-content{flex:1;display:flex;min-height:0;}
.fap-bezel{flex:1;display:flex;flex-direction:column;min-height:0;border-radius:16px;padding:14px 16px 18px;background:#3d4146;background-image:repeating-linear-gradient(45deg,rgba(255,255,255,0.025) 0 2px,transparent 2px 5px),repeating-linear-gradient(-45deg,rgba(0,0,0,0.05) 0 2px,transparent 2px 6px);box-shadow:inset 0 1px 0 rgba(255,255,255,0.12),inset 0 -2px 6px rgba(0,0,0,0.5),0 14px 40px rgba(0,0,0,0.45);}
.fap-brand{font:700 11px Arial,sans-serif;letter-spacing:0.25em;color:#a9aeb4;margin:0 0 8px 4px;}
.fap-screen{flex:1;display:flex;flex-direction:column;min-height:0;background:#d7dbdf;border-radius:6px;border:3px solid #1d2024;box-shadow:inset 0 0 12px rgba(0,0,0,0.25);font-family:Arial,'Helvetica Neue',sans-serif;color:#1b2430;overflow:hidden;}
.fap-top{display:flex;justify-content:space-between;align-items:center;padding:8px 14px;background:linear-gradient(#2f557d,#23405f);color:#fff;font:700 14px Arial,sans-serif;letter-spacing:0.05em;}
.fap-title{font-size:17px;}
.fap-body{flex:1;display:flex;min-height:0;}
.fap-nav{width:190px;flex:none;display:flex;flex-direction:column;gap:6px;padding:10px;background:#c4c9ce;border-right:2px solid #9aa1a8;}
button.fap-btn{flex:none;height:auto;padding:10px 12px;border-radius:5px;border:1px solid #7d858d !important;background:linear-gradient(#f2f4f6,#c9ced3);color:#1b2430;font:700 13px Arial,sans-serif;letter-spacing:0.03em;text-transform:uppercase;box-shadow:0 1px 0 rgba(255,255,255,0.8) inset,0 1px 2px rgba(0,0,0,0.25);}
button.fap-btn.on{background:linear-gradient(#3f7fc7,#2a5e9c);color:#fff;border-color:#1f4673 !important;}
button.fap-navb{text-align:left;}
.fap-main{flex:1;min-width:0;overflow-y:auto;padding:14px 16px;}
.fap-grid{display:grid;grid-template-columns:repeat(auto-fill,minmax(250px,1fr));gap:12px;}
.fap-grid.two{grid-template-columns:repeat(auto-fill,minmax(320px,1fr));}
.fap-box{background:#eef0f2;border:1px solid #9aa1a8;border-radius:6px;padding:10px 12px;}
.fap-boxh{font:700 12.5px Arial,sans-serif;letter-spacing:0.08em;color:#23405f;border-bottom:1px solid #b9bfc5;padding-bottom:6px;margin-bottom:6px;}
.fap-row{display:flex;justify-content:space-between;gap:10px;padding:4px 0;font:600 13.5px Arial,sans-serif;}
.fap-val{font-weight:700;}
.fap-val.green,.fap-door.green span:last-child{color:#137a3a;}
.fap-val.amber,.fap-door.amber span:last-child,.fap-note.amber{color:#b35f00;}
.fap-door.red span:last-child{color:#c0172b;}
.fap-plane{display:flex;flex-direction:column;gap:10px;background:#eef0f2;border:1px solid #9aa1a8;border-radius:30px 70px 70px 30px;padding:14px 26px;}
.fap-deck{display:flex;align-items:center;gap:10px;}
.fap-deckl{width:94px;flex:none;font:700 12px Arial,sans-serif;color:#23405f;letter-spacing:0.06em;}
.fap-zones{display:flex;gap:6px;flex-wrap:wrap;}
.fap-zone{width:64px;padding:6px 0;border-radius:5px;text-align:center;border:1px solid #7d858d;display:flex;flex-direction:column;}
.fap-zone.ok{background:#bfe3c8;}
.fap-zone.cold{background:#bcd6f2;}
.fap-zone.hot{background:#f4c3b9;}
.fap-zone.na{background:#d2d6da;color:#6a737c;}
.fap-zl{font:700 11px Arial,sans-serif;}
.fap-zt{font:700 15px Arial,sans-serif;}
.fap-sel{display:flex;align-items:center;gap:12px;margin:14px 0 6px;font:700 14px Arial,sans-serif;}
.fap-selv{font:700 18px Arial,sans-serif;min-width:80px;text-align:center;}
.fap-note{font:600 12.5px Arial,sans-serif;color:#45505c;margin:8px 0;}
.fap-doors{display:grid;grid-template-columns:repeat(auto-fill,minmax(150px,1fr));gap:8px;margin-bottom:12px;}
.fap-door{display:flex;justify-content:space-between;background:#eef0f2;border:1px solid #9aa1a8;border-radius:5px;padding:8px 10px;font:700 13px Arial,sans-serif;}
.fap-door.na span:last-child{color:#6a737c;}
.fap-door.armed{border-color:#2a5e9c;box-shadow:inset 3px 0 0 #2a5e9c;}
.fap-tanks{display:flex;gap:18px;justify-content:center;margin:6px 0 10px;}
.fap-tank{display:flex;flex-direction:column;align-items:center;gap:4px;font:700 13px Arial,sans-serif;}
.fap-bar{width:44px;height:110px;border:2px solid #4a545e;border-radius:4px;background:#f6f7f8;display:flex;align-items:flex-end;overflow:hidden;}
.fap-fill{width:100%;}
.fap-fill.water{background:#3b8fd6;}
.fap-fill.waste{background:#8a7a5c;}
.fap-fill.full{background:#c96a1b;}
.fap-small{font:600 11px Arial,sans-serif;color:#45505c;}
.fap-btns{display:flex;gap:8px;}
.fap-btns.wrap{flex-wrap:wrap;}
.fap-range{width:100%;accent-color:#2a5e9c;}
.fap-foot{display:flex;gap:10px;flex-wrap:wrap;padding:8px 12px;background:#1d2024;min-height:22px;}
.fap-caut{font:700 13px Arial,sans-serif;letter-spacing:0.05em;}
.fap-caut.red{color:#ff4a4a;}
.fap-caut.amber{color:#ffa31a;}
.fap-caut.cyan{color:#38d6ff;}
.fap-caut.none{color:#6fd18b;}
`;
  document.head.append(s);
})();

const OP_TRENT970 = { 'xphfbw.engineType': 'TRENT900', 'xphfbw.engineRating': '970' };
const OP_TRENT972 = { 'xphfbw.engineType': 'TRENT900', 'xphfbw.engineRating': '972' };
const OP_GP7270 = { 'xphfbw.engineType': 'GP7200', 'xphfbw.engineRating': 'GP7270' };

const OPERATORS = [
  { code: 'AF', name: 'Air France', re: /air ?france|\bf-hpj/i, model: 'A380-861', values: OP_GP7270 },
  { code: 'NH', name: 'ANA', re: /\bana\b|all nippon|\bja38\d/i, model: 'A380-841', values: OP_TRENT970 },
  { code: 'OZ', name: 'Asiana', re: /asiana|\bhl76[23]\d/i, model: 'A380-841', values: OP_TRENT970 },
  { code: 'BA', name: 'British Airways', re: /british|\bg-xle/i, model: 'A380-841', values: OP_TRENT970 },
  { code: 'CZ', name: 'China Southern', re: /china southern|\bb-61(3[6-9]|40)/i, model: 'A380-841', values: OP_TRENT970 },
  { code: 'EKR', name: 'Emirates (Rolls-Royce)', re: /\ba6-e[uv]/i, model: 'A380-842', values: OP_TRENT972 },
  { code: 'EK', name: 'Emirates', re: /emirates|\ba6-e[deo]/i, model: 'A380-861', values: OP_GP7270 },
  { code: 'EY', name: 'Etihad', re: /etihad|\ba6-ap/i, model: 'A380-861', values: OP_GP7270 },
  { code: 'KE', name: 'Korean Air', re: /korean|\bhl761\d/i, model: 'A380-861', values: OP_GP7270 },
  { code: 'LH', name: 'Lufthansa', re: /lufthansa|\bd-aim/i, model: 'A380-841', values: OP_TRENT970 },
  { code: 'MH', name: 'Malaysia Airlines', re: /malaysia|\b9m-mn/i, model: 'A380-841', values: OP_TRENT970 },
  { code: 'QF', name: 'Qantas', re: /qantas|\bvh-oq/i, model: 'A380-842', values: OP_TRENT972 },
  { code: 'QR', name: 'Qatar Airways', re: /qatar|\ba7-ap/i, model: 'A380-861', values: OP_GP7270 },
  { code: 'SQ', name: 'Singapore Airlines', re: /singapore|\b9v-sk/i, model: 'A380-841', values: OP_TRENT970 },
  { code: 'TG', name: 'Thai Airways', re: /thai|\bhs-tu/i, model: 'A380-841', values: OP_TRENT970 },
];

let opLastKey = null;
let opLiveryName = null;

function opFind(livery) {
  return livery ? OPERATORS.find((o) => o.re.test(livery)) || null : null;
}

function opCurrent(key) {
  return String(settings[key] ?? DEFAULTS[key] ?? '');
}

function opUserKeys() {
  try { return JSON.parse(settings['xphfbw.userKeys'] || '[]'); } catch { return []; }
}

async function opSave(values, message) {
  if (!Object.keys(values).length) return;
  const r = await bridge.setSettings(values);
  if (r && r.ok === false) return;
  settings = { ...settings, ...values };
  fill();
  if (message) show(message);
}

async function opFromLivery(livery, fileValues) {
  let prev = {};
  try { prev = JSON.parse(settings['xphfbw.liveryValues'] || '{}'); } catch {}
  const op = opFind(livery);
  const wanted = { ...(op ? op.values : {}), ...(fileValues || {}) };
  const out = {};
  const mine = opUserKeys();
  const untouched = (k) => !mine.includes(k) && (opCurrent(k) === String(DEFAULTS[k] ?? '') || (k in prev && String(prev[k]) === opCurrent(k)));
  for (const [k, v] of Object.entries(wanted)) if (untouched(k) && opCurrent(k) !== String(v)) out[k] = v;
  for (const k of Object.keys(prev)) if (!(k in wanted) && String(prev[k]) === opCurrent(k) && k in DEFAULTS) out[k] = DEFAULTS[k];
  out['xphfbw.liveryValues'] = JSON.stringify(wanted);
  const changed = Object.keys(out).filter((k) => k !== 'xphfbw.liveryValues').length;
  await opSave(out, changed ? `${op ? op.name : livery}: ${changed} operator setting${changed > 1 ? 's' : ''} from the livery` : '');
}

async function opPoll() {
  if (FIXTURE || study.preview) return;
  let r;
  try { r = await (await fetch(PLUGIN_ORIGIN + '/efb/livery')).json(); } catch { return; }
  opLiveryName = r.livery || null;
  const key = JSON.stringify([r.livery || '', r.operator || {}, settings['xphfbw.operator'] || 'AUTO']);
  if (key === opLastKey) return;
  try {
    const names = [1, 2, 3, 4].map((n) => `A32NX_ENGINE_STATE:${n}`);
    const v = (await studyGet('/vars?names=' + names.map(encodeURIComponent).join(','))).vars || {};
    if (names.some((n) => (v[n] || 0) !== 0)) return;
  } catch { return; }
  opLastKey = key;
  if ((settings['xphfbw.operator'] || 'AUTO') !== 'AUTO') return;
  await opFromLivery(r.livery || '', r.operator || null);
}

function opStart() {
  opPoll();
  setInterval(opPoll, 15000);
}

document.addEventListener('change', async (e) => {
  const t = e.target;
  if (!t.matches || !t.matches('#settings-content [data-key="xphfbw.operator"]')) return;
  const code = t.value;
  opLastKey = null;
  if (code === 'AUTO') {
    setTimeout(async () => {
      let livery = {};
      try { livery = JSON.parse(settings['xphfbw.liveryValues'] || '{}'); } catch {}
      const managed = new Set([...OPERATORS.flatMap((o) => Object.keys(o.values)), ...Object.keys(livery)]);
      await opSave({ 'xphfbw.userKeys': JSON.stringify(opUserKeys().filter((k) => !managed.has(k))) });
      opPoll();
    }, 300);
    return;
  }
  const op = OPERATORS.find((o) => o.code === code);
  if (!op) return;
  const out = {};
  for (const [k, v] of Object.entries(op.values)) if (opCurrent(k) !== String(v)) out[k] = v;
  setTimeout(() => opSave(out, `${op.name} (${op.model}) configuration applied`), 300);
});

const NADP = {
  STD: { CONFIG_THR_RED_ALT: '1500', CONFIG_ACCEL_ALT: '1500' },
  NADP1: { CONFIG_THR_RED_ALT: '800', CONFIG_ACCEL_ALT: '3000' },
  NADP2: { CONFIG_THR_RED_ALT: '1000', CONFIG_ACCEL_ALT: '1000' },
};

document.addEventListener('change', (e) => {
  const t = e.target;
  if (!t.matches || !t.matches('#settings-content [data-key="xphfbw.nadp"]')) return;
  const preset = NADP[t.value];
  if (!preset) return;
  const out = {};
  for (const [k, v] of Object.entries(preset)) if (opCurrent(k) !== v) out[k] = v;
  setTimeout(() => opSave(out, `${t.value === 'STD' ? 'Standard' : t.value} departure: thrust reduction ${preset.CONFIG_THR_RED_ALT} ft, acceleration ${preset.CONFIG_ACCEL_ALT} ft`), 300);
});

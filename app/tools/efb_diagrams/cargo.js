const CARGO_HOLDS = { fwd: { name: 'FWD hold', maxKg: 28577 }, aft: { name: 'AFT hold', maxKg: 20310 }, bulk: { name: 'Bulk hold', maxKg: 2513 } };

const CARGO_ITEMS = [
  { id: 'gen', name: 'General cargo', unit: 'PMC pallet', code: 'GEN', kg: 3000, holds: ['fwd', 'aft'], dims: [3.18, 2.44, 1.60] },
  { id: 'xps', name: 'Express parcels', unit: 'LD3 container', code: 'XPS', kg: 900, holds: ['fwd', 'aft'], dims: [1.56, 1.53, 1.63] },
  { id: 'mal', name: 'Mail', unit: 'LD3 container', code: 'MAL', kg: 700, holds: ['fwd', 'aft'], dims: [1.56, 1.53, 1.63] },
  { id: 'pil', name: 'Medical supplies, 2 to 8 °C', unit: 'active container (LD3 size)', code: 'PIL ACT', kg: 1100, holds: ['fwd', 'aft'], dims: [1.56, 1.53, 1.63], note: 'Temperature controlled; keep the hold heating on.' },
  { id: 'ice', name: 'Vaccines on dry ice', unit: 'insulated box, 200 kg dry ice', code: 'PIL ICE', kg: 450, holds: ['fwd', 'aft'], dims: [1.2, 1.0, 1.0], dg: 'UN1845 Dry ice, class 9, 200 kg', dryIceKg: 200 },
  { id: 'per', name: 'Fresh fish and flowers', unit: 'LD3 container', code: 'PER', kg: 1200, holds: ['fwd', 'aft'], dims: [1.56, 1.53, 1.63] },
  { id: 'avi_pet', name: 'Dogs and cats in kennels', unit: 'kennel', code: 'AVI', kg: 60, holds: ['bulk'], dims: [1.0, 0.7, 0.8], notoc: true, note: 'Bulk hold only: heated and ventilated. Set BULK heating on.' },
  { id: 'veh_kit', name: 'Formula 1 freight (spares, tyres, tools)', unit: 'PMC pallet', code: 'VEH', kg: 2500, holds: ['fwd', 'aft'], dims: [3.18, 2.44, 1.55] },
  { id: 'val', name: 'Valuables (banknotes, gold)', unit: 'secure LD3', code: 'VAL', kg: 500, holds: ['fwd'], dims: [1.56, 1.53, 1.63], notoc: true },
  { id: 'hum', name: 'Human remains', unit: 'coffin on skid', code: 'HUM', kg: 250, holds: ['fwd', 'aft'], dims: [2.2, 0.8, 0.7], notoc: true },
  { id: 'eli', name: 'Electronics with lithium batteries', unit: 'LD3 container', code: 'ELI', kg: 800, holds: ['fwd', 'aft'], dims: [1.56, 1.53, 1.63], dg: 'UN3481 Lithium ion batteries packed with equipment, class 9, PI 967' },
  { id: 'mag', name: 'Magnetised material', unit: 'crate', code: 'MAG', kg: 300, holds: ['aft'], dims: [1.2, 1.0, 0.8], dg: 'UN2807 Magnetized material, class 9' },
];

let cargoQty = {};
try { cargoQty = JSON.parse(localStorage.getItem('efb.cargoQty') || '{}'); } catch {}
let cargoAdded = { fwd: 0, aft: 0, bulk: 0 };
try { cargoAdded = { ...cargoAdded, ...JSON.parse(localStorage.getItem('efb.cargoAdded') || '{}') }; } catch {}
let cargoMsg = '';

function cargoSave() {
  try { localStorage.setItem('efb.cargoQty', JSON.stringify(cargoQty)); localStorage.setItem('efb.cargoAdded', JSON.stringify(cargoAdded)); } catch {}
}

function cargoPlan() {
  const load = { fwd: 0, aft: 0, bulk: 0 };
  const lines = [];
  const notoc = [];
  let dryIce = 0;
  for (const it of CARGO_ITEMS) {
    const q = cargoQty[it.id] || 0;
    if (!q) continue;
    let left = q;
    for (const h of it.holds) {
      while (left > 0 && load[h] + it.kg <= CARGO_HOLDS[h].maxKg) { load[h] += it.kg; left--; lines.push([h, it]); }
    }
    if (left > 0) return { error: `${it.name}: no room left in ${it.holds.map((h) => CARGO_HOLDS[h].name).join(' or ')}.` };
    if (it.dg || it.notoc) for (let k = 0; k < q; k++) notoc.push(it);
    dryIce += (it.dryIceKg || 0) * q;
  }
  const warn = dryIce > 1000 ? `Dry ice ${dryIce} kg: above the 1000 kg operator limit for the lower deck.` : '';
  return { load, lines, notoc, warn };
}

function cargoNotocText(plan) {
  const head = ['NOTOC - NOTIFICATION TO CAPTAIN', `ISSUED ${new Date().toISOString().slice(0, 16).replace('T', ' ')}Z`, ''];
  if (!plan.notoc.length) return head.concat(['NO DANGEROUS GOODS OR SPECIAL LOAD ON BOARD']).join('\n');
  const rows = plan.notoc.map((it) => {
    const hold = plan.lines.find(([, x]) => x === it)?.[0] || it.holds[0];
    return `${it.code.padEnd(8)} ${CARGO_HOLDS[hold].name.padEnd(10)} ${String(it.kg).padStart(5)} KG  ${it.dg || it.name.toUpperCase()}`;
  });
  return head.concat(['CODE     POSITION   WEIGHT   DESCRIPTION'], rows).join('\n');
}

async function cargoHoldDesired() {
  try {
    const r = await studyGet('/efb/state');
    const v = (r && r.values) || {};
    return { fwd: v['payload/cargo/fwd/kg_desired'] || 0, aft: v['payload/cargo/aft/kg_desired'] || 0, bulk: v['payload/cargo/bulk/kg_desired'] || 0 };
  } catch { return null; }
}

async function cargoApply(remove) {
  const plan = cargoPlan();
  if (plan.error && !remove) { cargoMsg = plan.error; cargoRender(); return; }
  const now = await cargoHoldDesired();
  if (!now) { cargoMsg = 'The aircraft is not connected.'; cargoRender(); return; }
  for (const h of Object.keys(CARGO_HOLDS)) {
    const base = Math.max(0, now[h] - cargoAdded[h]);
    const add = remove ? 0 : plan.load[h];
    if (base + add > CARGO_HOLDS[h].maxKg) { cargoMsg = `${CARGO_HOLDS[h].name}: ${Math.round(base + add)} kg is over its ${CARGO_HOLDS[h].maxKg} kg limit.`; cargoRender(); return; }
    const r = await efbSet(`payload/cargo/${h}/kg_desired`, base + add);
    if (r && r.ok === false) { cargoMsg = r.message || 'Not accepted'; cargoRender(); return; }
    cargoAdded[h] = add;
  }
  cargoSave();
  cargoMsg = remove ? 'Special cargo removed from the holds.' : 'Special cargo added to the hold targets. Press Board on Payload & Fuel to load it.';
  cargoRender();
}

async function cargoOfp() {
  if (loadsheet.ofp) return loadsheet.ofp;
  const id = settings.CONFIG_OVERRIDE_SIMBRIEF_USERID || '';
  if (!id) throw new Error('Set your SimBrief user ID in Settings > Connections first.');
  const r = await fetch(`https://www.simbrief.com/api/xml.fetcher.php?userid=${encodeURIComponent(id)}&json=v2`);
  const ofp = await r.json();
  if (ofp.fetch && ofp.fetch.status && ofp.fetch.status !== 'Success') throw new Error('SimBrief: ' + ofp.fetch.status);
  renderFlightPlan(ofp);
  return loadsheet.ofp;
}

async function cargoImportSimbrief() {
  let o;
  try { o = await cargoOfp(); } catch (e) { cargoMsg = e.message || 'SimBrief: could not fetch'; cargoRender(); return; }
  const r = await efbSet('payload/cargo_target_kg', o.freightKg);
  if (r && r.ok === false) { cargoMsg = r.message || 'Not accepted'; cargoRender(); return; }
  let now = null;
  for (let k = 0; k < 15; k++) {
    await new Promise((done) => setTimeout(done, 200));
    now = await cargoHoldDesired();
    if (now && Math.abs(now.fwd + now.aft + now.bulk - o.freightKg) < 1) break;
  }
  cargoAdded = { fwd: 0, aft: 0, bulk: 0 };
  cargoSave();
  if (CARGO_ITEMS.some((it) => cargoQty[it.id])) await cargoApply(false);
  cargoMsg = `SimBrief freight ${o.freightKg.toLocaleString()} kg set as the cargo target${CARGO_ITEMS.some((it) => cargoQty[it.id]) ? ', special loads on top' : ''}. Press Board on Payload & Fuel to load it.`;
  cargoRender();
}

function cargoRender() {
  const c = document.getElementById('cargo-content');
  if (!c) return;
  c.innerHTML = '';
  const plan = cargoPlan();
  const rows = CARGO_ITEMS.map((it) => {
    const q = cargoQty[it.id] || 0;
    const minus = el('button', { type: 'button', class: 'cg-step', text: '−' });
    const plus = el('button', { type: 'button', class: 'cg-step', text: '+' });
    minus.addEventListener('click', () => { cargoQty[it.id] = Math.max(0, q - 1); cargoSave(); cargoRender(); });
    plus.addEventListener('click', () => { cargoQty[it.id] = q + 1; cargoSave(); cargoRender(); });
    const status = el('div', { class: 'cg-meta', text: `${it.unit.charAt(0).toUpperCase() + it.unit.slice(1)}, ${it.kg} kg each, ${it.holds.map((h) => CARGO_HOLDS[h].name).join(' / ')}${it.dg ? ' · ' + it.dg : ''}${it.note ? ' · ' + it.note : ''}` });
    return el('div', { class: 'cg-row' },
      el('div', { class: 'cg-main' }, el('div', { class: 'cg-name' }, el('span', { class: 'cg-code', text: it.code }), el('span', { text: it.name })), status),
      el('div', { class: 'cg-qty' }, minus, el('span', { class: 'cg-n', text: String(q) }), plus));
  });
  const holds = el('div', { class: 'cg-holds' }, ...Object.entries(CARGO_HOLDS).map(([h, d]) => el('div', { class: 'cg-hold' }, el('div', { class: 'cg-hn', text: d.name }), el('div', { class: 'cg-hv', text: `${(plan.load ? plan.load[h] : 0).toLocaleString()} kg` }), el('div', { class: 'cg-hm', text: `of ${d.maxKg.toLocaleString()} kg` }))));
  const load = el('button', { type: 'button', class: 'primary', text: 'Add to hold targets' });
  load.addEventListener('click', () => cargoApply(false));
  const clear = el('button', { type: 'button', text: 'Remove special cargo' });
  clear.addEventListener('click', () => cargoApply(true));
  const simbrief = el('button', { type: 'button', text: 'Import from SimBrief' });
  simbrief.addEventListener('click', cargoImportSimbrief);
  const notocText = plan.error ? '' : cargoNotocText(plan);
  const save = el('button', { type: 'button', text: 'Save NOTOC to Documents' });
  save.addEventListener('click', async () => {
    const r = await studyPost('/oit/export', { name: 'NOTOC', text: notocText });
    cargoMsg = r && r.ok ? 'NOTOC saved to Documents/Exports.' : 'Could not save the NOTOC.';
    cargoRender();
  });
  const msg = cargoMsg || plan.error || plan.warn || '';
  c.append(el('div', { class: 'cg-wrap' },
    el('div', { class: 'cg-list' }, el('div', { class: 'section-title', text: 'Special loads' }), ...rows),
    el('div', { class: 'cg-side' }, el('div', { class: 'section-title', text: 'Holds' }), holds,
      el('div', { class: 'cg-actions' }, simbrief, load, clear),
      msg ? el('div', { class: 'cg-msg', text: msg }) : el('div'),
      el('div', { class: 'section-title', text: 'NOTOC' }), el('pre', { class: 'cg-notoc', text: notocText }), save)));
}

function cargoOnShow() {
  cargoMsg = '';
  cargoRender();
}

(function () {
  const s = document.createElement('style');
  s.textContent = `
.cg-wrap{display:grid;grid-template-columns:minmax(0,1.5fr) minmax(320px,1fr);gap:20px;align-items:start;}
.cg-list{display:flex;flex-direction:column;gap:8px;}
.cg-row{display:flex;justify-content:space-between;align-items:center;gap:14px;padding:12px 14px;border-radius:12px;background:#2a2c30;border:1px solid rgba(255,255,255,0.07);}
.cg-name{display:flex;align-items:center;gap:10px;font:700 15px 'Manrope','Segoe UI',sans-serif;color:#f2f2f2;}
.cg-code{font:800 11px 'Manrope','Segoe UI',sans-serif;letter-spacing:0.06em;padding:3px 7px;border-radius:6px;background:#3c3f43;color:#c9cbce;}
.cg-meta{font:500 13px 'Manrope','Segoe UI',sans-serif;color:#c4c6ca;margin-top:4px;}
.cg-qty{display:flex;align-items:center;gap:8px;flex:none;}
button.cg-step{width:36px;height:36px;padding:0;flex:none;font:700 18px 'Manrope','Segoe UI',sans-serif;}
.cg-n{min-width:28px;text-align:center;font:700 16px 'Manrope','Segoe UI',sans-serif;color:#fff;}
.cg-holds{display:grid;grid-template-columns:repeat(3,1fr);gap:8px;}
.cg-hold{padding:12px;border-radius:12px;background:#2a2c30;border:1px solid rgba(255,255,255,0.07);}
.cg-hn{font:800 11.5px 'Manrope','Segoe UI',sans-serif;letter-spacing:0.08em;color:#9a9da2;text-transform:uppercase;}
.cg-hv{font:700 18px 'Manrope','Segoe UI',sans-serif;color:#f2f2f2;margin-top:4px;}
.cg-hm{font:500 12px 'Manrope','Segoe UI',sans-serif;color:#9a9da2;}
.cg-actions{display:flex;gap:10px;margin:14px 0 8px;flex-wrap:wrap;}
.cg-msg{font:600 13.5px 'Manrope','Segoe UI',sans-serif;color:#f4c542;margin:6px 0 10px;}
.cg-notoc{font:500 12.5px 'Cascadia Mono','Consolas',monospace;color:#e6e7e9;background:#1f2124;border:1px solid rgba(255,255,255,0.08);border-radius:10px;padding:12px;white-space:pre-wrap;margin:0 0 10px;}
`;
  document.head.append(s);
})();

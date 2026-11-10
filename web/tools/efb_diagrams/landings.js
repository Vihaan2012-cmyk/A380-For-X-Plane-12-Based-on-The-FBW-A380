let lrList = null;
const lrAirports = new Map();

function lrAirport(l) {
  const key = `${l.lat.toFixed(3)},${l.lon.toFixed(3)}`;
  if (lrAirports.has(key)) return lrAirports.get(key) || '';
  lrAirports.set(key, null);
  fetch(`/api/nearest?lat=${l.lat}&lon=${l.lon}`).then((r) => r.json()).then((r) => {
    if (r && r.icao && r.distance_m < 8000) {
      lrAirports.set(key, r.icao);
      if (document.getElementById('tab-home')?.classList.contains('active') && typeof renderHome === 'function') renderHome();
    }
  }).catch(() => {});
  return '';
}

function lrTime(zulu) {
  const s = Math.floor(Number(zulu) || 0);
  return `${String(Math.floor(s / 3600) % 24).padStart(2, '0')}:${String(Math.floor(s / 60) % 60).padStart(2, '0')}Z`;
}

function lrClass(l) {
  return /^Hard/.test(l.grade) ? 'hard' : l.grade === 'Firm' ? 'firm' : 'ok';
}

function homeLandings() {
  const box = el('div', { class: 'box lr-box' }, el('div', { class: 'home-sec', text: 'Landings' }));
  const list = lrList || [];
  if (!list.length) {
    box.append(el('div', { class: 'label dim', text: FIXTURE ? 'Touchdowns appear here once X-Plane is running.' : 'No landing recorded yet. Each touchdown is graded against the A380 limit descent velocity.' }));
    return box;
  }
  const l = list[0];
  box.append(el('div', { class: 'lr-big ' + lrClass(l) }, el('span', { class: 'mono', text: `${Math.round(l.vs_fpm)} fpm` }), el('span', { class: 'mono dim', text: `${Number(l.g).toFixed(2)} g` })));
  box.append(el('div', { class: 'label lr-grade ' + lrClass(l), text: [lrAirport(l), lrTime(l.zulu_s), l.grade].filter(Boolean).join('  ·  ') }));
  box.append(el('div', { class: 'home-kv' }, el('span', { text: 'Speed' }), el('span', { class: 'v', text: `${Math.round(l.ias_kt)} kt IAS / ${Math.round(l.gs_kt)} kt GS` })));
  box.append(el('div', { class: 'home-kv' }, el('span', { text: 'Pitch / bank' }), el('span', { class: 'v', text: `${l.pitch_deg.toFixed(1)}° / ${l.roll_deg.toFixed(1)}°` })));
  box.append(el('div', { class: 'home-kv' }, el('span', { text: 'Weight' }), el('span', { class: 'v', text: `${(l.weight_kg / 1000).toFixed(1)} t (limit ${Math.round(l.limit_fpm)} fpm)` })));
  if (l.bounces) box.append(el('div', { class: 'home-kv' }, el('span', { text: 'Bounces' }), el('span', { class: 'v', text: String(l.bounces) })));
  if (list.length > 1) {
    box.append(el('div', { class: 'home-hr' }));
    for (const p of list.slice(1, 6)) {
      box.append(el('div', { class: 'home-kv lr-row' }, el('span', { text: [lrTime(p.zulu_s), lrAirport(p)].filter(Boolean).join('  ') }), el('span', { class: 'v lr-' + lrClass(p), text: `${Math.round(p.vs_fpm)} fpm  ${Number(p.g).toFixed(2)} g` })));
    }
  }
  return box;
}

async function lrPoll() {
  if (FIXTURE) return;
  let list;
  try { list = await (await fetch(PLUGIN_ORIGIN + '/efb/landings', { cache: 'no-store' })).json(); } catch { return; }
  if (!Array.isArray(list)) return;
  const first = lrList === null;
  const fresh = !first && list.length > 0 && (!lrList.length || list[0].zulu_s !== lrList[0].zulu_s || list.length !== lrList.length);
  lrList = list;
  if (fresh) {
    const l = list[0];
    show(`Touchdown ${Math.round(l.vs_fpm)} fpm, ${Number(l.g).toFixed(2)} g${l.bounces ? `, ${l.bounces} bounce${l.bounces > 1 ? 's' : ''}` : ''}: ${l.grade.split(':')[0]}`);
  }
  if ((fresh || (first && list.length)) && document.getElementById('tab-home')?.classList.contains('active') && typeof renderHome === 'function') renderHome();
}

setInterval(lrPoll, 10000);
setTimeout(lrPoll, 2000);

(function () {
  const style = document.createElement('style');
  style.textContent = `
.lr-big{display:flex;align-items:baseline;gap:14px;margin:4px 0 2px;}
.lr-big .mono:first-child{font-size:30px;font-weight:700;}
.lr-big.ok .mono:first-child{color:#4cc36a;}
.lr-big.firm .mono:first-child{color:#f0a020;}
.lr-big.hard .mono:first-child{color:#ff4d4d;}
.lr-grade{margin-bottom:8px;}
.lr-grade.hard{color:#ff6b6b;}
.lr-ok{color:#4cc36a;}.lr-firm{color:#f0a020;}.lr-hard{color:#ff4d4d;}
`;
  document.head.append(style);
})();

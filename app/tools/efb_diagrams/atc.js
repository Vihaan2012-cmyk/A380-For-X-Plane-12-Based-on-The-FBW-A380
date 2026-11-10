const ATC_TYPES = [['', 'All'], ['ATIS', 'ATIS'], ['DELIVERY', 'Delivery'], ['GROUND', 'Ground'], ['TOWER', 'Tower'], ['APPROACH', 'Approach'], ['RADAR', 'Centre']];
const ATC_VARS = ['PLANE LATITUDE', 'PLANE LONGITUDE', 'COM ACTIVE FREQUENCY:1', 'COM STANDBY FREQUENCY:1'];
const atc = { list: null, error: '', filter: '', search: '', selected: '', loadedAt: 0, active: '', standby: '', timer: null, busy: false };

function atcNetwork() {
  return settings['xphfbw.atcNetwork'] === 'IVAO' ? 'IVAO' : 'VATSIM';
}

async function atcSetNetwork(n) {
  if (n === atcNetwork()) return;
  const r = await bridge.setSettings({ 'xphfbw.atcNetwork': n });
  if (r && r.ok === false) return;
  settings = { ...settings, 'xphfbw.atcNetwork': n };
  atc.list = null;
  atc.selected = '';
  atcRender();
  atcLoad(true);
}

function atcBcd(v) {
  const n = Math.round(Number(v) || 0);
  if (!n) return '';
  const d = n.toString(16).padStart(7, '0');
  return `${d.slice(0, 3)}.${d.slice(3, 6)}`;
}

async function atcLoad(force) {
  if (atc.busy || FIXTURE) return;
  if (!force && atc.list && Date.now() - atc.loadedAt < 60000) return;
  atc.busy = true;
  try {
    const v = (await studyGet('/vars?names=' + ATC_VARS.map(encodeURIComponent).join(','))).vars || {};
    const lat = v['PLANE LATITUDE'], lon = v['PLANE LONGITUDE'];
    if (lat === undefined || lon === undefined) { atc.error = 'Aircraft position not available yet.'; return; }
    const r = await fetch(`${PLUGIN_ORIGIN}/atc/online?lat=${lat}&lon=${lon}&source=${atcNetwork().toLowerCase()}`);
    const j = await r.json();
    if (!r.ok) { atc.error = j.error || `ATC list unavailable (${r.status})`; return; }
    atc.list = j.stations || [];
    atc.error = '';
    atc.loadedAt = Date.now();
  } catch {
    atc.error = 'ATC list unavailable: the plugin is not answering.';
  } finally {
    atc.busy = false;
  }
  atcRender();
}

async function atcPollRadio() {
  if (!document.getElementById('tab-atc')?.classList.contains('active')) { clearInterval(atc.timer); atc.timer = null; return; }
  try {
    const v = (await studyGet('/vars?names=' + ATC_VARS.slice(2).map(encodeURIComponent).join(','))).vars || {};
    const a = atcBcd(v['COM ACTIVE FREQUENCY:1']), s = atcBcd(v['COM STANDBY FREQUENCY:1']);
    if (a !== atc.active || s !== atc.standby) { atc.active = a; atc.standby = s; atcRender(); }
  } catch {}
  atcLoad(false);
}

async function atcTune(which, freq) {
  const hz = Math.round(parseFloat(freq) * 1e6);
  const r = await studyPost('/study/action', { kind: 'tuneCom', which, hz });
  if (r && r.ok === false) return;
  show(`COM 1 ${which === 'active' ? 'active' : 'standby'} ${freq}`);
}

function atcRender() {
  const host = document.getElementById('atc-content');
  if (!host) return;
  const keep = host.querySelector('.atc-list')?.scrollTop || 0;
  host.innerHTML = '';
  const head = el('div', { class: 'atc-head' },
    el('div', { class: 'atc-radio' }, el('span', { class: 'label dim', text: 'COM 1 ACTIVE' }), el('span', { class: 'mono atc-f', text: atc.active || '---.---' })),
    el('div', { class: 'atc-radio' }, el('span', { class: 'label dim', text: 'STANDBY' }), el('span', { class: 'mono atc-f', text: atc.standby || '---.---' })));
  const nets = el('div', { class: 'atc-nets' });
  for (const n of ['VATSIM', 'IVAO']) {
    const b = el('button', { type: 'button', class: 'pc-tab' + (atcNetwork() === n ? ' on' : ''), text: n });
    b.addEventListener('click', () => atcSetNetwork(n));
    nets.append(b);
  }
  const refresh = el('button', { type: 'button', class: 'small grey', text: atc.busy ? 'Loading…' : 'Refresh' });
  refresh.addEventListener('click', () => atcLoad(true));
  head.append(nets, refresh);
  host.append(head);
  const filters = el('div', { class: 'atc-filters' });
  for (const [k, label] of ATC_TYPES) {
    const b = el('button', { type: 'button', class: 'pc-tab' + (atc.filter === k ? ' on' : ''), text: label });
    b.addEventListener('click', () => { atc.filter = k; atcRender(); });
    filters.append(b);
  }
  const search = el('input', { type: 'text', placeholder: 'Callsign', value: atc.search });
  search.addEventListener('input', () => { atc.search = search.value; atcRender(); const s = document.querySelector('#atc-content .atc-filters input'); s.focus(); s.setSelectionRange(s.value.length, s.value.length); });
  filters.append(search);
  host.append(filters);
  if (atc.error) host.append(el('div', { class: 'label dim', text: atc.error }));
  if (!atc.list) {
    host.append(el('div', { class: 'label dim', text: FIXTURE ? 'Online ATC needs X-Plane running.' : `Loading ${atcNetwork()} controllers near the aircraft…` }));
    return;
  }
  const q = atc.search.trim().toUpperCase();
  const rows = atc.list.filter((s) => (!atc.filter || s.type === atc.filter) && (!q || s.callsign.includes(q)));
  const body = el('div', { class: 'atc-body' });
  const list = el('div', { class: 'atc-list' });
  for (const s of rows) {
    const on = s.frequency === atc.active ? ' tuned' : s.frequency === atc.standby ? ' stby' : '';
    const row = el('div', { class: 'atc-row' + on + (atc.selected === s.callsign ? ' sel' : '') },
      el('div', { class: 'atc-cs' }, el('span', { text: s.callsign }), el('span', { class: 'label dim', text: `${s.type.toLowerCase()} · ${s.distance} nm` })),
      el('span', { class: 'mono atc-f', text: s.frequency }));
    const act = el('button', { type: 'button', class: 'small', text: 'Active' });
    act.addEventListener('click', (e) => { e.stopPropagation(); atcTune('active', s.frequency); });
    const stby = el('button', { type: 'button', class: 'small grey', text: 'Standby' });
    stby.addEventListener('click', (e) => { e.stopPropagation(); atcTune('standby', s.frequency); });
    row.append(act, stby);
    row.addEventListener('click', () => { atc.selected = s.callsign; atcRender(); });
    list.append(row);
  }
  if (!rows.length) list.append(el('div', { class: 'label dim', text: 'No station in range matches.' }));
  const sel = atc.list.find((s) => s.callsign === atc.selected);
  const info = el('div', { class: 'atc-info' }, el('div', { class: 'home-sec', text: sel ? sel.callsign : 'Station information' }));
  const lines = sel ? (sel.atis || []) : [];
  info.append(el('div', { class: 'mono atc-atis', text: lines.length ? lines.join('\n') : sel ? 'No information published.' : 'Tap a station for its ATIS or controller information.' }));
  body.append(list, info);
  host.append(body);
  host.append(el('div', { class: 'label dim', text: `${atcNetwork()} stations whose range covers the aircraft, nearest first${atcNetwork() === 'IVAO' ? ' (IVAO publishes no range: typical ranges by position)' : ''}. Updated ${atc.loadedAt ? new Date(atc.loadedAt).toLocaleTimeString() : '--'}.` }));
  const nl = host.querySelector('.atc-list'); if (nl) nl.scrollTop = keep;
}

function atcOnShow() {
  atcRender();
  atcLoad(false);
  if (!atc.timer) atc.timer = setInterval(atcPollRadio, 2000);
  atcPollRadio();
}

(function () {
  const css = `
.atc-head{display:flex;align-items:center;gap:28px;margin:0 0 14px;}
.atc-radio{display:flex;flex-direction:column;gap:2px;}
.atc-head button.small{flex:none;width:110px;}
.atc-nets{display:flex;gap:8px;margin-left:auto;}
.atc-f{font-size:20px;color:#7fd36b;}
.atc-filters{display:flex;flex-wrap:wrap;gap:8px;align-items:center;margin:0 0 12px;}
.atc-filters input{width:180px;margin-left:auto;}
.atc-body{display:grid;grid-template-columns:minmax(0,1.4fr) minmax(0,1fr);gap:16px;min-height:0;}
.atc-list{display:flex;flex-direction:column;gap:6px;max-height:560px;overflow:auto;}
.atc-row{display:flex;align-items:center;gap:12px;padding:10px 12px;border-radius:12px;background:rgba(255,255,255,0.04);border:1px solid transparent;cursor:pointer;}
.atc-row.sel{border-color:#ff4d6a;}
.atc-row.tuned .atc-f{color:#ff4d6a;}
.atc-row.stby .atc-f{color:#f0a020;}
.atc-row .atc-cs{flex:1;display:flex;flex-direction:column;gap:2px;font:700 14px 'Manrope','Segoe UI',sans-serif;}
.atc-row .atc-f{font-size:15px;}
.atc-row button.small{flex:none;width:84px;}
.atc-info{padding:14px 16px;border-radius:12px;background:rgba(255,255,255,0.04);}
.atc-atis{white-space:pre-wrap;font-size:13px;line-height:1.55;color:var(--text);margin-top:8px;}
`;
  const style = document.createElement('style');
  style.textContent = css;
  document.head.append(style);
})();

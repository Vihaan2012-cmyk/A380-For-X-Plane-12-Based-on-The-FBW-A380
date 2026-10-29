const LP_COUNT = 8;
const LP_POWER = 'A32NX_ELEC_AC_1_BUS_IS_POWERED';
let lpEditing = 0;
let lpArmed = 0;
let lpArmedTimer = null;
let lpPoweredBefore = null;

function lpNames() {
  try {
    const raw = JSON.parse(settings.LIGHT_PRESET_NAMES || '');
    if (raw && raw.dataType === 'Map' && Array.isArray(raw.value)) return new Map(raw.value.map(([k, v]) => [Number(k), String(v)]));
  } catch {}
  return new Map();
}

async function lpSaveName(id, name) {
  const names = lpNames();
  if (name) names.set(id, name); else names.delete(id);
  const value = JSON.stringify({ dataType: 'Map', value: [...names.entries()] });
  const r = await bridge.setSettings({ LIGHT_PRESET_NAMES: value });
  if (r && r.ok === false) return;
  settings = { ...settings, LIGHT_PRESET_NAMES: value };
}

async function lpAction(op, id) {
  const r = await studyPost('/study/action', { kind: 'lightingPreset', op, id });
  if (r && r.ok === false) return;
  const name = lpNames().get(id) || `Preset ${id}`;
  show(op === 'save' ? `${name}: cockpit lighting saved` : `Loading ${name}`);
}

function lpSection(vars) {
  const powered = Number(vars[LP_POWER] || 0) !== 0;
  const names = lpNames();
  const box = el('div', { class: 'lp-panel' });
  box.append(el('div', { class: 'section-title', text: 'Interior lighting presets' }));
  box.append(el('div', { class: 'label dim', text: powered
    ? 'Save the flight deck lighting as it is now, or load a saved set. The lights fade to it over a few seconds.'
    : 'The aircraft must be powered (AC bus 1) to load or save lighting presets.' }));
  const list = el('div', { class: 'lp-list' });
  for (let id = 1; id <= LP_COUNT; id++) {
    const name = names.get(id) || '';
    const row = el('div', { class: 'lp-row' }, el('span', { class: 'lp-n mono', text: String(id) }));
    if (lpEditing === id) {
      const input = el('input', { type: 'text', value: name, placeholder: `Preset ${id}`, maxlength: '28' });
      let closed = false;
      const finish = async (keep) => {
        if (closed) return;
        closed = true;
        lpEditing = 0;
        input.blur();
        if (keep) await lpSaveName(id, input.value.trim());
        groundRefresh();
      };
      input.addEventListener('keydown', (e) => { if (e.key === 'Enter') finish(true); if (e.key === 'Escape') finish(false); });
      input.addEventListener('blur', () => finish(true));
      row.append(input);
      setTimeout(() => { input.focus(); input.select(); }, 0);
    } else {
      const label = el('button', { type: 'button', class: 'lp-name' + (name ? '' : ' dim'), text: name || `Preset ${id}`, title: 'Rename' });
      label.addEventListener('click', () => { lpEditing = id; groundRefresh(); });
      row.append(label);
    }
    const load = el('button', { type: 'button', class: 'small', text: 'Load' });
    load.disabled = !powered;
    load.addEventListener('click', () => lpAction('load', id));
    const save = el('button', { type: 'button', class: 'small grey', text: lpArmed === id ? 'Overwrite?' : 'Save' });
    save.disabled = !powered;
    save.addEventListener('click', () => {
      if (lpArmed !== id) {
        lpArmed = id;
        clearTimeout(lpArmedTimer);
        lpArmedTimer = setTimeout(() => { lpArmed = 0; groundRefresh(); }, 3000);
        groundRefresh();
        return;
      }
      lpArmed = 0;
      lpAction('save', id);
      groundRefresh();
    });
    row.append(load, save);
    list.append(row);
  }
  box.append(list);
  const auto = settings.LIGHT_PRESET_AUTOLOAD === '1'
    ? `Autoload on power-up: day ${lpLabel(settings.LIGHT_PRESET_AUTOLOAD_DAY)}, dawn/dusk ${lpLabel(settings.LIGHT_PRESET_AUTOLOAD_DAWNDUSK)}, night ${lpLabel(settings.LIGHT_PRESET_AUTOLOAD_NIGHT)} (Settings > Misc).`
    : 'Autoload on power-up is off (Settings > Misc).';
  box.append(el('div', { class: 'label dim', text: auto }));
  return box;
}

function lpLabel(v) {
  const id = Number(v || 0);
  return id ? lpNames().get(id) || `preset ${id}` : 'none';
}

function groundRefresh() {
  if (typeof ground === 'object' && ground.redraw) ground.redraw();
}

async function lpAutoloadPoll() {
  if (FIXTURE || study.preview) return;
  let v;
  try { v = (await studyGet('/vars?names=' + encodeURIComponent(LP_POWER) + ',A32NX_IS_READY')).vars || {}; } catch { return; }
  if (!(LP_POWER in v)) return;
  const powered = Number(v[LP_POWER]) !== 0 && Number(v.A32NX_IS_READY ?? 1) !== 0;
  const rising = powered && lpPoweredBefore !== true;
  lpPoweredBefore = powered;
  if (!rising || settings.LIGHT_PRESET_AUTOLOAD !== '1') return;
  const ids = { day: Number(settings.LIGHT_PRESET_AUTOLOAD_DAY || 0), dawnDusk: Number(settings.LIGHT_PRESET_AUTOLOAD_DAWNDUSK || 0), night: Number(settings.LIGHT_PRESET_AUTOLOAD_NIGHT || 0) };
  if (!ids.day && !ids.dawnDusk && !ids.night) return;
  await studyPost('/study/action', { kind: 'lightingPreset', op: 'autoload', ...ids });
}

setInterval(lpAutoloadPoll, 3000);

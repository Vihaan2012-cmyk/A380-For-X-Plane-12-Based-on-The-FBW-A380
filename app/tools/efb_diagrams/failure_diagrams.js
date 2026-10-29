function failEngineOf(f) {
  const m = f.name.match(/\bENG(?:INE)?\s*(\d)/i);
  if (m) return +m[1];
  if (f.id >= 54010 && f.id < 54014) return f.id - 54009;
  if (f.id >= 54000 && f.id < 54004) return f.id - 53999;
  if (Math.floor(f.id / 1000) === 72) return (f.id % 4) + 1;
  return 0;
}

function failIsEngine(ata, f) {
  if (ata === 24 || ata === 29) return false;
  if ((ata >= 70 && ata <= 80) || ata === 54) return failEngineOf(f) > 0;
  return (ata === 26 || ata === 36) && failEngineOf(f) > 0;
}

function failDiagram(fam, members, chosen, severity) {
  const pick = chosen.length ? chosen : members.slice(0, 1);
  if (!pick.length) return null;
  const list = pick.map((u) => ({ ...u.f, active: true, magnitude: severity || 1 }));
  const ata = fam.key === 'part-separation' ? Math.floor(list[0].id / 1000) : parseInt(fam.chapter, 10);
  if (fam.key === 'part-separation') {
    const title = list.length > 1 ? 'PART SEPARATION' : list[0].name.toUpperCase();
    const engines = [...new Set(list.filter((f) => f.id >= 54000 && f.id < 54004).map((f) => f.id - 53999))];
    if (engines.length) failWatchFirePb();
    const leaks = hydSeparationLeaks(list, failFirePb);
    const views = leaks ? [hydView(leaks, title)] : [];
    if (engines.length) {
      elecLoadBreakers();
      const e = { dead: new Set(engines.map((n) => 'GEN' + n)), degraded: new Set(), edge: new Map() };
      views.push(elecView(e, { title, loads: elecLoadsLost(e) }));
      const b = new Map();
      for (const n of engines) for (const k of ['HP', 'IPC', 'PRV', 'PCLR', 'FAV', 'NAI', 'START']) b.set(k + n, 'fail');
      views.push(bleedView(b, title));
    }
    if (views.length) {
      const d = views.length === 1 ? views[0] : el('div', { class: 'ff-diagram-stack' }, ...views);
      if (engines.length) failWatchFirePb.node = d;
      return d;
    }
  }
  if (ata === 24 || /\b(?:AC|DC)[ _]?(?:\d|ESS|EHA|EMER)|\bTR \d|STATIC INV|BUS TIE|CONTACTOR/i.test(list[0].name)) {
    elecLoadBreakers();
    const d = elecDiagram(list.map((f) => f.name).join(' '), fam.title.toUpperCase());
    if (d) return d;
  }
  if (ata === 21 || ata === 36 || (ata === 30 && /ANTI.?ICE|WING|NACELLE/i.test(list[0].name))) {
    const d = airDiagram(list.map((f) => f.name).join(' '), fam.title.toUpperCase());
    if (d) return d;
  }
  if (ata === 27 || ata === 55 || ata === 57) {
    const detail = typeof fctlDetailView === 'function' ? fctlDetailView(list[0].name, fam.title.toUpperCase()) : null;
    if (detail) return detail;
    const tm = fctlTargets(list.map((f) => f.name).join(' '));
    const d = tm ? fctlPlanView(tm, fam.title.toUpperCase()) : null;
    if (d) return d;
  }
  if (ata === 28) {
    const d = fuelDiagram(list.map((f) => f.name).join(' '), fam.title.toUpperCase());
    if (d) return d;
  }
  if (ata === 29) {
    const d = hydDiagram(list.map((f) => f.name).join(' '), fam.title.toUpperCase());
    if (d) return d;
  }
  if (failIsEngine(ata, list[0])) {
    if (list.some((f) => f.id >= 54000 && f.id < 54004)) return null;
    const n = failEngineOf(list[0]);
    return engineSvg({ n1: 85, n3: 96, faults: engFaults(list, n), title: `ENGINE ${n}`, readout: 'CRUISE, N1 85 %' });
  }
  return null;
}

const failFirePb = [false, false, false, false];

function failWatchFirePb() {
  if (failWatchFirePb.timer || typeof studyGet !== 'function') return;
  const names = [1, 2, 3, 4].map((n) => 'A32NX_FIRE_BUTTON_ENG' + n);
  const tick = async () => {
    failWatchFirePb.timer = null;
    if (failWatchFirePb.node && !failWatchFirePb.node.isConnected) return;
    try {
      const v = ((await studyGet('/vars?names=' + names.join(','))) || {}).vars || {};
      const now = names.map((k) => +v[k] > 0.5);
      if (now.some((x, i) => x !== failFirePb[i])) {
        now.forEach((x, i) => { failFirePb[i] = x; });
        if (typeof drawFailures === 'function') drawFailures();
      }
    } catch {}
    if (!failWatchFirePb.timer) failWatchFirePb.timer = setTimeout(tick, 1500);
  };
  failWatchFirePb.timer = setTimeout(tick, 0);
}

function elecLoadBreakers() {
  if (elecBreakers || elecLoadBreakers.busy || typeof studyGet !== 'function') return;
  elecLoadBreakers.busy = true;
  studyGet('/study/breakers').then((d) => { elecBreakers = (d.breakers || []).filter((c) => c.bus); if (typeof drawFailures === 'function') drawFailures(); }).catch(() => {});
}

const exEcamBeforeEngine = ecamDiagram;
ecamDiagram = function (p) {
  const s = (p.s || '').toUpperCase().replace(/^\*/, '');
  const t = p.t.toUpperCase();
  const m = t.match(/^ENG (\d)\b/);
  if (m && (/^ENG/.test(s) || /FIRE/.test(s)) && !/BLEED|GEN|EDP|HYD/.test(t)) {
    const n = +m[1];
    const faults = engFaults([{ id: 1e7, name: t, active: true, magnitude: 1 }], n);
    return engineSvg({ n1: /FAIL|SHUT ?DOWN|FIRE/.test(t) ? 20 : 85, n3: /FAIL|SHUT ?DOWN|FIRE/.test(t) ? 30 : 96, faults, title: `ENGINE ${n}` });
  }
  if (/^(AIR|COND|VENT|CAB PRESS|A-ICE|ANTI ICE)/.test(s)) {
    const d = airDiagram(t, s);
    if (d) return d;
  }
  if (/^F\/CTL/.test(s)) {
    const tm = fctlTargets(t);
    const d = tm ? fctlPlanView(tm, s) : null;
    if (d) return d;
  }
  if (/^FUEL/.test(s)) {
    const d = fuelDiagram(t, s);
    if (d) return d;
  }
  if (/^HYD/.test(s)) {
    const d = hydDiagram(t, s);
    if (d) return d;
  }
  if (/^ELEC/.test(s)) {
    elecLoadBreakers();
    const d = elecDiagram(t, s);
    if (d) return d;
  }
  return exEcamBeforeEngine(p);
};

const SYSTEM_MAPS = [
  ['ELECTRICAL', () => elecView({ dead: new Set(), degraded: new Set(), edge: new Map() }, { title: 'ELECTRICAL NETWORK', full: true })],
  ['HYDRAULICS', () => hydView(null, 'HYDRAULIC SYSTEMS')],
  ['FUEL', () => fuelView(new Map(), 'FUEL SYSTEM')],
  ['FLIGHT CONTROLS', () => fctlPlanView(new Map(), 'FLIGHT CONTROLS')],
  ['LIVE AIRFLOW', null],
  ['BLEED AND PACKS', () => bleedView(new Map(), 'PNEUMATIC SYSTEM')],
  ['CABIN AIR AND VENTILATION', () => ventView(new Map(), 'VENTILATION')],
  ['ENGINE', () => engineSvg({ n1: 85, n3: 96, faults: new Map(), title: 'TRENT 900' })],
];

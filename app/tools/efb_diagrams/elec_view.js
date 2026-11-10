const ELN = {
  GEN1: ['GEN 1', 50, 20, 'src'], GEN2: ['GEN 2', 230, 20, 'src'], GEN3: ['GEN 3', 650, 20, 'src'], GEN4: ['GEN 4', 830, 20, 'src'],
  APUGENA: ['APU GEN A', 380, 20, 'src'], APUGENB: ['APU GEN B', 500, 20, 'src'],
  EXT1: ['EXT 1', 50, 66, 'ext'], EXT2: ['EXT 2', 230, 66, 'ext'], EXT3: ['EXT 3', 650, 66, 'ext'], EXT4: ['EXT 4', 830, 66, 'ext'],
  AC1: ['AC 1', 50, 128, 'bus'], AC2: ['AC 2', 230, 128, 'bus'], AC3: ['AC 3', 650, 128, 'bus'], AC4: ['AC 4', 830, 128, 'bus'],
  ACEHA: ['AC EHA', 60, 300, 'bus'], ACESS: ['AC ESS', 440, 238, 'bus'], ACESSSHED: ['AC ESS SHED', 440, 300, 'bus'], ACEMER: ['AC EMER', 600, 300, 'bus'], ACGND: ['AC GND/FLT', 830, 238, 'bus'],
  EMERGEN: ['EMER GEN', 600, 238, 'src'],
  TR1: ['TR 1', 230, 372, 'tr'], ESSTR: ['ESS TR', 440, 372, 'tr'], TR2: ['TR 2', 650, 372, 'tr'], APUTR: ['APU TR', 830, 372, 'tr'], STATINV: ['STAT INV', 520, 420, 'tr'],
  DC1: ['DC 1', 140, 480, 'bus'], DCESS: ['DC ESS', 380, 480, 'bus'], DC2: ['DC 2', 650, 480, 'bus'], DCAPU: ['DC APU', 830, 480, 'bus'], DCEHA: ['DC EHA', 380, 590, 'bus'], DCGND: ['DC GND/FLT', 520, 640, 'bus'],
  BAT1: ['BAT 1', 20, 590, 'bat'], BATESS: ['BAT ESS', 200, 590, 'bat'], BAT2: ['BAT 2', 650, 590, 'bat'], BATAPU: ['BAT APU', 830, 590, 'bat'],
};
const ELN_W = 120, ELN_H = 34;

const ELN_FEEDS = {
  AC1: [['GEN1'], ['TIE']], AC2: [['GEN2'], ['TIE']], AC3: [['GEN3'], ['TIE']], AC4: [['GEN4'], ['TIE']],
  ACESS: [['AC1'], ['AC4'], ['EMERGEN']], ACESSSHED: [['ACESS']], ACEHA: [['AC3'], ['ACESS']], ACEMER: [['ACESS'], ['STATINV']], ACGND: [['AC3']],
  TR1: [['AC2']], TR2: [['AC3']], ESSTR: [['ACESS']], APUTR: [['AC4']], STATINV: [['BATESS'], ['DCESS']],
  DC1: [['TR1'], ['DC2'], ['BAT1']], DC2: [['TR2'], ['DC1'], ['BAT2']], DCESS: [['ESSTR'], ['DC1'], ['BATESS']], DCAPU: [['APUTR'], ['BATAPU']],
  DCEHA: [['DC2'], ['DCESS']], DCGND: [['TR2']],
};

function elecTargets(name) {
  const n = ' ' + name.toUpperCase().replace(/[_-]/g, ' ') + ' ';
  const out = { dead: new Set(), degraded: new Set(), edge: new Map() };
  const soft = /DEGRAD|DRIFT|EFFICIENCY|MONITOR|WELDED|BINDING|BIAS/.test(n);
  const hit = (id) => (soft ? out.degraded : out.dead).add(id);
  let m;
  if ((m = n.match(/CONTACTOR GEN (\d) LINE/))) return out.edge.set(`GEN${m[1]}>AC${m[1]}`, soft ? 'stuck' : 'open'), out;
  if ((m = n.match(/CONTACTOR APU GEN (\d) LINE/))) return out.edge.set(`APUGEN${m[1] === '1' ? 'A' : 'B'}>TIE`, soft ? 'stuck' : 'open'), out;
  if ((m = n.match(/CONTACTOR TR (\d|ESS|APU) LINE/))) { const t = m[1] === 'ESS' ? 'ESSTR' : m[1] === 'APU' ? 'APUTR' : 'TR' + m[1]; return out.edge.set(`${ELN_FEEDS[t][0][0]}>${t}`, soft ? 'stuck' : 'open'), out; }
  if ((m = n.match(/APU GEN(?:ERATOR)? ?([AB12])/))) hit('APUGEN' + (m[1] === '1' ? 'A' : m[1] === '2' ? 'B' : m[1]));
  else if ((m = n.match(/\b(?:GEN(?:ERATOR)?|VFG) ?(\d)/))) hit('GEN' + m[1]);
  if (/EMER GEN|\bRAT\b/.test(n)) hit('EMERGEN');
  if ((m = n.match(/EXT(?: PWR)? ?(\d)/))) hit('EXT' + m[1]);
  if (/AC ESS SHED/.test(n)) hit('ACESSSHED'); else if (/AC ESS\b/.test(n)) hit('ACESS');
  if (/AC 247XP|AC EHA/.test(n)) hit('ACEHA');
  if (/AC EMER/.test(n)) hit('ACEMER');
  if (/AC GND FLT/.test(n)) hit('ACGND');
  if ((m = n.match(/\bAC ?(\d)\b/))) hit('AC' + m[1]);
  if (/DC ESS/.test(n)) hit('DCESS');
  if (/DC 247PP|DC EHA/.test(n)) hit('DCEHA');
  if (/DC GND FLT/.test(n)) hit('DCGND');
  if (/DC APU|DC 309PP/.test(n)) hit('DCAPU');
  if ((m = n.match(/DC HOT ?(\d)/))) hit('BAT' + m[1]);
  else if ((m = n.match(/\bDC ?(\d)\b/))) hit('DC' + m[1]);
  if (/ESS TR|TR ESS/.test(n)) hit('ESSTR'); else if (/APU TR|TR APU/.test(n)) hit('APUTR'); else if ((m = n.match(/\bTR ?(\d)\b/))) hit('TR' + m[1]);
  if (/STATIC INV|STAT INV/.test(n)) hit('STATINV');
  if (/ESS BAT|BAT ESS/.test(n)) hit('BATESS'); else if (/APU BAT|BAT APU/.test(n)) hit('BATAPU'); else if ((m = n.match(/\bBAT(?:TERY)? ?(\d)/))) hit('BAT' + m[1]);
  if (/BUS TIE/.test(n)) out.edge.set('TIE', soft ? 'stuck' : 'open');
  return out;
}

function elecSolve(t) {
  const live = new Map();
  const via = new Map();
  for (const k of Object.keys(ELN)) if (/^(GEN|EMERGEN|BAT)/.test(k) && !t.dead.has(k)) live.set(k, true);
  const genOk = (i) => !t.dead.has('GEN' + i) && t.edge.get(`GEN${i}>AC${i}`) !== 'open';
  const anyGen = [1, 2, 3, 4].some(genOk);
  if (anyGen) live.delete('EMERGEN');
  const tieOk = t.edge.get('TIE') !== 'open';
  for (let pass = 0; pass < 6; pass++) {
    for (const [bus, feeds] of Object.entries(ELN_FEEDS)) {
      if (t.dead.has(bus)) { live.delete(bus); continue; }
      let fed = null;
      for (const f of feeds) {
        const src = f[0];
        if (t.edge.get(`${src}>${bus}`) === 'open') continue;
        if (src === 'TIE') { if (tieOk && [1, 2, 3, 4].some((i) => i !== +bus.slice(2) && genOk(i))) { fed = 'TIE'; break; } continue; }
        if (/^GEN/.test(src) && !genOk(+src.slice(3))) continue;
        if (src === 'EMERGEN' && anyGen) continue;
        if (src === 'STATINV' && ['AC1', 'AC2', 'AC3', 'AC4'].some((b) => live.get(b))) continue;
        if (/^BAT/.test(src) && feeds[0][0] !== src && live.get(feeds[0][0])) continue;
        if (live.get(src)) { fed = src; break; }
      }
      if (fed) { live.set(bus, true); via.set(bus, fed); } else { live.delete(bus); via.delete(bus); }
    }
  }
  return { live, via };
}

const ELN_LINKS = {
  'DC2>DC1': 'DC 2 → DC 1 INTER-BUS', 'DC1>DC2': 'DC 1 → DC 2 INTER-BUS', 'DC1>DCESS': 'DC 1 → DC ESS INTER-BUS', 'AC4>ACESS': 'AC ESS ALTN FEED', 'EMERGEN>ACESS': 'EMER GEN',
  'ACESS>ACEHA': 'EHA BACKUP FEED', 'STATINV>ACEMER': 'STATIC INVERTER', 'DCESS>DCEHA': 'DC EHA BACKUP', 'BAT1>DC1': 'BAT 1', 'BAT2>DC2': 'BAT 2', 'BATESS>DCESS': 'BAT ESS', 'BATAPU>DCAPU': 'BAT APU',
};

function elecView(t, opts = {}) {
  const W = 1000;
  const { live, via } = elecSolve(t);
  const base = elecSolve({ dead: new Set(), degraded: new Set(), edge: new Map() });
  const loads = opts.loads || [];
  const H = loads.length ? 760 : 690;
  const state = (k) => {
    if (t.dead.has(k)) return 'fail';
    if (t.degraded.has(k)) return 'warn';
    if (base.live.get(k) && !live.get(k)) return 'warn';
    if (via.get(k) && base.via.get(k) && via.get(k) !== base.via.get(k)) return 'alt';
    return live.get(k) ? 'live' : 'off';
  };
  const affected = (k) => !['live', 'off'].includes(state(k));
  const parts = [];
  const pos = (k) => ELN[k];
  const top = (k) => [pos(k)[1] + ELN_W / 2, pos(k)[2]];
  const bot = (k) => [pos(k)[1] + ELN_W / 2, pos(k)[2] + ELN_H];
  const tieY = 200;
  const tieUsed = [1, 2, 3, 4].some((i) => via.get('AC' + i) === 'TIE');
  const tieSt = t.edge.get('TIE') === 'open' ? 'fail' : tieUsed ? 'alt' : 'off';
  parts.push(nsWire([[top('AC1')[0], tieY], [top('AC4')[0], tieY]], tieSt, tieUsed ? '7 5' : ''));
  parts.push(exT(top('AC1')[0] - 8, tieY - 6, 'BUS TIE', { size: 11, fill: tieUsed ? NS.orange : NS.greyText, anchor: 'end' }));
  for (const i of [1, 2, 3, 4]) {
    const st = via.get('AC' + i) === 'TIE' ? 'alt' : 'off';
    parts.push(nsWire([bot('AC' + i), [bot('AC' + i)[0], tieY]], st));
    parts.push(nsElbow(bot('EXT' + i), top('AC' + i), 'off'));
  }
  for (const s of ['APUGENA', 'APUGENB']) parts.push(nsWire([bot(s), [bot(s)[0], tieY]], t.edge.get(`${s}>TIE`) === 'open' ? 'fail' : 'off'));
  const labels = [];
  const active = [];
  for (const [bus, feeds] of Object.entries(ELN_FEEDS)) {
    feeds.forEach((f) => {
      const src = f[0];
      if (src === 'TIE' || /^EXT/.test(src)) return;
      const broken = t.edge.get(`${src}>${bus}`) === 'open';
      const used = via.get(bus) === src;
      const normal = base.via.get(bus) === src;
      let st = 'off';
      if (broken) st = 'fail';
      else if (used && !normal) st = 'alt';
      else if (normal && !used && (affected(src) || affected(bus))) st = 'warn';
      else if (used) st = 'live';
      const a = pos(src)[2] < pos(bus)[2] ? bot(src) : top(src);
      const b = pos(src)[2] < pos(bus)[2] ? top(bus) : bot(bus);
      const lane = { 'DC2>DC1': 532, 'DC1>DC2': 532, 'DC1>DCESS': 546 }[`${src}>${bus}`];
      const pts = lane ? [bot(src), [bot(src)[0], lane], [bot(bus)[0], lane], bot(bus)] : null;
      (st === 'off' ? parts : active).push(pts ? nsWire(pts, st, broken ? '3 4' : st === 'alt' ? '7 5' : '') : nsElbow(a, b, st, broken ? '3 4' : st === 'alt' ? '7 5' : ''));
      if (st === 'alt' && ELN_LINKS[`${src}>${bus}`]) labels.push(exT(pts ? (pts[1][0] + pts[2][0]) / 2 : (a[0] + b[0]) / 2, pts ? lane + 15 : (a[1] + b[1]) / 2 + 15, ELN_LINKS[`${src}>${bus}`], { size: 10, fill: NS.green }));
    });
  }
  parts.push(...active);
  for (const [k, [label, x, y]] of Object.entries(ELN)) {
    const st = state(k);
    const tag = st === 'fail' ? 'FAILED' : st === 'alt' ? 'FED FROM ' + (ELN[via.get(k)] ? ELN[via.get(k)][0] : via.get(k)) : st === 'warn' ? (t.degraded.has(k) ? 'DEGRADED' : 'LOST') : '';
    parts.push(nsBox(x, y, ELN_W, ELN_H, label, st, { tag }));
  }
  parts.push(...labels);
  if (loads.length) {
    const y0 = 700;
    parts.push(exT(20, y0, 'LOADS LOST', { anchor: 'start', size: 12, fill: NS.orange }));
    loads.slice(0, 6).forEach((l, i) => parts.push(exT(20 + (i % 3) * 330, y0 + 22 + Math.floor(i / 3) * 20, l.length > 40 ? l.slice(0, 39) + '…' : l, { anchor: 'start', size: 11, fill: '#d8d8d8', weight: 500 })));
    if (loads.length > 6) parts.push(exT(W - 20, y0, `+${loads.length - 6} MORE`, { anchor: 'end', size: 11, fill: NS.greyText }));
  }
  if (opts.title) parts.push(exT(W - 16, H - 12, opts.title, { anchor: 'end', size: 12, fill: NS.greyText, weight: 500 }));
  const hotRects = Object.entries(ELN).filter(([k]) => state(k) !== 'live' && state(k) !== 'off').flatMap(([k, [, x, y]]) => {
    const r = [[x, y - 14, ELN_W, ELN_H + 14]];
    const src = via.get(k) || base.via.get(k);
    if (src && ELN[src]) r.push([ELN[src][1], ELN[src][2], ELN_W, ELN_H]);
    return r;
  });
  return nsSvg(W, H, parts, opts.full ? null : nsBounds(hotRects));
}

let elecBreakers = null;
function elecLoadsLost(t) {
  if (!elecBreakers) return [];
  const { live } = elecSolve(t);
  const base = elecSolve({ dead: new Set(), degraded: new Set(), edge: new Map() });
  const lostBus = new Set([...base.live.keys()].filter((k) => !live.get(k)));
  const norm = (b) => String(b || '').toUpperCase().replace(/[_\s-]/g, '').replace('247XP', 'EHA').replace('247PP', 'EHA').replace('GNDFLTSVC', 'GND').replace('ESSSHED', 'ESSSHED');
  const key = (b) => { const n = norm(b); return Object.keys(ELN).find((k) => k === n || k === n.replace('HOT', 'BAT')); };
  return elecBreakers.filter((c) => lostBus.has(key(c.bus))).map((c) => c.name);
}

function elecDiagram(name, title) {
  const t = elecTargets(name);
  if (!t.dead.size && !t.degraded.size && !t.edge.size) return null;
  return elecView(t, { title, loads: elecLoadsLost(t) });
}

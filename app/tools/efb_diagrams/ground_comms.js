const GC_CHOCKS = ['nose', 'lwing', 'rwing', 'lbody', 'rbody'];
const GC_COVERS = ['pitot_cover_1', 'pitot_cover_2', 'pitot_cover_3', 'static_covers', 'aoa_cover_1', 'aoa_cover_2', 'aoa_cover_3',
  'eng_inlet_cover_1', 'eng_inlet_cover_2', 'eng_inlet_cover_3', 'eng_inlet_cover_4', 'eng_exhaust_cover_1', 'eng_exhaust_cover_2', 'eng_exhaust_cover_3', 'eng_exhaust_cover_4',
  'gear_pin_nose', 'gear_pin_lwing', 'gear_pin_rwing', 'gear_pin_lbody', 'gear_pin_rbody'];
const GC_VARS = ['SIM ON GROUND', 'GROUND VELOCITY', 'PUSHBACK ATTACHED', 'A32NX_PARK_BRAKE_LEVER_POS', 'LIGHT BEACON', 'PUSH_OVHD_CALLS_MECH',
  'A32NX_EFB_GROUND_GPU_CONNECTED', 'A32NX_REFUEL_STARTED_BY_USR', 'A32NX_BOARDING_STARTED_BY_USR',
  ...[1, 2, 3, 4].map((n) => 'A32NX_ENGINE_STATE:' + n),
  ...Array.from({ length: 18 }, (_, i) => 'INTERACTIVE POINT OPEN:' + i),
  ...GC_CHOCKS.map((c) => 'A32NX_WALKAROUND_CHOCKS_' + c.toUpperCase()),
  ...GC_COVERS.map((c) => 'A32NX_WALKAROUND_' + c.toUpperCase())];
const GC_ARRIVE_MS = 12000;
const GC_ARRIVAL_MS = 20000;
const GC_DISCONNECT_MS = 6000;
const GC_BYE_MS = 4000;
const GC_SERVICES = { gpu: ['Ground power', 'fbw/efb/ground/gpu'], asu: ['Air start unit', 'fbw/gse/asu/toggle'], pca: ['Preconditioned air', 'fbw/gse/pca/toggle'] };

const gc = { present: false, coming: 0, phase: 'idle', log: [], v: {}, gse: {}, pending: [], flown: false, arrival: 0, mech: 0, beaconSaid: false, movedAt: 0, stillSince: 0, timer: null };

function gcOn(name) { return +gc.v[name] > 0.5; }
function gcGse(name) { return +gc.gse[name + '/state'] || 0; }
function gcAt(svc) { return svc === 'gpu' ? gcOn('A32NX_EFB_GROUND_GPU_CONNECTED') || gcGse('gpu') === 2 : gcGse(svc) === 2; }
function gcCommand(command) { return studyPost('/study/action', { kind: 'command', command }).catch(() => {}); }

function gcLine(who, text) {
  gc.log.push({ who, text, t: new Date() });
  if (gc.log.length > 8) gc.log.shift();
  gcRefresh();
}

const GC_TEXT = {
  go_ahead: 'Cockpit, ground. Go ahead.', on_headset: "Cockpit, ground. I'm on the headset.", release_brakes: 'Roger. Release the parking brake, please.',
  push_start: 'Brakes released. Commencing pushback.', tug_coming: 'Standby, cockpit. The tug is on its way.', tug_wait: 'Standby. Connecting the tug.',
  engines_clear: 'Area is clear. You are cleared to start engines.', push_complete: 'Pushback complete. Set the parking brake, please.',
  disconnect: 'Parking brake set. Disconnecting. Steering pin removed. Hand signals on your left.', bye: 'Disconnected. Have a nice flight.',
  stop: 'Stopping the pushback.', arrival: 'Cockpit, ground. Chocks are in place. You can release the parking brake.', chocks_in: 'Chocks in place.',
  chocks_out: 'Chocks removed.', gpu_on: 'Ground power connected and available.', gpu_off: 'Ground power disconnected.', asu_on: 'Air start unit connected.',
  asu_off: 'Air start unit removed.', pca_on: 'Preconditioned air connected.', pca_off: 'Preconditioned air disconnected.',
  covers_off: 'All covers and pins removed. Walkaround complete.', refuelling: 'Negative, cockpit. Refuelling is still in progress.',
  doors_open: 'Negative, cockpit. A door is still open.', boarding: 'Negative, cockpit. Loading is still in progress.', beacon: 'Beacon is on. Clearing the area.',
  roger: 'Roger.', not_now: 'Unable, cockpit. Not at this time.',
};

function gcSay(key) {
  if (typeof foSay === 'function') foSay(['gnd_' + key], false);
  gcLine('GND', GC_TEXT[key]);
  if (typeof show === 'function') show('Ground: ' + GC_TEXT[key]);
}

function gcParked() { return gcOn('SIM ON GROUND') && Math.abs(+gc.v['GROUND VELOCITY'] || 0) < 1; }

function gcSetChocks(want) {
  GC_CHOCKS.forEach((c) => { if (gcOn('A32NX_WALKAROUND_CHOCKS_' + c.toUpperCase()) !== want) gcCommand(`fbw/walkaround/chocks_${c}_toggle`); });
}

function gcBlocker() {
  if (gcOn('A32NX_REFUEL_STARTED_BY_USR') || [1, 2].includes(gcGse('fuel'))) return 'refuelling';
  if (Array.from({ length: 18 }, (_, i) => +gc.v['INTERACTIVE POINT OPEN:' + i] || 0).some((o) => o > 0.5)) return 'doors_open';
  if (gcOn('A32NX_BOARDING_STARTED_BY_USR') || [1, 2].includes(gcGse('baggage')) || [1, 2].includes(gcGse('catering'))) return 'boarding';
  return null;
}

function gcPushStep() {
  if (gc.phase === 'await_tug') {
    if (!gcOn('PUSHBACK ATTACHED')) return;
    gc.phase = 'await_brake';
    if (gcOn('A32NX_PARK_BRAKE_LEVER_POS')) { gcSay('release_brakes'); return; }
  }
  if (gc.phase === 'await_brake' && !gcOn('A32NX_PARK_BRAKE_LEVER_POS')) {
    gcSay('push_start');
    gc.phase = 'pushing';
    gc.movedAt = 0;
    gc.stillSince = 0;
    if (typeof pbStartAuto === 'function') {
      pb.v = Object.assign({}, pb.v, gc.v);
      pbStartAuto();
      if (!pb.timer) pb.timer = setInterval(pbPoll, 200);
    }
  }
}

function gcRequest(what) {
  const parked = gcParked();
  const capt = {
    call: 'Ground, cockpit.', push: 'Ready for pushback.', start: 'Request engine start.', stop: 'Stop the pushback.', disconnect: 'Parking brake set, disconnect.',
    chocks: 'Chocks, please.', covers: 'Remove covers and pins, please.', gpu: 'Ground power, please.', asu: 'Air start unit, please.', pca: 'Preconditioned air, please.',
  }[what];
  gcLine('CAPT', capt);
  if (what === 'call') {
    if (gc.present) { setTimeout(() => gcSay('go_ahead'), 1500); return; }
    if (!parked && !gcOn('PUSHBACK ATTACHED')) { gcLine('', 'No answer.'); return; }
    if (!gc.coming) gc.coming = Date.now() + GC_ARRIVE_MS;
    gcLine('', 'Calling the ground engineer…');
    return;
  }
  if (!gc.present) { gcLine('', 'Nobody on the headset. Call ground first.'); return; }
  setTimeout(() => gcAnswer(what), 1500);
}

function gcAnswer(what) {
  const pushing = ['pushing', 'await_brake', 'await_tug'].includes(gc.phase);
  if (what === 'push') {
    if (pushing) { gcSay('roger'); return; }
    const blocker = gcBlocker();
    if (blocker) { gcSay(blocker); return; }
    if (GC_CHOCKS.some((c) => gcOn('A32NX_WALKAROUND_CHOCKS_' + c.toUpperCase()))) { gcSetChocks(false); gcSay('chocks_out'); }
    if (!gcOn('PUSHBACK ATTACHED')) {
      if (gcGse('tug') === 0) gcCommand('fbw/pushback/toggle');
      gcSay(gcGse('tug') === 0 ? 'tug_coming' : 'tug_wait');
      gc.phase = 'await_tug';
      return;
    }
    gc.phase = 'await_brake';
    if (gcOn('A32NX_PARK_BRAKE_LEVER_POS')) gcSay('release_brakes');
    else gcPushStep();
    return;
  }
  if (what === 'start') {
    gcSay(gcOn('A32NX_REFUEL_STARTED_BY_USR') || [1, 2].includes(gcGse('fuel')) ? 'refuelling' : 'engines_clear');
    return;
  }
  if (what === 'stop') {
    if (typeof pbStop === 'function') pbStop();
    gcSay('stop');
    if (pushing) gc.phase = 'push_stopped';
    return;
  }
  if (what === 'disconnect') {
    if (!gcOn('PUSHBACK ATTACHED')) { gcSay('not_now'); return; }
    if (!gcOn('A32NX_PARK_BRAKE_LEVER_POS')) { gcSay('push_complete'); gc.phase = 'push_done'; return; }
    gcDisconnect();
    return;
  }
  if (what === 'chocks') {
    const want = !GC_CHOCKS.some((c) => gcOn('A32NX_WALKAROUND_CHOCKS_' + c.toUpperCase()));
    if (want && !gcParked()) { gcSay('not_now'); return; }
    gcSetChocks(want);
    gcSay(want ? 'chocks_in' : 'chocks_out');
    return;
  }
  if (what === 'covers') {
    GC_COVERS.forEach((c) => { if (gcOn('A32NX_WALKAROUND_' + c.toUpperCase())) gcCommand(`fbw/walkaround/${c}_toggle`); });
    gcSay('covers_off');
    return;
  }
  if (GC_SERVICES[what]) {
    if (!gcParked() || gcOn('LIGHT BEACON')) { gcSay('not_now'); return; }
    const want = !gcAt(what);
    gcCommand(GC_SERVICES[what][1]);
    gc.pending = gc.pending.filter((p) => p.svc !== what).concat({ svc: what, want, until: Date.now() + 120000 });
    gcSay('roger');
  }
}

function gcDisconnect() {
  gcSay('disconnect');
  gc.phase = 'releasing';
  setTimeout(() => { if (gcOn('PUSHBACK ATTACHED')) gcCommand('fbw/pushback/toggle'); }, GC_DISCONNECT_MS);
}

function gcTick() {
  const now = Date.now();
  if (gc.v['SIM ON GROUND'] === undefined) return;
  const onGround = gcOn('SIM ON GROUND');
  const gs = Math.abs(+gc.v['GROUND VELOCITY'] || 0);
  gc.airborneMs = onGround ? 0 : (gc.airborneMs || 0) + 1000;
  if (gc.airborneMs >= 15000) gc.flown = true;
  if (!onGround) {
    gc.present = false;
    gc.coming = 0;
    gc.phase = 'idle';
    gc.pending = [];
  }
  const mech = +gc.v.PUSH_OVHD_CALLS_MECH || 0;
  if (mech > 0.5 && gc.mech <= 0.5 && onGround) gcRequest('call');
  gc.mech = mech;
  if (gc.coming && now >= gc.coming) {
    gc.coming = 0;
    if (gcParked() || gcOn('PUSHBACK ATTACHED')) { gc.present = true; gcSay('on_headset'); }
    else gcLine('', 'No answer.');
  }
  if (!gc.present && onGround && gcOn('PUSHBACK ATTACHED')) gc.present = true;
  const enginesOff = [1, 2, 3, 4].every((n) => (+gc.v['A32NX_ENGINE_STATE:' + n] || 0) === 0);
  if (gc.flown && onGround && gs < 1 && enginesOff && gcOn('A32NX_PARK_BRAKE_LEVER_POS')) {
    if (!gc.arrival) gc.arrival = now + GC_ARRIVAL_MS;
    else if (now >= gc.arrival) {
      gc.flown = false;
      gc.arrival = 0;
      gc.present = true;
      gcSetChocks(true);
      gcSay('arrival');
    }
  } else gc.arrival = 0;
  if (gc.present && gs > 3 && !gcOn('PUSHBACK ATTACHED') && !['releasing'].includes(gc.phase)) {
    gc.present = false;
    gc.phase = 'idle';
  }
  gcPushStep();
  if (gc.phase === 'pushing') {
    if (gs > 0.5) { gc.movedAt = now; gc.stillSince = 0; }
    else if (gc.movedAt && !gc.stillSince) gc.stillSince = now;
    const autoDone = typeof pb === 'object' && !pb.auto;
    if (autoDone && gc.movedAt && gc.stillSince && now - gc.stillSince > 3000) {
      gc.phase = 'push_done';
      gcSay('push_complete');
    }
    if (!gcOn('PUSHBACK ATTACHED')) gc.phase = 'idle';
  }
  if ((gc.phase === 'push_done' || gc.phase === 'push_stopped') && gcOn('A32NX_PARK_BRAKE_LEVER_POS') && gcOn('PUSHBACK ATTACHED')) gcDisconnect();
  if (gc.phase === 'releasing' && !gcOn('PUSHBACK ATTACHED')) {
    gc.phase = 'bye';
    setTimeout(() => { gcSay('bye'); gc.present = false; gc.phase = 'idle'; gcRefresh(); }, GC_BYE_MS);
  }
  gc.pending = gc.pending.filter((p) => {
    if (gcAt(p.svc) === p.want) { if (gc.present) gcSay(`${p.svc}_${p.want ? 'on' : 'off'}`); return false; }
    return now < p.until;
  });
  const services = ['gpu', 'asu', 'pca', 'fuel', 'baggage', 'catering', 'water', 'lav', 'clean'].some((s) => [1, 2].includes(gcGse(s)));
  if (gcOn('LIGHT BEACON') && gc.present && services && !gc.beaconSaid) { gc.beaconSaid = true; gcSay('beacon'); }
  if (!gcOn('LIGHT BEACON')) gc.beaconSaid = false;
}

async function gcPoll() {
  if (FIXTURE || study.preview) return;
  try {
    const [vars, gse, efb] = await Promise.all([studyGet('/vars?names=' + GC_VARS.concat(typeof PB_VARS === 'object' ? PB_VARS : []).map(encodeURIComponent).join(',')), studyGet('/gse/state'), studyGet('/efb/state')]);
    gc.v = (vars || {}).vars || {};
    gc.gse = (gse || {}).values || {};
    gcPresses(((efb || {}).values) || {});
  } catch { return; }
  gcTick();
  gcRefresh();
}

function gcPresses(values) {
  const first = !gc.presses;
  gc.presses = gc.presses || {};
  for (const [name, count] of Object.entries(values)) {
    if (!name.startsWith('interphone/')) continue;
    const what = name.slice('interphone/'.length);
    const was = gc.presses[what];
    gc.presses[what] = count;
    if (!first && was !== undefined && count > was) gcRequest(what);
  }
}

function gcStatus() {
  if (gc.present) return { text: 'ON HEADSET', cls: 'on' };
  if (gc.coming) return { text: 'CALLING…', cls: 'wait' };
  return { text: 'NOT CONNECTED', cls: 'off' };
}

function gcCard() {
  const st = gcStatus();
  const btn = (what, label, wide) => {
    const b = el('button', { type: 'button', class: 'pc-btn' + (wide ? ' pc-go' : ''), text: label });
    b.addEventListener('click', () => gcRequest(what));
    if (what !== 'call' && !gc.present) b.disabled = true;
    return b;
  };
  const phaseText = {
    await_tug: 'Waiting for the tug', await_brake: 'Waiting for the parking brake release', pushing: 'Pushing back', push_done: 'Pushback complete, set the parking brake',
    push_stopped: 'Stopped, set the parking brake to disconnect', releasing: 'Disconnecting the tug', bye: 'Tug disconnected',
  }[gc.phase] || '';
  const log = el('div', { class: 'gc-log' }, ...gc.log.map((l) => el('div', { class: 'gc-line' + (l.who === 'GND' ? ' gnd' : l.who === 'CAPT' ? ' capt' : '') },
    el('span', { class: 'gc-who', text: l.who || '·' }), el('span', { text: l.text }))));
  return el('div', { class: 'pc-card', id: 'gc-card' },
    el('div', { class: 'gc-head' }, el('div', { class: 'pc-h', text: 'INTERPHONE · GROUND ENGINEER' }), el('span', { class: 'gc-state ' + st.cls, text: st.text })),
    phaseText ? el('div', { class: 'gc-phase', text: phaseText }) : '',
    el('div', { class: 'gc-btns' },
      btn('call', 'GROUND, COCKPIT', true), btn('push', 'READY FOR PUSHBACK'), btn('start', 'REQUEST ENGINE START'), btn('stop', 'STOP PUSHBACK'),
      btn('disconnect', 'DISCONNECT TUG'), btn('chocks', 'CHOCKS'), btn('covers', 'REMOVE COVERS + PINS'), btn('gpu', 'GROUND POWER'), btn('asu', 'AIR START UNIT'), btn('pca', 'PRECONDITIONED AIR')),
    log,
    el('div', { class: 'pb-note', text: 'The overhead CALLS MECH button calls the ground engineer too, and every request is an X-Plane command (fbw/interphone/...) you can bind to a key.' }));
}

function gcRefresh() {
  const old = document.getElementById('gc-card');
  if (old) old.replaceWith(gcCard());
}

if (!gc.timer) gc.timer = setInterval(gcPoll, 1000);

(function () {
  const s = document.createElement('style');
  s.textContent = `
.gc-head{display:flex;justify-content:space-between;align-items:center;gap:10px;}
.gc-state{font:800 12px 'Manrope','Segoe UI',sans-serif;letter-spacing:0.08em;padding:5px 10px;border-radius:8px;background:#26282c;color:#9da3aa;}
.gc-state.on{background:#1f5f3a;color:#fff;}
.gc-state.wait{background:#7a5a12;color:#fff;}
.gc-phase{font:700 14px 'Manrope','Segoe UI',sans-serif;color:#ffb347;margin:8px 0 2px;}
.gc-btns{display:flex;flex-wrap:wrap;gap:8px;margin:12px 0;}
.gc-log{background:#1b1d20;border:1px solid rgba(255,255,255,0.08);border-radius:10px;padding:8px 12px;min-height:60px;display:flex;flex-direction:column;gap:4px;}
.gc-line{font:500 13.5px 'Manrope','Segoe UI',sans-serif;color:#9da3aa;display:flex;gap:10px;}
.gc-line.gnd{color:#7fd6ff;}
.gc-line.capt{color:#fff;}
.gc-who{font-weight:800;min-width:42px;letter-spacing:0.06em;}
`;
  document.head.append(s);
})();

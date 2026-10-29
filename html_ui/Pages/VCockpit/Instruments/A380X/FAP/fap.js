(() => {
  const { Vars, PanelInstrument, defineInstrument, loadStored, saveStored } = globalThis.xplaneJsData;
  const W = 800;
  const H = 600;
  const PAGES = [
    ['status', 'Cabin<br>Status', 'CABIN STATUS'],
    ['music', 'Music', 'MUSIC'],
    ['preann', 'Pre.<br>Ann.', 'PRE ANNOUNCEMENT'],
    ['lights', 'Lights', 'CABIN LIGHTING'],
    ['doors', 'Doors/<br>Slides', 'DOORS / SLIDES'],
    ['temp', 'Temp.', 'CABIN TEMPERATURE'],
    ['water', 'Water/<br>Waste', 'WATER / WASTE'],
    ['smoke', 'Smoke<br>Detect', 'SMOKE DETECTION'],
    ['seats', 'Seat<br>Setting', 'SEAT SETTING'],
    ['ready', 'Cabin<br>Ready', 'CABIN READY'],
    ['sysinfo', 'System<br>Info', 'SYSTEM INFO'],
  ];
  const ZONES = ['FWD', 'MID', 'AFT'];
  const DOORS = [
    ['M1L', -3.05, -28.85, 'M'], ['M1R', 3.05, -28.85, 'M'], ['M2L', -3.58, -18.65, 'M'], ['M2R', 3.58, -18.65, 'M'],
    ['M3L', -3.71, -2.44, 'M'], ['M3R', 3.71, -2.44, 'M'], ['M4L', -3.57, 9.66, 'M'], ['M4R', 3.57, 9.66, 'M'],
    ['M5L', -3.35, 18.6, 'M'], ['M5R', 3.35, 18.6, 'M'], ['U1L', -3.38, -14.2, 'U'], ['U1R', 3.38, -14.2, 'U'],
    ['U2L', -3.36, 5.21, 'U'], ['U2R', 3.36, 5.21, 'U'], ['U3L', -3.37, 14.1, 'U'], ['U3R', 3.37, 14.1, 'U'],
    ['FWD CARGO', 3.4, -23.45, 'C'], ['AFT CARGO', 3.37, 13.03, 'C'],
  ];
  const ANNOUNCEMENTS = [[2, 'Boarding'], [3, 'Taxi /<br>Safety'], [4, 'Takeoff'], [5, 'After<br>Takeoff'], [6, 'Cruise'], [7, 'Descent'], [8, 'Approach'], [9, 'After<br>Landing'], [10, 'Gate<br>Arrival'], [11, 'Deboard-<br>ing']];
  const SCENES = [
    [12, 'Auto', null], [1, 'Boarding', '#ffb86b'], [2, 'Taxi', '#5280ff'], [3, 'Takeoff', '#5280ff'], [4, 'Climb', '#ffb86b'],
    [5, 'Cruise', '#fff2e0'], [6, 'Meal', '#ffb86b'], [7, 'Night', '#1f2973'], [8, 'Sunrise', '#ff8026'], [9, 'Descent', '#ffb86b'],
    [10, 'Landing', '#5280ff'], [11, 'Arrival', '#fff2e0'],
  ];
  const AREAS = [['MD FWD', 'MAIN DECK FWD'], ['MD MID', 'MAIN DECK MID'], ['MD AFT', 'MAIN DECK AFT'], ['UD FWD', 'UPPER DECK FWD'], ['UD MID', 'UPPER DECK MID'], ['UD AFT', 'UPPER DECK AFT']];
  const COLOURS = { armed: '#1f3dff', disarmed: '#25cf3a', moving: '#ff9a1a', open: '#8c0d0d' };
  const L = (n) => `L:${n}`;

  const num = (name, unit = 'number') => {
    try {
      const v = Vars.read(name, unit);
      return typeof v === 'number' && isFinite(v) ? v : null;
    } catch (e) {
      return null;
    }
  };
  const on = (name) => (num(name) || 0) >= 0.5;
  const set = (name, value) => {
    try {
      Vars.write(name, 'number', value);
    } catch (e) {
      /* not available outside the sim */
    }
  };
  const fmt = (x, d = 0, u = '') => (x === null ? '--' : `${x.toFixed(d)}${u}`);
  const box = (x, y, w, h, cls, inner, extra = '') => `<div class="${cls}" style="left:${x}px;top:${y}px;width:${w}px;height:${h}px;${extra}">${inner}</div>`;
  const key = (x, y, w, h, act, label, cls = '') => `<div class="key ${cls}" data-a="${act}" style="position:absolute;left:${x}px;top:${y}px;width:${w}px;height:${h}px">${label}</div>`;

  const noise = (() => {
    let url = null;
    return () => {
      if (url) return url;
      try {
        const c = document.createElement('canvas');
        c.width = c.height = 256;
        const g = c.getContext('2d');
        const img = g.createImageData(256, 256);
        let seed = 380;
        const rnd = () => ((seed = (seed * 1103515245 + 12345) & 0x7fffffff) / 0x7fffffff);
        const coarse = [];
        for (let i = 0; i < 64 * 64; i++) coarse.push(rnd());
        for (let y = 0; y < 256; y++) {
          for (let x = 0; x < 256; x++) {
            const cx = x / 4, cy = y / 4, x0 = Math.floor(cx) & 63, y0 = Math.floor(cy) & 63, x1 = (x0 + 1) & 63, y1 = (y0 + 1) & 63;
            const fx = cx - Math.floor(cx), fy = cy - Math.floor(cy);
            const blob = coarse[y0 * 64 + x0] * (1 - fx) * (1 - fy) + coarse[y0 * 64 + x1] * fx * (1 - fy) + coarse[y1 * 64 + x0] * (1 - fx) * fy + coarse[y1 * 64 + x1] * fx * fy;
            const v = 98 + (blob - 0.5) * 26 + (rnd() - 0.5) * 18;
            const i = (y * 256 + x) * 4;
            img.data[i] = v;
            img.data[i + 1] = v + 2;
            img.data[i + 2] = v + 5;
            img.data[i + 3] = 255;
          }
        }
        g.putImageData(img, 0, 0);
        url = c.toDataURL();
      } catch (e) {
        url = '';
      }
      return url;
    };
  })();

  const plan = (w, h, inner, opts = {}) => {
    const sil = (window.FAP_SIL && window.FAP_SIL.top) || [];
    const span = 80, length = 73;
    const s = Math.min((w - 20) / span, (h - 16) / length);
    const cx = w / 2, top = (h - length * s) / 2 + 35.2 * s;
    const P = (x, z) => [cx + x * s, top + z * s];
    const d = sil.map((p, i) => `${i ? 'L' : 'M'}${P(p[0], p[1]).map((v) => v.toFixed(1)).join(',')}`).join('') + 'Z';
    const extra = inner(P, s);
    return `<svg width="${w}" height="${h}" viewBox="0 0 ${w} ${h}" style="position:absolute;left:0;top:0">
      <path d="${d}" fill="${opts.fill || '#45474b'}" stroke="${opts.stroke || '#d6d9dc'}" stroke-width="1.1" stroke-linejoin="round"/>${extra}</svg>`;
  };

  class FAP extends PanelInstrument {
    get templateId() {
      return 'A380X_FAP';
    }

    get interactive() {
      return true;
    }

    onAttach() {
      super.onAttach();
      this.root = this.querySelector('#FAP_ROOT');
      this.root.innerHTML = '<div id="SCR"></div><div id="HK"></div>';
      this.scr = this.root.querySelector('#SCR');
      this.hk = this.root.querySelector('#HK');
      this.page = 'status';
      this.lockUntil = 0;
      this.bootUntil = 0;
      this.lavMaint = false;
      this.evacArmedUntil = 0;
      this.areas = {};
      this.off = false;
      this.paCmd = 0;
      this.doorSeq = 0;
      this.msg = null;
      this.lastDraw = 0;
      this.showMain = true;
      this.showUpper = true;
      this.root.addEventListener('click', (e) => this.click(e));
    }

    powered() {
      return [1, 2, 3, 4].some((n) => on(L(`A32NX_ELEC_AC_${n}_BUS_IS_POWERED`)));
    }

    say(text) {
      this.msg = { text, until: Date.now() + 4000 };
      this.draw();
    }

    click(e) {
      if (!this.powered()) return;
      const hk = e.target.closest('[data-k]');
      if (hk) {
        this.hardkey(hk.getAttribute('data-k'));
        this.draw();
        return;
      }
      if (Date.now() < this.lockUntil || Date.now() < this.bootUntil) return;
      if (this.off) {
        this.off = false;
        this.draw();
        return;
      }
      const t = e.target.closest('[data-a]');
      if (!t) return;
      const [a, arg] = t.getAttribute('data-a').split(':');
      const i = PAGES.findIndex((p) => p[0] === this.page);
      if (a === 'page') this.page = arg;
      else if (a === 'prev') this.page = PAGES[(i + PAGES.length - 1) % PAGES.length][0];
      else if (a === 'next') this.page = PAGES[(i + 1) % PAGES.length][0];
      else if (a === 'screenoff') this.off = true;
      else if (a === 'pa') {
        this.paCmd = +arg;
        set(L('A380X_FAP_PA_CMD'), this.paCmd);
        set(L('A380X_FAP_PA_SEQ'), (num(L('A380X_FAP_PA_SEQ')) || 0) + 1);
      } else if (a === 'door') {
        this.doorSeq = (num(L('A380X_FAP_DOOR_SEQ')) || 0) + 1;
        set(L('A380X_FAP_DOOR'), +arg + 1);
        set(L('A380X_FAP_DOOR_SEQ'), this.doorSeq);
        this.doorAsked = Date.now();
      } else if (a === 'deck') {
        if (arg === 'M') this.showMain = !this.showMain;
        else this.showUpper = !this.showUpper;
      } else if (a === 'scene') set(L('A380X_FAP_CABIN_SCENE'), +arg);
      else if (a === 'level') set(L('A380X_FAP_CABIN_LEVEL'), +arg + 1);
      else if (a === 'slider') {
        const r = t.getBoundingClientRect();
        const pct = Math.round(Math.max(0, Math.min(1, (r.bottom - e.clientY) / r.height)) * 20) * 5;
        set(L('A380X_FAP_CABIN_LEVEL'), pct + 1);
      } else if (a === 'lightsreset') {
        set(L('A380X_FAP_CABIN_SCENE'), 0);
        set(L('A380X_FAP_CABIN_LEVEL'), 0);
      } else if (a === 'temp') {
        const cur = num(L('A32NX_COND_PURS_SEL_TEMPERATURE')) || 24;
        set(L('A32NX_COND_PURS_SEL_TEMPERATURE'), Math.max(18, Math.min(30, Math.round((cur + +arg) * 2) / 2)));
      } else if (a === 'heaters') set(L('A380X_FAP_WATER_HEATERS_OFF'), on(L('A380X_FAP_WATER_HEATERS_OFF')) ? 0 : 1);
      else if (a === 'seatpower') set(L('A380X_FAP_SEAT_POWER_OFF'), on(L('A380X_FAP_SEAT_POWER_OFF')) ? 0 : 1);
      else if (a === 'galley') set(L(`A380X_FAP_GALLEY_OFF_${arg}`), on(L(`A380X_FAP_GALLEY_OFF_${arg}`)) ? 0 : 1);
      else if (a === 'callreset') set(L('A380X_FAP_CALL_RESET_SEQ'), (num(L('A380X_FAP_CALL_RESET_SEQ')) || 0) + 1);
      else if (a === 'area') this.areas[arg] = !this.areas[arg];
      else if (a === 'cabinready') {
        if (AREAS.every(([id]) => this.areas[id])) set(L('A380X_FAP_CABIN_READY_SEQ'), (num(L('A380X_FAP_CABIN_READY_SEQ')) || 0) + 1);
        else this.say('NOT ALL AREAS READY');
      }
      this.draw();
    }

    hardkey(k) {
      const now = Date.now();
      if (k === 'ped') set(L('A380X_FAP_SEAT_POWER_OFF'), on(L('A380X_FAP_SEAT_POWER_OFF')) ? 0 : 1);
      else if (k === 'lights') {
        const lvl = Math.round(num(L('A380X_FAP_CABIN_LEVEL')) || 0);
        set(L('A380X_FAP_CABIN_LEVEL'), lvl === 1 ? 101 : 1);
      } else if (k === 'lav') this.lavMaint = !this.lavMaint;
      else if (k === 'lock') this.lockUntil = now + 30000;
      else if (k === 'evac') {
        if (now < this.evacArmedUntil) {
          set(L('A32NX_EVAC_COMMAND_TOGGLE'), 1);
          this.evacArmedUntil = 0;
        } else {
          this.evacArmedUntil = now + 3000;
          this.say('PRESS EVAC CMD AGAIN TO CONFIRM');
        }
      } else if (k === 'evacreset') set(L('A32NX_EVAC_COMMAND_TOGGLE'), 0);
      else if (k === 'smoke') set(L('A380X_FAP_CALL_RESET_SEQ'), (num(L('A380X_FAP_CALL_RESET_SEQ')) || 0) + 1);
      else if (k === 'pcreset') {
        this.bootUntil = now + 9000;
        this.page = 'status';
        this.off = false;
      } else if (k === 'emer') set(L('A380X_FAP_EMER_LIGHT'), on(L('A380X_FAP_EMER_LIGHT')) ? 0 : 1);
      else if (k === 'paxsys') set(L('A380X_FAP_PAX_SYS_OFF'), on(L('A380X_FAP_PAX_SYS_OFF')) ? 0 : 1);
    }

    drawKeys() {
      const now = Date.now();
      const evac = on(L('A32NX_EVAC_COMMAND_TOGGLE'));
      const blink = Math.floor(now / 500) % 2 === 0;
      const keys = [
        ['ped', 'PED<br>POWER', !on(L('A380X_FAP_SEAT_POWER_OFF')) && !on(L('A380X_FAP_PAX_SYS_OFF'))],
        ['lights', 'LIGHTS MAIN<br>ON/OFF', Math.round(num(L('A380X_FAP_CABIN_LEVEL')) || 0) !== 1],
        ['lav', 'LAV<br>MAINT', this.lavMaint],
        ['lock', 'SCREEN<br>30 sec LOCK', now < this.lockUntil],
        ['evac', 'EVAC<br>CMD', evac && blink ? 'red' : false, 'evac' + (now < this.evacArmedUntil ? ' guard' : '')],
        ['evacreset', 'EVAC<br>RESET', false],
        ['smoke', 'SMOKE<br>RESET', false],
        ['pcreset', 'FAP - PC<br>RESET', now < this.bootUntil],
      ];
      let h = keys.map(([k, label, state, cls], i) => `<div class="hk ${cls || ''}${state === 'red' ? ' alert' : state ? ' lit' : ''}" data-k="${k}" style="left:${18 + i * 92}px">${label}</div>`).join('');
      const sw = (k, x, label, upText, downText, up) => `<div class="tsw" data-k="${k}" style="left:${x}px"><div>${label}</div><div class="base"><div class="bat ${up ? 'up' : 'down'}"></div></div><div style="font-weight:400">${up ? upText : downText}</div></div>`;
      h += sw('emer', 768, 'EMER LIGHT', 'ON', 'ARMED', on(L('A380X_FAP_EMER_LIGHT')));
      h += sw('paxsys', 860, 'PAX SYS', 'ON', 'OFF', !on(L('A380X_FAP_PAX_SYS_OFF')));
      this.hk.innerHTML = h;
    }

    onFrame() {
      super.onFrame();
      if (this.doorAsked && Date.now() - this.doorAsked > 400) {
        this.doorAsked = 0;
        if ((num(L('A380X_FAP_DOOR_REFUSED')) || 0) === this.doorSeq) this.say('DOOR OPERATION INHIBITED: AIRCRAFT MOVING OR ENGINE RUNNING');
      }
      if (Date.now() - this.lastDraw >= 500) this.draw();
    }

    cautions() {
      const out = [];
      if (on(L('A32NX_CARGO_FWD_SMOKE_DETECTED')) || on(L('A32NX_CARGO_AFT_SMOKE_DETECTED'))) out.push(['red', 'CARGO SMOKE']);
      for (let i = 1; i <= 3; i++) {
        if (on(L(`A32NX_CABIN_IFE_ZONE_SMOKE:${i}`)) || on(L(`A32NX_CABIN_GALLEY_OVEN_SMOKE:${i}`))) out.push(['red', `SMOKE ${ZONES[i - 1]}`]);
        if (on(L(`A32NX_CABIN_WASTE_TANK_FULL:${i}`))) out.push(['amber', `WASTE ${ZONES[i - 1]} FULL`]);
      }
      if (on(L('A32NX_CABIN_SLIDE_PRESSURE_LOW:1'))) out.push(['amber', 'SLIDE PRESS LO']);
      const calls = num(L('A32NX_CABIN_CREW_CALL_NEW_COUNT')) || 0;
      if (calls > 0) out.push(['cyan', `CALL ${calls}`]);
      return out.slice(0, 3);
    }

    draw() {
      this.lastDraw = Date.now();
      const r = this.scr;
      if (!this.powered()) {
        r.className = 'dark';
        r.style.backgroundImage = 'none';
        r.innerHTML = '';
        this.hk.innerHTML = '';
        return;
      }
      this.drawKeys();
      if (Date.now() < this.bootUntil) {
        r.className = 'dark';
        r.style.backgroundImage = 'none';
        r.innerHTML = Date.now() > this.bootUntil - 5000 ? '<div style="position:absolute;left:0;right:0;top:280px;text-align:center;color:#c9ccd0;font:700 14px B612">FAP INITIALIZING...</div>' : '';
        return;
      }
      if (this.off) {
        r.className = 'dark';
        r.style.backgroundImage = 'none';
        r.innerHTML = '';
        return;
      }
      r.className = '';
      const bg = noise();
      if (bg) r.style.backgroundImage = `url(${bg})`;
      const title = (PAGES.find((p) => p[0] === this.page) || PAGES[0])[2];
      let html = `<div class="hdr">${title}</div>`;
      html += key(8, 6, 46, 28, 'screenoff', 'Screen<br>Off', 'screenoff');
      html += `<div class="caut">${this.cautions().map(([c, t]) => `<span class="${c}">${t}</span>`).join(' &nbsp;')}</div>`;
      html += this[this.page]();
      html += `<div class="nav">${key(0, 0, 40, 34, 'prev', '<svg width="16" height="18" viewBox="0 0 16 18"><polygon points="15,1 15,17 1,9" fill="#1d2125"/></svg>', 'arrow').replace('position:absolute;left:0px;top:0px;', '')}${PAGES.map(([id, label]) => key(0, 0, 52, 34, `page:${id}`, label, id === this.page ? 'on' : '').replace('position:absolute;left:0px;top:0px;', '')).join('')}${key(0, 0, 40, 34, 'next', '<svg width="16" height="18" viewBox="0 0 16 18"><polygon points="1,1 1,17 15,9" fill="#1d2125"/></svg>', 'arrow').replace('position:absolute;left:0px;top:0px;', '')}</div>`;
      if (this.msg && Date.now() < this.msg.until) html += `<div class="msg">${this.msg.text}</div>`;
      if (Date.now() < this.lockUntil) html += `<div class="msg">SCREEN LOCKED FOR CLEANING ${Math.ceil((this.lockUntil - Date.now()) / 1000)} s</div>`;
      r.innerHTML = html;
    }

    status() {
      const doorStates = DOORS.map((d, i) => this.doorState(i));
      const open = doorStates.filter((st) => st === 'open' || st === 'moving').length;
      const armed = doorStates.slice(0, 16).filter((st) => st === 'armed').length;
      const temps = [];
      for (let i = 1; i <= 8; i++) temps.push(num(L(`A32NX_COND_MAIN_DECK_${i}_TEMP`)));
      for (let i = 1; i <= 7; i++) temps.push(num(L(`A32NX_COND_UPPER_DECK_${i}_TEMP`)));
      const known = temps.filter((t) => t !== null);
      const avg = known.length ? known.reduce((a, b) => a + b, 0) / known.length : null;
      const waste = Math.max(...[1, 2, 3].map((i) => num(L(`A32NX_CABIN_WASTE_LEVEL_PERCENT:${i}`)) || 0));
      const calls = num(L('A32NX_CABIN_CREW_CALL_NEW_COUNT')) || 0;
      const row = (k, v, c = '') => `${k}<div class="s" style="${c ? 'color:' + c : ''}">${v}</div>`;
      const low = on(L('A32NX_CABIN_SLIDE_PRESSURE_LOW:1'));
      const ready = on(L('A32NX_CABIN_READY'));
      const evac = on(L('A32NX_EVAC_COMMAND_TOGGLE'));
      let h = box(20, 44, 370, 230, 'panel', '<div class="ttl">DOORS / SLIDES</div>');
      h += box(30, 68, 350, 196, 'inset', `<div class="status">${row('DOORS OPEN', String(open), open ? '#ffa21f' : '#7cf04a')}${row('SLIDES ARMED', `${armed} OF 16`)}${row('SLIDE BOTTLES', low ? 'PRESSURE LOW' : 'NORMAL', low ? '#ffa21f' : '')}</div>`);
      h += box(410, 44, 370, 230, 'panel', '<div class="ttl">TEMPERATURE / WATER / WASTE</div>');
      h += box(420, 68, 350, 196, 'inset', `<div class="status">${row('CABIN AVERAGE', fmt(avg, 1, ' °C'))}${row('SELECTED', fmt(num(L('A32NX_COND_PURS_SEL_TEMPERATURE')), 1, ' °C'))}${row('POTABLE WATER', fmt(num(L('A32NX_CABIN_WATER_QTY_PERCENT')), 0, ' %'))}${row('WASTE (HIGHEST TANK)', `${Math.round(waste)} %`, waste > 90 ? '#ffa21f' : '')}</div>`);
      h += box(20, 290, 370, 228, 'panel', '<div class="ttl">CABIN</div>');
      h += box(30, 314, 350, 194, 'inset', `<div class="status">${row('CABIN READY', ready ? 'SENT TO COCKPIT' : 'NOT SENT', ready ? '#7cf04a' : '')}${row('CREW CALLS', String(calls), calls ? '#6fe3ff' : '')}${row('IN-SEAT POWER', on(L('A32NX_CABIN_SEAT_POWER_CMD')) ? 'ON' : 'OFF')}${row('EVACUATION', evac ? 'ALERT' : 'NORMAL', evac ? '#ff5a4a' : '')}</div>`);
      h += box(410, 290, 370, 228, 'inset', plan(370, 228, (P) => DOORS.map((d, i) => {
        const [x, y] = P(d[1], d[2]);
        return `<circle cx="${x}" cy="${y}" r="3" fill="${COLOURS[doorStates[i]] || '#777'}"/>`;
      }).join('')));
      return h;
    }

    ready() {
      const all = AREAS.every(([id]) => this.areas[id]);
      const sent = on(L('A32NX_CABIN_READY'));
      let h = box(20, 44, 480, 474, 'panel', '<div class="ttl">AREA STATUS</div>');
      AREAS.forEach(([id, label], k) => {
        const ok = !!this.areas[id];
        h += `<div style="position:absolute;left:36px;top:${78 + k * 62}px;width:300px;height:46px;display:flex;align-items:center;font:700 13px B612;color:#2a2e33">${label}</div>`;
        h += key(340, 74 + k * 62, 140, 50, `area:${id}`, ok ? 'AREA READY' : 'NOT READY', ok ? 'on' : '');
      });
      h += box(520, 44, 260, 474, 'panel', '<div class="ttl">CABIN READY</div><div class="note" style="padding:6px 14px">When every area reports ready, the purser sends CABIN READY to the cockpit (ECAM).</div>');
      h += key(560, 160, 180, 70, 'cabinready', 'CABIN<br>READY', sent ? 'on' : all ? '' : 'off');
      h += box(540, 260, 220, 80, 'inset', `<div class="status">${sent ? 'CABIN READY SENT' : all ? 'ALL AREAS READY' : 'WAITING FOR AREAS'}</div>`);
      return h;
    }

    music() {
      const playing = this.paCmd === 1;
      let h = box(20, 48, 330, 250, 'panel', `<div class="ttl">BOARDING MUSIC</div>
        <div class="note" style="padding:4px 14px">Cabin boarding music on the passenger address system, on every deck.</div>`);
      h += key(48, 140, 120, 56, 'pa:1', 'PLAY', playing ? 'on' : '');
      h += key(200, 140, 120, 56, 'pa:0', 'STOP', 'red');
      h += box(20, 318, 330, 150, 'inset', `<div class="otitle" style="margin-top:6px">MUSIC STATUS</div><div class="status">CHANNEL<div class="s">BOARDING</div>STATE<div class="s">${playing ? 'PLAYING' : 'STOPPED'}</div></div>`);
      h += box(380, 48, 400, 470, 'inset', plan(400, 470, (P) => DOORS.filter((d) => d[3] !== 'C').map((d) => {
        const [x, y] = P(d[1] * 0.75, d[2]);
        return `<circle cx="${x}" cy="${y}" r="4" fill="${playing ? '#7cf04a' : '#6a6d71'}"/>`;
      }).join('')));
      return h;
    }

    preann() {
      const auto = on(L('A32NX_SOUND_ANNOUNCEMENTS_ENABLED'));
      let h = box(20, 48, 470, 330, 'panel', '<div class="ttl">SELECT ANNOUNCEMENT</div>');
      ANNOUNCEMENTS.forEach(([id, label], k) => {
        h += key(40 + (k % 5) * 88, 82 + Math.floor(k / 5) * 72, 78, 56, `pa:${id}`, label, this.paCmd === id ? 'on' : '');
      });
      h += key(40, 250, 166, 46, 'pa:0', 'STOP ANNOUNCEMENT', 'red');
      h += box(20, 398, 470, 120, 'inset', `<div class="otitle" style="margin-top:6px">ANNOUNCEMENT STATUS</div><div class="status">SELECTED<div class="s">${(ANNOUNCEMENTS.find((a) => a[0] === this.paCmd) || [0, 'NONE'])[1].replace('<br>', ' ').toUpperCase()}</div>AUTOMATIC BY FLIGHT PHASE<div class="s">${auto ? 'ON' : 'OFF'}</div></div>`);
      h += box(510, 48, 270, 470, 'inset', plan(270, 470, (P) => {
        const [x0, y0] = P(-2, -30);
        const [, y1] = P(0, 30);
        return `<rect x="${x0}" y="${y0}" width="${P(2, 0)[0] - x0}" height="${y1 - y0}" fill="${this.paCmd > 1 ? 'rgba(124,240,74,0.35)' : 'none'}"/>`;
      }));
      return h;
    }

    lights() {
      const scene = Math.round(num(L('A380X_FAP_CABIN_SCENE')) || 0);
      const levelRaw = Math.round(num(L('A380X_FAP_CABIN_LEVEL')) || 0);
      const pct = levelRaw ? levelRaw - 1 : 100;
      const cur = SCENES.find((s) => s[0] === scene);
      let h = box(150, 40, 250, 220, 'inset', '<div class="otitle" style="margin-top:5px">SELECT LIGHT SCENE</div>');
      SCENES.forEach(([id, label, colour], k) => {
        const swatch = colour ? `<div style="position:absolute;left:4px;right:4px;top:4px;height:9px;border-radius:2px;background:${colour}"></div>` : '';
        h += key(162 + (k % 4) * 58, 64 + Math.floor(k / 4) * 62, 52, 54, `scene:${id}`, `${swatch}<span style="margin-top:12px">${label}</span>`, scene === id ? 'on' : '');
      });
      h += box(420, 40, 170, 220, 'inset', '<div class="otitle" style="margin-top:5px">BRIGHTNESS</div>');
      h += key(432, 66, 54, 40, 'level:100', 'BRT<br>100%', pct === 100 && levelRaw ? 'on' : '');
      h += key(432, 116, 54, 40, 'level:25', 'DIM<br>25%', pct === 25 ? 'on' : '');
      h += key(432, 166, 54, 40, 'level:0', 'OFF<br>0%', pct === 0 && levelRaw ? 'on' : '');
      h += `<svg style="position:absolute;left:500px;top:64px" width="30" height="150"><polygon points="2,0 28,0 15,150" fill="url(#wedge)"/><defs><linearGradient id="wedge" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#ffb347"/><stop offset="1" stop-color="#ff7a00" stop-opacity="0.2"/></linearGradient></defs></svg>`;
      h += `<div data-a="slider" style="position:absolute;left:540px;top:64px;width:26px;height:150px;border-radius:4px;background:linear-gradient(90deg,#1e2022,#4b4e52,#1e2022);border:1px solid #111;cursor:pointer">
        <div style="position:absolute;left:-4px;right:-4px;top:${(1 - pct / 100) * 140}px;height:10px;border-radius:3px;background:linear-gradient(#b8f58a,#46a81b);border:1px solid #22420f;pointer-events:none"></div></div>`;
      h += box(500, 222, 70, 26, 'inset', `<div style="text-align:center;font:700 14px 'B612';line-height:22px">${pct} %</div>`);
      h += box(150, 280, 440, 150, 'panel', `<div class="ttl" style="text-align:left;padding-left:10px">LIGHT STATUS</div>`);
      h += box(160, 302, 420, 116, 'inset', `<div class="status">SELECTED SCENE<div class="s">${cur ? cur[1].toUpperCase() : 'SET BY EFB'}</div>BRIGHTNESS LEVEL<div class="s">${levelRaw ? pct + '%' : 'SET BY EFB'}</div>${cur && cur[0] === 12 ? '<div class="w">SCENE FOLLOWS THE FLIGHT PHASE</div>' : ''}</div>`);
      h += key(150, 446, 120, 34, 'lightsreset', 'RESET TO EFB');
      const glow = cur && cur[2] ? cur[2] : '#fff2e0';
      h += box(620, 40, 160, 470, 'inset', plan(160, 470, (P) => {
        const [x0, y0] = P(-2.6, -31);
        const [x1, y1] = P(2.6, 30);
        return `<rect x="${x0}" y="${y0}" width="${x1 - x0}" height="${y1 - y0}" rx="6" fill="${glow}" opacity="${0.15 + 0.6 * pct / 100}"/>`;
      }));
      return h;
    }

    doorState(i) {
      const p = num(`A:INTERACTIVE POINT OPEN:${i}`, 'percent');
      if (p === null) return 'na';
      if (p > 98) return 'open';
      if (p > 2) return 'moving';
      if (i >= 16) return 'disarmed';
      return (num(L(`ANIM_DOOR_${DOORS[i][0]}_DISARMED`)) ?? 1) < 0.5 ? 'armed' : 'disarmed';
    }

    doors() {
      const sw = (c) => `<div class="sw" style="background:${c}"></div>`;
      const legend = `<div class="ttl">LEGEND</div><div class="legend">
        <div class="row">${sw(COLOURS.armed)}<div>Door locked<br>Slide armed</div></div>
        <div class="row">${sw(COLOURS.disarmed)}<div>Door locked<br>Slide disarmed</div></div>
        <div class="row">${sw(COLOURS.moving)}<div>Door moving</div></div>
        <div class="row">${sw(COLOURS.open)}<div>Door open</div></div>
        <div style="border-top:1px solid #8d9196;margin:10px 8px"></div>
        <div class="row" data-a="deck:M" style="cursor:pointer"><div class="sw" style="width:10px;height:10px;background:${this.showMain ? '#2a2e33' : '#fff'}"></div>Middle deck M1 - M5</div>
        <div class="row" data-a="deck:U" style="cursor:pointer"><div class="sw" style="width:10px;height:10px;background:${this.showUpper ? '#2a2e33' : '#fff'}"></div>Upper deck U1 - U3</div></div>
        <div class="note" style="position:absolute;left:10px;right:10px;bottom:10px">Tap a door to open or close it. Only available with the aircraft stationary and all engines shut down.</div>`;
      let h = box(20, 44, 160, 474, 'panel', legend);
      h += box(196, 44, 584, 474, 'panel', '<div class="ttl" style="text-align:left;padding-left:8px">MIDDLE &amp; UPPER DECK DOORS</div>');
      const iw = 570, ih = 442;
      const marks = (P) => DOORS.map((d, i) => {
        if ((d[3] === 'M' || d[3] === 'C') && !this.showMain) return '';
        if (d[3] === 'U' && !this.showUpper) return '';
        const c = COLOURS[this.doorState(i)] || '#777';
        const left = d[1] < 0;
        const inboard = d[3] === 'U' ? 0.55 : 0;
        const [x, y] = P(d[1] + (left ? inboard : -inboard), d[2]);
        const shape = d[3] === 'U'
          ? `<text x="${x}" y="${y + 6}" text-anchor="middle" font-size="17" font-weight="700" fill="${c}">&#10033;</text>`
          : `<rect x="${x - 4}" y="${y - 7}" width="8" height="14" fill="${c}" stroke="#111" stroke-width="0.8"/>`;
        const cargo = d[3] === 'C';
        const lx = left ? x - 10 : x + (cargo ? 62 : 10);
        const leader = cargo ? `<line x1="${x + 5}" y1="${y}" x2="${lx - 3}" y2="${y}" stroke="#9a9ea3" stroke-width="0.8"/>` : '';
        return `<g data-a="door:${i}" style="cursor:pointer"><rect x="${x - 14}" y="${y - 12}" width="28" height="24" fill="transparent"/>${leader}${shape}
          <text x="${lx}" y="${y + 4}" text-anchor="${left ? 'end' : 'start'}" font-size="10" fill="#e9eaec">${d[0]}</text></g>`;
      }).join('');
      h += box(203, 66, iw, ih, 'inset', plan(iw, ih, marks));
      return h;
    }

    temp() {
      const sel = num(L('A32NX_COND_PURS_SEL_TEMPERATURE'));
      const chip = (x, y, label, t) => {
        const c = t === null ? '#5b5e62' : t < 20 ? '#3c6fb5' : t > 26 ? '#b54a3c' : '#3f8f55';
        return `<rect x="${x - 23}" y="${y - 13}" width="46" height="26" rx="3" fill="${c}" stroke="#111"/><text x="${x}" y="${y - 2}" text-anchor="middle" font-size="8" fill="#e9eaec">${label}</text><text x="${x}" y="${y + 10}" text-anchor="middle" font-size="11" font-weight="700" fill="#fff">${fmt(t, 1)}</text>`;
      };
      let h = box(20, 44, 520, 474, 'panel', '<div class="ttl" style="text-align:left;padding-left:8px">ZONE TEMPERATURES (°C)</div>');
      h += box(27, 66, 506, 442, 'inset', plan(506, 442, (P) => {
        let s = chip(...P(0, -31.5), 'CKPT', num(L('A32NX_COND_CKPT_TEMP')));
        for (let i = 1; i <= 8; i++) s += chip(...P(-9.5, -27 + (i - 1) * 7.4), `MAIN ${i}`, num(L(`A32NX_COND_MAIN_DECK_${i}_TEMP`)));
        for (let i = 1; i <= 7; i++) s += chip(...P(9.5, -17 + (i - 1) * 6.4), `UPPER ${i}`, num(L(`A32NX_COND_UPPER_DECK_${i}_TEMP`)));
        s += chip(...P(-17, -21), 'FWD CARGO', num(L('A32NX_COND_CARGO_FWD_TEMP')));
        s += chip(...P(17, 17), 'BULK', num(L('A32NX_COND_CARGO_BULK_TEMP')));
        return s;
      }));
      h += box(556, 44, 224, 250, 'panel', '<div class="ttl">CABIN TEMPERATURE SELECTION</div>');
      h += key(574, 90, 56, 48, 'temp:0.5', '&#9650;');
      h += key(574, 150, 56, 48, 'temp:-0.5', '&#9660;');
      h += box(642, 112, 120, 50, 'inset', `<div style="text-align:center;font:700 22px 'B612';line-height:46px">${fmt(sel, 1, ' °C')}</div>`);
      h += box(566, 210, 204, 76, '', `<div class="note">The zones follow this selection when the cockpit CABIN selector is at PURS SEL. Range 18 to 30 °C.</div>`, 'position:absolute');
      if (on(L('A32NX_DEEP_CABIN_PURSER_TEMP_SEL_FAULT'))) h += box(556, 310, 224, 60, 'inset', '<div class="status w">PURSER TEMP SEL FAULT<br>COCKPIT CONTROL ONLY</div>');
      return h;
    }

    water() {
      const qty = num(L('A32NX_CABIN_WATER_QTY_PERCENT'));
      const litres = num(L('A32NX_CABIN_WATER_QTY_KG'));
      const heatersOff = on(L('A380X_FAP_WATER_HEATERS_OFF'));
      const bar = (x, label, pct, l, colour) => {
        const p = Math.max(0, Math.min(100, pct || 0));
        return `<div style="position:absolute;left:${x}px;top:76px;width:70px;text-align:center" class="scale">${label}</div>
          <div style="position:absolute;left:${x + 18}px;top:96px;width:34px;height:200px;border-radius:5px;border:1px solid #555;background:linear-gradient(90deg,#d7d9db,#f6f7f8,#c9ccd0);overflow:hidden">
            <div style="position:absolute;left:0;right:0;bottom:0;height:${p * 2}px;background:linear-gradient(90deg,${colour},#ffffff55,${colour})"></div></div>
          <div style="position:absolute;left:${x}px;top:302px;width:70px;text-align:center;font:700 13px 'B612'">${fmt(pct, 0, ' %')}</div>
          <div style="position:absolute;left:${x}px;top:320px;width:70px;text-align:center" class="scale">${l === null ? '--' : `${l.toFixed(0)} L / ${(l / 3.785).toFixed(0)} US GAL`}</div>`;
      };
      let h = box(20, 44, 470, 330, 'panel', '<div style="display:flex"><div class="ttl" style="flex:3">WASTE QUANTITY</div><div class="ttl" style="flex:1.3">WATER QUANTITY</div></div>');
      for (let i = 1; i <= 3; i++) {
        const w = num(L(`A32NX_CABIN_WASTE_LEVEL_PERCENT:${i}`));
        h += bar(20 + (i - 1) * 90, `TANK ${ZONES[i - 1]}`, w, w === null ? null : w * 2, w > 90 ? '#c96a1b' : '#7d6c4e');
      }
      h += bar(340, 'POTABLE', qty, litres, '#2f7fd0');
      h += box(20, 390, 470, 128, 'inset', `<div class="otitle" style="margin-top:6px">SYSTEM STATUS</div><div class="status" style="columns:2">
        PRESSURE<div class="s">${fmt(num(L('A32NX_CABIN_WATER_PRESS_PSI')), 1, ' PSI')}</div>
        COMPRESSOR<div class="s">${on(L('A32NX_CABIN_WATER_COMPRESSOR_CMD')) ? 'ON' : 'OFF'}</div>
        LEAK<div class="s">${(num(L('A32NX_CABIN_WATER_LEAK_L_S')) || 0) > 0.001 ? 'DETECTED' : 'NONE'}</div>
        VACUUM GENERATOR<div class="s">${on(L('A32NX_CABIN_WASTE_GENERATOR_RUNNING')) ? 'RUNNING' : 'OFF'}</div>
        DRAIN MASTS<div class="s">${[1, 2].some((i) => on(L(`A32NX_CABIN_MAST_BLOCKED:${i}`))) ? 'BLOCKED' : 'CLEAR'}</div>
        LAVATORIES INOP<div class="s">${[1, 2, 3].filter((i) => on(L(`A32NX_CABIN_LAV_INOPERATIVE:${i}`))).map((i) => ZONES[i - 1]).join(' ') || 'NONE'}</div></div>`);
      h += box(506, 44, 274, 230, 'panel', '<div class="ttl">WATER HEATERS</div>');
      for (let i = 1; i <= 3; i++) h += `<div class="row" style="position:absolute;left:520px;top:${70 + (i - 1) * 22}px;width:240px;display:flex;justify-content:space-between;font:700 12px 'B612';color:#2a2e33"><span>HEATER ${ZONES[i - 1]}</span><span>${fmt(num(L(`A32NX_CABIN_WATER_HEATER_TEMP_C:${i}`)), 0, ' °C')}</span></div>`;
      h += key(560, 150, 160, 50, 'heaters', heatersOff ? 'WATER HEATERS<br>OFF' : 'WATER HEATERS<br>ON', heatersOff ? '' : 'on');
      return h;
    }

    smoke() {
      const det = [
        ['FWD CARGO', 'A32NX_CARGO_FWD_SMOKE_DETECTED', 1.6, -23.4], ['AFT CARGO', 'A32NX_CARGO_AFT_SMOKE_DETECTED', 1.6, 13],
        ...[1, 2, 3].map((i) => [`IFE ${ZONES[i - 1]}`, `A32NX_CABIN_IFE_ZONE_SMOKE:${i}`, -1.6, -20 + (i - 1) * 18]),
        ...[1, 2, 3].map((i) => [`GALLEY ${ZONES[i - 1]}`, `A32NX_CABIN_GALLEY_OVEN_SMOKE:${i}`, 0, -26 + (i - 1) * 22]),
      ];
      let h = box(20, 44, 300, 474, 'panel', '<div class="ttl">DETECTORS</div>');
      det.forEach(([label, v], k) => {
        const s = num(L(v));
        const c = s === null ? '#777' : s >= 0.5 ? '#d8473a' : '#25cf3a';
        h += `<div style="position:absolute;left:34px;top:${74 + k * 34}px;width:272px;height:26px;display:flex;align-items:center;justify-content:space-between;background:#eceef0;border:1px solid #8d9196;border-radius:3px;padding:0 8px;box-sizing:border-box;font:700 12px 'B612';color:#2a2e33"><span>${label}</span><span style="color:${c}">${s === null ? '--' : s >= 0.5 ? 'SMOKE' : 'NORMAL'}</span></div>`;
      });
      h += box(336, 44, 444, 474, 'inset', plan(444, 474, (P) => det.map(([label, v, x, z]) => {
        const s = num(L(v));
        const [px, py] = P(x, z);
        return `<circle cx="${px}" cy="${py}" r="7" fill="${s === null ? '#777' : s >= 0.5 ? '#ff4a3a' : '#25cf3a'}" stroke="#111"/><text x="${px + 10}" y="${py + 4}" font-size="9" fill="#e9eaec">${label}</text>`;
      }).join('')));
      return h;
    }

    seats() {
      const off = on(L('A380X_FAP_SEAT_POWER_OFF'));
      let h = '';
      for (let i = 1; i <= 3; i++) {
        const tripped = on(L(`A32NX_CABIN_IFE_ZONE_TRIPPED:${i}`));
        const hot = on(L(`A32NX_CABIN_IFE_ZONE_OVERHEAT:${i}`));
        h += box(20 + (i - 1) * 256, 44, 244, 200, 'panel', `<div class="ttl">ZONE ${ZONES[i - 1]}</div>`);
        h += box(30 + (i - 1) * 256, 70, 224, 160, 'inset', `<div class="status">IN-SEAT POWER<div class="s">${tripped ? 'TRIPPED' : on(L('A32NX_CABIN_SEAT_POWER_CMD')) ? 'ON' : 'OFF'}</div>EQUIPMENT TEMP<div class="s">${fmt(num(L(`A32NX_CABIN_IFE_ZONE_TEMP_C:${i}`)), 0, ' °C')}</div>${hot ? '<div class="w">OVERHEAT</div>' : ''}</div>`);
      }
      h += box(20, 262, 500, 140, 'panel', '<div class="ttl">IN-SEAT POWER SUPPLY</div><div class="note" style="padding:4px 14px">Seat power is shed with the galleys when commercial power is lost, and per zone when a zone trips.</div>');
      h += key(60, 330, 180, 54, 'seatpower', off ? 'SEAT POWER<br>OFF' : 'SEAT POWER<br>ON', off ? '' : 'on');
      h += box(540, 262, 240, 140, 'panel', '<div class="ttl">PASSENGER CALLS</div>');
      h += box(550, 286, 220, 40, 'inset', `<div class="status" style="padding:10px 14px">ACTIVE CALLS<span class="s">${num(L('A32NX_CABIN_CREW_CALL_NEW_COUNT')) || 0}</span></div>`);
      h += key(580, 340, 160, 50, 'callreset', 'CALL RESET', 'blue');
      return h;
    }

    sysinfo() {
      const st = (bad) => (bad ? 'FAULT' : 'NORMAL');
      let h = box(20, 44, 250, 230, 'panel', '<div class="ttl">CIDS / IFE</div>');
      h += box(30, 68, 230, 196, 'inset', `<div class="status">${[1, 2, 3].map((i) => `DIRECTOR ${i}<div class="s">${st(on(L(`A32NX_DEEP_COM_CIDS_${i}_FAULT`)))}</div>`).join('')}${[1, 2].map((i) => `IFE SERVER ${i}<div class="s">${st(on(L(`A32NX_CABIN_IFE_SERVER_FAIL:${i}`)))}</div>`).join('')}</div>`);
      h += box(20, 288, 250, 230, 'panel', '<div class="ttl">VENTILATION</div>');
      h += box(30, 312, 230, 196, 'inset', `<div class="status">LAV / GALLEY EXTRACT<div class="s">${st(on(L('A32NX_DEEP_CABIN_LAV_GALLEY_EXTRACT_FAULT')))}</div>SECONDARY FANS FAILED<div class="s">${num(L('A32NX_DEEP_CABIN_SECONDARY_FANS_FAILED_COUNT')) || 0}</div>IFE BAY<div class="s">${st(on(L('A32NX_DEEP_CABIN_IFE_BAY_VENT_FAULT')) || on(L('A32NX_DEEP_CABIN_IFE_BAY_ISOL_FAULT')))}</div></div>`);
      for (let i = 1; i <= 3; i++) {
        const off = on(L(`A380X_FAP_GALLEY_OFF_${i}`));
        const y = 44 + (i - 1) * 160;
        h += box(286, y, 494, 150, 'panel', `<div class="ttl" style="text-align:left;padding-left:8px">GALLEY ${ZONES[i - 1]}</div>`);
        h += box(296, y + 24, 300, 116, 'inset', `<div class="status" style="padding:6px 12px">SUPPLY<div class="s">${st(on(L(`A32NX_CABIN_GALLEY_BUS_FAULT:${i}`)))}</div>OVEN / CHILLER / BOILER<div class="s">${fmt(num(L(`A32NX_CABIN_GALLEY_OVEN_TEMP_C:${i}`)), 0)} / ${fmt(num(L(`A32NX_CABIN_GALLEY_CHILLER_TEMP_C:${i}`)), 1)} / ${fmt(num(L(`A32NX_CABIN_GALLEY_BOILER_TEMP_C:${i}`)), 0)} °C</div></div>`);
        h += key(620, y + 44, 140, 64, `galley:${i}`, off ? 'GALLEY POWER<br>OFF' : 'GALLEY POWER<br>ON', off ? '' : 'on');
      }
      return h;
    }
  }

  defineInstrument('a380x-fap', FAP);
})();

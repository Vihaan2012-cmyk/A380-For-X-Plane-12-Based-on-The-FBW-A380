const FIND_OBJ = (pred, slot) => `(() => {
  if (window.${slot}) return true;
  const pred = ${pred};
  const seen = new Set();
  const q = [...document.querySelectorAll('*')].filter((e) => e.tagName.includes('-')).map((e) => [e, 0]);
  while (q.length && seen.size < 300000) {
    const [o, d] = q.shift();
    if (!o || typeof o !== 'object' || seen.has(o)) continue;
    seen.add(o);
    try { if (pred(o)) { window.${slot} = o; return true; } } catch (e) {}
    if (d > 7 || (Array.isArray(o) && o.length > 64)) continue;
    let keys = [];
    try { keys = Object.getOwnPropertyNames(o); } catch (e) { continue; }
    for (const k of keys) {
      let v;
      try { v = o[k]; } catch (e) { continue; }
      if (v && typeof v === 'object' && !(v instanceof Node) && v !== window) q.push([v, d + 1]);
    }
  }
  return false;
})()`;

export default async function extra(c) {
  const { MFD, OITS, capture, log, nap, sleep, HELPERS, selectPage, mfdClickText, mfdClickLabel, mfdList, mfdClick, mfdTexts, enterField, revise, closeMenus, eraseTmpy, MENU_EL, OPEN_MENU_EXTRA, TAB } = c;
  const step = async (name, f) => {
    try {
      await f();
    } catch (e) {
      log(`extra ${name} failed:`, e.stack || e.message);
    }
  };
  const clickAny = async (...texts) => {
    for (const t of texts) if (await mfdClickText('.mfd-button, .mfd-button *, .mfd-context-menu-element, span, div', t)) return t;
    return null;
  };
  const tabs = async (prefix) => {
    const list = await mfdList(TAB);
    for (const t of list) {
      await mfdClick(TAB, t.i);
      await nap(700);
      await capture(`${prefix}/tab-${t.i}-${t.text}`, [MFD]);
    }
    if (list.length) {
      await mfdClick(TAB, 0);
      await nap(400);
    }
  };
  const openDropdownByLabel = async (label) => {
    const ok = await MFD.ev(`(() => { ${HELPERS}; const want = ${JSON.stringify(label)}; const labs = [...document.querySelectorAll('.mfd-label, span, div')].filter((el) => __TH.shown(el) && el.getBoundingClientRect().x < 820 && !el.children.length && __TH.text(el) === want); for (const lab of labs) { let p = lab.parentElement; for (let k = 0; k < 4 && p; k++, p = p.parentElement) { const dd = p.querySelector('.mfd-dropdown-outer'); if (dd && __TH.shown(dd)) { __TH.fire(dd); return true; } } } return false; })()`);
    await nap(600);
    return ok;
  };
  const fmc = async (expr) => {
    const found = await MFD.ev(FIND_OBJ("(o) => o.flightPhaseManager && o.fmgc && typeof o.flightPhaseManager.changePhase === 'function'", '__TOUR_FMC'));
    if (!found) throw new Error('no FMC object in the MFD page');
    return MFD.ev(`(() => { const f = window.__TOUR_FMC; return (${expr}); })()`);
  };
  const back = async () => {
    await closeMenus();
    await selectPage('ACTIVE', 'F-PLN');
    await eraseTmpy();
  };

  const mfdSteps = async () => {
    await MFD.ev(HELPERS);
    for (const ident of ['PKS', 'APOMA']) {
      for (const [item, tag] of [['HOLD', 'hold'], ['DIR TO', 'dir-to'], ['AIRWAYS', 'airways']]) {
        await step(`${ident} ${item}`, async () => {
          await selectPage('ACTIVE', 'F-PLN');
          if (!(await revise('.mfd-fms-fpln-line-ident, .mfd-fms-fpln-line-ident *', ident, item))) return;
          await capture(`extra/${tag}/${ident}`, [MFD]);
          if (tag === 'dir-to') {
            if (await openDropdownByLabel('DIRECT TO') || (await mfdClick('.mfd-dropdown-outer', 0, "!el.closest('.mfd-header-page-select-row, .mfd-header-sys-select-row')"))) {
              await nap(500);
              const items = await mfdList(MENU_EL, OPEN_MENU_EXTRA);
              const pick = items.find((x) => x.text.trim() === ident) || items[1];
              if (pick) {
                await mfdClick(MENU_EL, pick.i, OPEN_MENU_EXTRA);
                await nap(2500);
                await capture(`extra/dir-to/${ident}/selected-${pick.text}`, [MFD]);
              }
            }
            if (await clickAny('DIRECT WITH ABEAM')) {
              await nap(1000);
              await capture(`extra/dir-to/${ident}/abeam`, [MFD]);
            }
          }
          if (tag === 'hold') {
            if (await clickAny('DATABASE')) {
              await nap(1000);
              await capture(`extra/hold/${ident}/database`, [MFD]);
            }
          }
          if (tag === 'airways') {
            await MFD.ev(`(() => { ${HELPERS}; const box = [...document.querySelectorAll('.mfd-input-field-text-input-container')].find((el) => __TH.shown(el) && el.getBoundingClientRect().x < 820); if (box) __TH.fire(box); return !!box; })()`);
            await nap(300);
            await c.typeKeys('A576');
            await nap(1500);
            await capture(`extra/airways/${ident}/via-A576`, [MFD]);
          }
          await back();
        });
      }
    }
    await step('fpln footer DIR TO', async () => {
      await selectPage('ACTIVE', 'F-PLN');
      if (await mfdClickLabel('.mfd-button, .mfd-button *', 'DIRTO')) {
        await nap(1500);
        await capture('extra/dir-to/footer', [MFD]);
      }
      await back();
    });
    await step('departure', async () => {
      await selectPage('ACTIVE', 'F-PLN');
      if (!(await revise('.mfd-fms-fpln-line-ident, .mfd-fms-fpln-line-ident *', 'YSSY', 'DEPARTURE'))) return;
      await capture('extra/departure/page', [MFD]);
      for (const b of ['RWY', 'SID', 'TRANS']) {
        if (!(await mfdClickLabel('.mfd-button, .mfd-button *', b))) continue;
        await nap(800);
        await capture(`extra/departure/${b}-menu`, [MFD], (await mfdList(MENU_EL, OPEN_MENU_EXTRA)).map((x) => x.text).join(' | ').slice(0, 400));
        await closeMenus();
      }
      await back();
    });
    await step('arrival', async () => {
      await selectPage('ACTIVE', 'F-PLN');
      if (!(await revise('.mfd-fms-fpln-line-destination *', 'OMDB', null))) return;
      await capture('extra/arrival/page', [MFD]);
      for (const b of ['APPR', 'VIA', 'STAR', 'TRANS']) {
        if (!(await mfdClickLabel('.mfd-button, .mfd-button *', b))) continue;
        await nap(800);
        await capture(`extra/arrival/${b}-menu`, [MFD], (await mfdList(MENU_EL, OPEN_MENU_EXTRA)).map((x) => x.text).join(' | ').slice(0, 400));
        await closeMenus();
      }
      await back();
    });
    await step('init/fuel alternate', async () => {
      await selectPage('ACTIVE', 'INIT');
      await capture('extra/init/altn-uplinked', [MFD]);
      await selectPage('ACTIVE', 'FUEL&LOAD');
      await capture('extra/fuel-load/altn-uplinked', [MFD]);
      for (const [v, tag] of [['NONE', 'altn-none'], ['OMAA', 'altn-omaa']]) {
        await selectPage('ACTIVE', 'INIT');
        await enterField('ALTN', v);
        await nap(1500);
        await capture(`extra/init/${tag}`, [MFD]);
        await selectPage('ACTIVE', 'FUEL&LOAD');
        await nap(2000);
        await capture(`extra/fuel-load/${tag}`, [MFD]);
      }
    });
    await step('perf preflight', async () => {
      await selectPage('ACTIVE', 'PERF');
      await capture('extra/perf/to', [MFD]);
      const radio = async (i) =>
        MFD.ev(`(() => { ${HELPERS}; const r = [...document.querySelectorAll('input[id*="toThrustSettingRadio"]')].filter((el) => el.getBoundingClientRect().x < 820); const el = r[${i}]; if (!el) return false; el.click(); return true; })()`);
      if (await radio(2)) {
        await nap(1000);
        await capture('extra/perf/to-derated', [MFD]);
        const dd = await MFD.ev(`(() => { ${HELPERS}; const el = [...document.querySelectorAll('[id*="deratedDropdown"]')].find((e) => __TH.shown(e) && e.getBoundingClientRect().x < 820); if (!el) return false; const o = el.closest('.mfd-dropdown-outer') || el; __TH.fire(o); return true; })()`);
        await nap(700);
        const items = await mfdList(MENU_EL, OPEN_MENU_EXTRA);
        if (dd && items.length) {
          await capture('extra/perf/to-derated-menu', [MFD], items.map((x) => x.text).join(' | '));
          await mfdClick(MENU_EL, Math.min(1, items.length - 1), OPEN_MENU_EXTRA);
          await nap(1000);
          await capture('extra/perf/to-derated-dialog', [MFD]);
          await clickAny('CANCEL');
          await nap(600);
        } else log('extra: no derated dropdown');
        await radio(1);
        await nap(800);
      } else log('extra: no thrust setting radios');
      for (const t of [0, 1]) {
        await mfdClick(TAB, t);
        await nap(600);
        if (await clickAny('NOISE')) {
          await nap(1000);
          await capture(`extra/perf/noise-tab-${t}`, [MFD]);
        }
      }
      await mfdClick(TAB, 0);
      await nap(600);
      await capture('extra/perf/to-noise', [MFD]);
      await clickAny('CANCEL NOISE');
      await nap(600);
      const set = await fmc(`(f.fmgc.data.v1ToBeConfirmed.set(142), f.fmgc.data.vrToBeConfirmed.set(150), f.fmgc.data.v2ToBeConfirmed.set(158), true)`);
      log('extra: v-speeds to be confirmed', set);
      await selectPage('ACTIVE', 'F-PLN');
      await selectPage('ACTIVE', 'PERF');
      await nap(800);
      await capture('extra/perf/to-vspeed-confirm', [MFD]);
    });
    await step('perf tmpy', async () => {
      await selectPage('ACTIVE', 'F-PLN');
      if (await revise('.mfd-fms-fpln-line-ident, .mfd-fms-fpln-line-ident *', 'PKS', 'HOLD')) {
        await selectPage('ACTIVE', 'PERF');
        await capture('extra/perf/tmpy', [MFD]);
        await tabs('extra/perf/tmpy');
      }
      await back();
    });
    await step('perf secondary', async () => {
      if (!(await selectPage('SEC INDEX', 'SEC 1'))) log('extra: no SEC 1 item');
      await capture('extra/sec/index', [MFD]);
      if (await clickAny('COPY ACTIVE')) {
        await nap(2500);
        await capture('extra/sec/copied', [MFD]);
      }
      if (await mfdClickLabel('.mfd-button, .mfd-button *', 'PERF') || (await clickAny('SEC PERF'))) {
        await nap(1500);
        await capture('extra/perf/sec', [MFD]);
        await tabs('extra/perf/sec');
      } else log('extra: no PERF button on SEC INDEX');
      await selectPage('ACTIVE', 'F-PLN');
    });
    const phases = [['CLB', 2], ['CRZ', 3], ['DES', 4], ['APPR', 5], ['GA', 6]];
    for (const [name, n] of phases) {
      await step(`phase ${name}`, async () => {
        await fmc(`(f.flightPhaseManager.changePhase(${n}), true)`);
        await sleep(1500);
        const got = await MFD.getVar('L:A32NX_FMGC_FLIGHT_PHASE');
        log(`extra: phase ${name} -> L:A32NX_FMGC_FLIGHT_PHASE ${got}`);
        await selectPage('ACTIVE', 'F-PLN');
        await capture(`extra/phase-${name}/fpln`, [MFD], `flight phase ${got}`);
        await selectPage('ACTIVE', 'PERF');
        await capture(`extra/phase-${name}/perf`, [MFD], `flight phase ${got}`);
        await tabs(`extra/phase-${name}/perf`);
        if (name === 'CLB' || name === 'DES') {
          for (let t = 0; t < 6; t++) {
            await mfdClick(TAB, t);
            await nap(400);
            if (await clickAny('ACTIVATE APPR', 'ACTIVATE')) {
              await nap(800);
              await capture(`extra/phase-${name}/activate-appr-dialog`, [MFD]);
              await clickAny('CANCEL');
              await nap(500);
              break;
            }
          }
        }
        if (name === 'CRZ') {
          await fmc('(f.fmgc.data.engineOut.set(true), true)');
          await nap(1200);
          await capture('extra/phase-CRZ/perf-eo', [MFD]);
          await tabs('extra/phase-CRZ/perf-eo');
          for (let t = 0; t < 6; t++) {
            await mfdClick(TAB, t);
            await nap(400);
            if (await clickAny('CLEAR EO')) {
              await nap(800);
              await capture('extra/phase-CRZ/clear-eo-dialog', [MFD]);
              await clickAny('CANCEL');
              await nap(500);
              break;
            }
          }
          await fmc('(f.fmgc.data.engineOut.set(false), true)');
        }
      });
    }
  };

  const oitNav = async (v, uri) => {
    const found = await v.ev(FIND_OBJ("(o) => typeof o.navigateTo === 'function' && o.activeUri && typeof o.activeUri.get === 'function'", '__TOUR_OITUI'));
    if (!found) return false;
    await v.ev(`(window.__TOUR_OITUI.navigateTo(${JSON.stringify(uri)}), true)`);
    return true;
  };
  const oitClickText = (v, t) =>
    v.ev(`(() => { ${HELPERS}; const want = ${JSON.stringify(t.replace(/\s+/g, ''))}; const hits = [...document.querySelectorAll('body *')].filter((el) => __TH.shown(el) && __TH.text(el).replace(/\\s+/g, '') === want); const el = hits.find((h) => !hits.some((o) => o !== h && h.contains(o))); if (!el) return false; __TH.fire(el); return true; })()`);

  const oitSteps = async () => {
    const fo = OITS.find((v) => v.name === 'oit-2') || OITS[0];
    const capt = OITS.find((v) => v.name === 'oit-1') || OITS[1];
    if (fo) {
      await fo.ev(HELPERS);
      await oitClickText(fo, 'PILOT');
      await nap(1500);
      for (const [label, uri] of [['LDG PERF', 'flt-ops/ldg-perf'], ['IN-FLT PERF', 'flt-ops/in-flt-perf'], ['LOADSHEET', 'flt-ops/loadsheet'], ['OPS LIBRARY', 'flt-ops/ops-library'], ['LOAD BOX', 'flt-ops/load-box'], ['EXPORT BOX', 'flt-ops/export-box']]) {
        await step(`oit ${label}`, async () => {
          let how = 'menu';
          await oitClickText(fo, 'MENU');
          await nap(600);
          if (!(await oitClickText(fo, label))) {
            how = 'navigateTo';
            if (!(await oitNav(fo, uri))) how = 'none';
          }
          await nap(2000);
          await capture(`extra/oit/${fo.name}/${label}`, [fo], `opened by ${how}`);
        });
      }
    }
    if (capt) {
      await step('oit avncs', async () => {
        await capt.setVar('L:A380X_SWITCH_OIT_SIDE_LEFT', 1);
        await capt.setVar('L:A380X_SWITCH_OIT_SIDE_RIGHT', 0);
        await nap(3000);
        await capt.ev(HELPERS);
        await capture(`extra/oit/${capt.name}/avncs-home`, [capt]);
        await oitClickText(capt, 'PILOT');
        await nap(4000);
        await capture(`extra/oit/${capt.name}/avncs-logged-in`, [capt]);
        for (const [label, uri] of [['COMPANY COM', 'nss-avncs/company-com'], ['MAINTENANCE', 'nss-avncs/maintenance'], ['A380X SYSTEMS', 'nss-avncs/a380x-systems']]) {
          let how = 'click';
          if (!(await oitClickText(capt, label))) {
            how = 'navigateTo';
            if (!(await oitNav(capt, uri))) how = 'none';
          }
          await nap(2000);
          await capture(`extra/oit/${capt.name}/AVNCS ${label}`, [capt], `opened by ${how}`);
        }
        for (const uri of ['nss-avncs/company-com/inbox', 'nss-avncs/maintenance/system-status', 'nss-avncs/maintenance/logbook', 'nss-avncs/a380x-systems/app-ldg-cap']) {
          if (!(await oitNav(capt, uri))) continue;
          await nap(2000);
          await capture(`extra/oit/${capt.name}/${uri}`, [capt], 'opened by navigateTo');
        }
      });
    }
  };

  await Promise.all([mfdSteps(), oitSteps()]);
}

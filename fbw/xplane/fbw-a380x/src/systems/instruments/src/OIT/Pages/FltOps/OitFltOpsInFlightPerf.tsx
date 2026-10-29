/** @jsx UI.node */
/** @jsxFrag UI.Group */
//  Copyright (c) 2026 FlyByWire Simulations
//  SPDX-License-Identifier: GPL-3.0

import { TickTopics as ClockTopics, Part as View, Parts as UI, State as Signal, Handle as Bond, PartNode as ViewNode } from 'xplane-js/ui';
import { AbstractOitFltOpsPageProps } from '../../OIT';
import { Button } from '../../../MsfsAvionicsCommon/UiWidgets/Button';
import { arincValue, readWeights } from './OitFltOpsPerfCommon';
import { atmosphere, cruise, descent, engineOutCeilingFl, greenDotKcas, holding, optimumFl, plan, recMaxFl } from '../../System/InFlightPerf';
import { Vars } from 'xplane-js/data';

type Module = 'CRZ' | 'HLD' | 'DES' | 'PLAN' | 'ATMO';

const MODULES: Module[] = ['CRZ', 'HLD', 'DES', 'PLAN', 'ATMO'];

const ENGINES = [4, 3, 2];

interface Live {
  weightKg: number | null;
  fobKg: number | null;
  fl: number | null;
  mach: number | null;
  satC: number | null;
  tatC: number | null;
  isaDev: number | null;
  ffKgH: number;
  gsKt: number;
  windKt: number;
  windDir: number;
}

function readLive(): Live {
  const w = readWeights();
  const alt = arincValue('L:A32NX_ADIRS_ADR_1_ALTITUDE') ?? Vars.read('fbw/INDICATED_ALTITUDE', 'feet');
  const mach = arincValue('L:A32NX_ADIRS_ADR_1_MACH') ?? Vars.read('sim/flightmodel/misc/machno', 'mach');
  const sat = arincValue('L:A32NX_ADIRS_ADR_1_STATIC_AIR_TEMPERATURE') ?? Vars.read('sim/weather/aircraft/temperature_ambient_deg_c', 'celsius');
  const tat = arincValue('L:A32NX_ADIRS_ADR_1_TOTAL_AIR_TEMPERATURE');
  let ff = 0;
  for (let e = 1; e <= 4; e++) {
    ff += Vars.read(`L:A32NX_ENGINE_FF:${e}`, 'number') || 0;
  }
  const isa = typeof alt === 'number' && typeof sat === 'number' ? sat - (atmosphere(alt).temperatureK - 273.15) : null;
  return {
    weightKg: w.grossWeightKg,
    fobKg: w.fobKg,
    fl: typeof alt === 'number' ? Math.max(0, Math.round(alt / 100)) : null,
    mach: typeof mach === 'number' ? mach : null,
    satC: typeof sat === 'number' ? sat : null,
    tatC: tat,
    isaDev: isa,
    ffKgH: ff,
    gsKt: Vars.read('fbw/GPS_GROUND_SPEED', 'knots') || 0,
    windKt: Vars.read('sim/weather/aircraft/wind_now_speed_msc', 'knots') || 0,
    windDir: Vars.read('sim/weather/aircraft/wind_now_direction_degt', 'degrees') || 0,
  };
}

let fmCalibration = 1;
let fmSamples = 0;

function learnFlightModel(l: Live, vsFpm: number): void {
  if (l.weightKg === null || l.fl === null || l.mach === null || l.isaDev === null || l.fl < 200 || l.mach < 0.7 || Math.abs(vsFpm) > 300 || l.ffKgH < 2000) {
    return;
  }
  const model = cruise(l.weightKg, l.fl, l.mach, l.isaDev).fuelFlowKgH;
  const ratio = Math.min(1.6, Math.max(0.6, l.ffKgH / model));
  fmSamples++;
  fmCalibration += (ratio - fmCalibration) * (fmSamples < 20 ? 1 / fmSamples : 0.05);
}

const t = (kg: number) => (kg / 1000).toFixed(1);
const hhmm = (min: number) => `${Math.floor(min / 60)}H${String(Math.round(min % 60)).padStart(2, '0')}`;
const fl = (v: number) => `FL${String(Math.round(v)).padStart(3, '0')}`;

export class OitFltOpsInFlightPerf extends View<AbstractOitFltOpsPageProps> {
  private readonly subs = [] as Bond[];

  private readonly module = Signal.of<Module>('CRZ');

  private readonly engines = Signal.of(4);

  private readonly liveWeight = Signal.of(true);

  private readonly liveLevel = Signal.of(true);

  private readonly weightKg = Signal.of(450000);

  private readonly level = Signal.of(370);

  private readonly mach = Signal.of(0.85);

  private readonly isaDev = Signal.of(0);

  private readonly fuelFactor = Signal.of(1);

  private readonly distanceNm = Signal.of(500);

  private readonly windKt = Signal.of(0);

  private readonly elevationFt = Signal.of(0);

  private readonly inputText = Signal.of('');

  private readonly resultsRef = UI.ref<HTMLDivElement>();

  private live: Live = readLive();

  private step(subject: Signal<number>, delta: number, min: number, max: number, digits = 0): void {
    const v = Math.round((subject.get() + delta) * 10 ** digits) / 10 ** digits;
    subject.set(Math.min(max, Math.max(min, v)));
    this.refresh();
  }

  private stepper(label: string, minus: () => void, plus: () => void, value: Signal<string>, live?: Signal<boolean>): ViewNode {
    return (
      <div class="fr oit-perf-row" style="align-items: center;">
        <div class="oit-label bigger oit-perf-key" style="width: 230px;">
          {label}
        </div>
        <Button label="−" onClick={minus} containerStyle="width: 70px; margin-right: 10px;" />
        <div class="oit-label bigger oit-green-text" style="width: 220px; text-align: center;">
          {value}
        </div>
        <Button label="+" onClick={plus} containerStyle="width: 70px; margin-right: 20px;" />
        {live ? (
          <Button
            label="LIVE"
            selected={live}
            onClick={() => {
              live.set(!live.get());
              this.refresh();
            }}
            containerStyle="width: 110px;"
          />
        ) : (
          <div />
        )}
      </div>
    );
  }

  private readonly weightText = Signal.of('');

  private readonly levelText = Signal.of('');

  private readonly machText = Signal.of('');

  private readonly isaText = Signal.of('');

  private readonly factorText = Signal.of('');

  private readonly distanceText = Signal.of('');

  private readonly windText = Signal.of('');

  private readonly elevationText = Signal.of('');

  private rows(lines: [string, string][]): void {
    const root = this.resultsRef.peek();
    if (!root) {
      return;
    }
    while (root.firstChild) {
      root.removeChild(root.firstChild);
    }
    UI.mount(
      <div class="oit-perf-grid">
        {lines.flatMap(([k, v]) => [<div class="oit-label bigger">{k}</div>, <div class="oit-label bigger oit-green-text">{v}</div>])}
      </div>,
      root,
    );
  }

  private refresh(): void {
    this.live = readLive();
    const l = this.live;
    learnFlightModel(l, Vars.read('sim/flightmodel/position/vh_ind_fpm', 'feet per minute') || 0);
    if (this.liveWeight.get() && l.weightKg !== null && l.weightKg > 100000) {
      this.weightKg.set(Math.round(l.weightKg / 100) * 100);
    }
    if (this.liveLevel.get() && l.fl !== null) {
      this.level.set(Math.max(100, Math.min(431, Math.round(l.fl / 10) * 10)));
      if (l.mach !== null && l.mach > 0.5) {
        this.mach.set(Math.round(l.mach * 100) / 100);
      }
      if (l.isaDev !== null) {
        this.isaDev.set(Math.round(l.isaDev));
      }
    }
    const w = this.weightKg.get();
    const level = this.level.get();
    const m = this.mach.get();
    const isa = this.isaDev.get();
    const eng = this.engines.get();
    const factor = this.fuelFactor.get() * fmCalibration;
    this.weightText.set(`${t(w)} T`);
    this.levelText.set(fl(level));
    this.machText.set(`M ${m.toFixed(2)}`);
    this.isaText.set(`ISA ${isa >= 0 ? '+' : ''}${isa}`);
    this.factorText.set(this.fuelFactor.get().toFixed(2));
    this.distanceText.set(`${this.distanceNm.get()} NM`);
    this.windText.set(`${this.windKt.get() >= 0 ? 'TL' : 'HD'} ${Math.abs(this.windKt.get())} KT`);
    this.elevationText.set(`${this.elevationFt.get()} FT`);
    this.inputText.set(`${eng === 4 ? 'ALL ENGINES' : eng === 3 ? 'ONE ENGINE INOPERATIVE' : 'TWO ENGINES INOPERATIVE'}, FUEL FACTOR ${this.fuelFactor.get().toFixed(2)}, X-PLANE FM CAL ${fmSamples > 0 ? fmCalibration.toFixed(3) : 'PENDING (STABLE CRUISE NEEDED)'}`);

    switch (this.module.get()) {
      case 'CRZ': {
        const c = cruise(w, level, m, isa, factor, eng);
        const rec = eng === 4 ? recMaxFl(w, m, isa) : engineOutCeilingFl(w, eng, isa);
        this.rows([
          ['FUEL FLOW', `${Math.round(c.fuelFlowKgH)} KG/H`],
          ['SPEC RANGE', `${c.specificRangeNmPerT.toFixed(1)} NM/T`],
          ['TAS', `${Math.round(c.tasKt)} KT`],
          ['THRUST/ENG', `${(c.thrustPerEngineN / 1000).toFixed(0)} KN`],
          [eng === 4 ? 'REC MAX' : 'EO CEILING', fl(rec)],
          ['OPT FL', eng === 4 ? fl(optimumFl(w, m, isa)) : '---'],
          ['1 ENG OUT CEILING', fl(engineOutCeilingFl(w, 3, isa))],
          ['2 ENG OUT CEILING', fl(engineOutCeilingFl(w, 2, isa))],
          ['LIVE FUEL FLOW', `${Math.round(l.ffKgH)} KG/H`],
          ['LIVE ENDURANCE', l.fobKg !== null && l.ffKgH > 500 ? hhmm((l.fobKg / l.ffKgH) * 60) : '---'],
        ]);
        break;
      }
      case 'HLD': {
        const h = holding(w, level, isa, eng);
        const fob = l.fobKg ?? 0;
        this.rows([
          ['HOLD SPEED (GD)', `${Math.round(h.kcas)} KT`],
          ['FUEL FLOW', `${Math.round(h.fuelFlowKgH * factor)} KG/H`],
          ['FUEL PER 30 MIN', `${t((h.fuelFlowKgH * factor) / 2)} T`],
          ['MAX HOLD TIME', fob > 0 ? hhmm((fob / (h.fuelFlowKgH * factor)) * 60) : '---'],
          ['GREEN DOT FL200', `${Math.round(greenDotKcas(w, 200))} KT`],
          ['GREEN DOT FL100', `${Math.round(greenDotKcas(w, 100))} KT`],
        ]);
        break;
      }
      case 'DES': {
        const d = descent(level, this.elevationFt.get(), w, this.windKt.get());
        this.rows([
          ['DISTANCE', `${Math.round(d.distanceNm)} NM`],
          ['TIME', hhmm(d.timeMin)],
          ['FUEL', `${t(d.fuelKg * factor)} T`],
          ['GRADIENT', `${(((level * 100 - this.elevationFt.get()) / Math.max(1, d.distanceNm)) / 1000).toFixed(1)} FT/1000 PER NM`],
        ]);
        break;
      }
      case 'PLAN': {
        const fob = l.fobKg ?? 0;
        const p = plan(w, fob, this.distanceNm.get(), level, m, isa, this.windKt.get(), factor);
        const dFuel = descent(level, this.elevationFt.get(), w - p.tripFuelKg, this.windKt.get()).fuelKg * factor;
        this.rows([
          ['TRIP FUEL', `${t(p.tripFuelKg + dFuel)} T`],
          ['TRIP TIME', hhmm(p.timeMin)],
          ['FOB NOW', fob > 0 ? `${t(fob)} T` : '---'],
          ['EFOB AT DEST', fob > 0 ? `${t(p.efobKg - dFuel)} T` : '---'],
          ['LANDING WEIGHT', `${t(w - p.tripFuelKg - dFuel)} T`],
        ]);
        break;
      }
      case 'ATMO': {
        const atm = atmosphere(level * 100, isa);
        this.rows([
          ['PRESS ALT', l.fl !== null ? `${l.fl * 100} FT` : '---'],
          ['SAT', l.satC !== null ? `${Math.round(l.satC)}°C` : '---'],
          ['TAT', l.tatC !== null ? `${Math.round(l.tatC)}°C` : '---'],
          ['ISA DEV', l.isaDev !== null ? `${l.isaDev >= 0 ? '+' : ''}${Math.round(l.isaDev)}°C` : '---'],
          ['WIND', `${String(Math.round(l.windDir)).padStart(3, '0')}°/${Math.round(l.windKt)} KT`],
          ['GROUND SPEED', `${Math.round(l.gsKt)} KT`],
          ['DENSITY RATIO', atm.sigma.toFixed(3)],
          ['SPEED OF SOUND', `${Math.round(atm.soundMs / 0.514444)} KT`],
        ]);
        break;
      }
    }
  }

  public afterMount(node: ViewNode): void {
    super.afterMount(node);
    const sub = this.props.bus.receiver<ClockTopics>();
    this.subs.push(
      sub
        .topic('realTime')
        .sampled(1)
        .listen(() => this.refresh()),
      this.module.watch(() => this.refresh()),
      this.engines.watch(() => this.refresh()),
    );
    this.refresh();
  }

  public destroy(): void {
    for (const s of this.subs) {
      s.destroy();
    }
    super.destroy();
  }

  render(): ViewNode {
    return (
      <div class="oit-page-container oit-perf-page">
        <div class="fr oit-perf-row">
          {MODULES.map((m) => (
            <Button label={m} onClick={() => this.module.set(m)} selected={this.module.derive((x) => x === m)} containerStyle="width: 150px; margin-right: 15px;" />
          ))}
          <div style="flex-grow: 1" />
          {ENGINES.map((n) => (
            <Button
              label={n === 4 ? 'AEO' : n === 3 ? 'OEI' : 'TEO'}
              onClick={() => this.engines.set(n)}
              selected={this.engines.derive((x) => x === n)}
              containerStyle="width: 110px; margin-left: 10px;"
            />
          ))}
        </div>
        <div class="oit-labeled-box-container">
          <span class="oit-labeled-box-label">INPUT</span>
          {this.stepper('WEIGHT', () => this.step(this.weightKg, -5000, 250000, 600000), () => this.step(this.weightKg, 5000, 250000, 600000), this.weightText, this.liveWeight)}
          {this.stepper('FLIGHT LEVEL', () => this.step(this.level, -10, 100, 431), () => this.step(this.level, 10, 100, 431), this.levelText, this.liveLevel)}
          {this.stepper('MACH', () => this.step(this.mach, -0.01, 0.6, 0.89, 2), () => this.step(this.mach, 0.01, 0.6, 0.89, 2), this.machText)}
          {this.stepper('TEMPERATURE', () => this.step(this.isaDev, -1, -40, 40), () => this.step(this.isaDev, 1, -40, 40), this.isaText)}
          {this.stepper('FUEL FACTOR', () => this.step(this.fuelFactor, -0.01, 0.9, 1.3, 2), () => this.step(this.fuelFactor, 0.01, 0.9, 1.3, 2), this.factorText)}
          {this.stepper('DISTANCE', () => this.step(this.distanceNm, -50, 0, 8000), () => this.step(this.distanceNm, 50, 0, 8000), this.distanceText)}
          {this.stepper('WIND', () => this.step(this.windKt, -10, -200, 200), () => this.step(this.windKt, 10, -200, 200), this.windText)}
          {this.stepper('DEST ELEVATION', () => this.step(this.elevationFt, -100, -1000, 14000), () => this.step(this.elevationFt, 100, -1000, 14000), this.elevationText)}
        </div>
        <div class="oit-labeled-box-container">
          <span class="oit-labeled-box-label">{this.module.derive((m) => `${m} RESULTS`)}</span>
          <div class="oit-label oit-perf-note">{this.inputText}</div>
          <div ref={this.resultsRef} />
        </div>
        <div class="oit-label amber oit-perf-note">
          OIS PERFORMANCE MODEL: DRAG POLAR, THRUST LAPSE AND FUEL FLOW CALIBRATED TO TYPICAL A380 VALUES, FUEL RESULTS SCALED BY THE X-PLANE FM CAL LEARNT IN STABLE CRUISE. NOT AIRBUS-CERTIFIED;
          FMS PREDICTIONS AND THE QRH PREVAIL.
        </div>
      </div>
    );
  }
}

/** @jsx UI.node */
/** @jsxFrag UI.Group */
//  Copyright (c) 2025 FlyByWire Simulations
//  SPDX-License-Identifier: GPL-3.0

import { StateList as SignalList, Parts as UI, ComputedState as Derived, State as Signal, PartNode as ViewNode, makeStateList as $list, makeComputed as $derive, makeState as $cell } from 'xplane-js/ui';
import { DestroyableComponent } from '@flybywiresim/msfs-avionics-common';
import { FmsData, getSimBriefOfp, ISimbriefData, ISimbriefTlrPhase } from '@flybywiresim/fbw-sdk';
import { AbstractOitFltOpsPageProps } from '../../OIT';
import { Button } from '../../../MsfsAvionicsCommon/UiWidgets/Button';
import { DropdownMenu } from '../../../MsfsAvionicsCommon/UiWidgets/DropdownMenu';
import { InputField } from '../../../MsfsAvionicsCommon/UiWidgets/InputField';
import { AirportFormat, LengthFormat } from '../../../MFD/pages/common/DataEntryFormats';
import { OitFltOpsPerfOptions } from './OitFltOpsPerfOptions';
import {
  A380_MAX_GW_KG,
  A380_MAX_ZFW_KG,
  refreshWeightVariant,
  DEFAULT_UNITS,
  OisRunwayEnd,
  OisUnits,
  UNAVAILABLE,
  UNAVAILABLE_REASON,
  arincValue,
  distanceUnitLabel,
  formatDistance,
  formatPressure,
  formatWeight,
  loadAirport,
  pressureAltitude,
  pressureUnitLabel,
  readWeather,
  readWeights,
  runwayEnds,
  weightUnitLabel,
  windComponents,
} from './OitFltOpsPerfCommon';
import { AntiIce, LINEUP, TO_COND, ToConf, atmos, calibrationText, loadCalibration, takeoff } from '../../System/PerfModel';
import { Vars } from 'xplane-js/data';

const ENTRY_ANGLES = ['0', '90', '180'];
const RUNWAY_CONDITIONS = ['DRY', 'WET'];
const ANTI_ICE = ['OFF [STD]', 'ON [ENG]', 'ON [ENG & WING]'];
const AIR_COND = ['ON [STD]', 'OFF'];
const FLAPS = ['OPTIMUM', '1+F', '2', '3'];
const THRUST = ['FLEX [STD]', 'TOGA'];

export class OitFltOpsTakeoffPerformance extends DestroyableComponent<AbstractOitFltOpsPageProps> {
  private readonly units = Signal.of<OisUnits>(DEFAULT_UNITS);

  private readonly sub = this.props.bus.receiver<FmsData>();

  private readonly airport = Signal.of<string | null>(null);
  private readonly runwayIdents = SignalList.of<string>([]);
  private readonly selectedRunway = Signal.of<number | null>(null);
  private readonly runways = Signal.of<OisRunwayEnd[]>([]);
  private readonly databaseMessage = $cell('');
  private readonly takeoffShift = Signal.of<number | null>(null);
  private readonly entryAngle = Signal.of<number | null>(0);

  private readonly runway = $derive(
    ([list, index]) => (index === null ? null : (list[index] ?? null)),
    this.runways,
    this.selectedRunway,
  );

  private readonly usableTora = $derive(
    ([rwy, shift]) => (rwy === null ? null : Math.max(0, rwy.tora - (shift ?? 0))),
    this.runway,
    this.takeoffShift,
  );

  private readonly usableAsda = $derive(
    ([rwy, shift]) => (rwy === null ? null : Math.max(0, rwy.asda - (shift ?? 0))),
    this.runway,
    this.takeoffShift,
  );

  private readonly oat = Signal.of<number | null>(null);
  private readonly qnh = Signal.of<number | null>(null);
  private readonly windText = $cell('---°/---');
  private readonly headwindText = $cell('---');
  private readonly crosswindText = $cell('---');
  private readonly pressureAltText = $cell('-----');
  private readonly weatherSource = $cell('NO DATA');

  private readonly runwayCondition = Signal.of<number | null>(0);
  private readonly antiIce = Signal.of<number | null>(0);
  private readonly airCond = Signal.of<number | null>(0);
  private readonly flaps = Signal.of<number | null>(0);
  private readonly thrust = Signal.of<number | null>(0);

  private readonly zfwText = $cell('---.-');
  private readonly zfwCgText = $cell('--.-');
  private readonly fobText = $cell('---.-');
  private readonly towText = $cell('---.-');
  private readonly toCgText = $cell('--.-');
  private readonly weightWarning = $cell('');

  private readonly computed = $cell(false);
  private readonly thrRedText = $cell('-----');
  private readonly eoAccelText = $cell('-----');
  private readonly resultRunwayText = $cell('----/---');

  private tlrTakeoff: ISimbriefTlrPhase | null = null;
  private tlrWeightUnits = '';
  private readonly v1Text = $cell(UNAVAILABLE);
  private readonly vrText = $cell(UNAVAILABLE);
  private readonly v2Text = $cell(UNAVAILABLE);
  private readonly flapsText = $cell(UNAVAILABLE);
  private readonly flexText = $cell(UNAVAILABLE);
  private readonly stopMarginText = $cell(UNAVAILABLE);
  private readonly resultNote = $cell(UNAVAILABLE_REASON);
  private readonly fromSimbrief = $cell(false);
  private readonly fromModel = $cell(false);
  private readonly thsText = $cell(UNAVAILABLE);
  private readonly mtowText = $cell(UNAVAILABLE);

  public afterMount(node: ViewNode): void {
    super.afterMount(node);

    this.subscriptions.push(
      this.runway,
      this.usableTora,
      this.usableAsda,
      this.sub.topic('fmsOrigin').listen((origin) => {
        this.fmsOriginValue = origin;
        if (origin && this.airport.get() === null) {
          this.airport.set(origin);
        }
      }),
      this.sub.topic('fmsDepartureRunway').listen((runway) => {
        this.fmsDepartureRunwayValue = runway;
        if (runway && this.selectedRunway.get() === null) {
          this.selectRunway(this.fmsDepartureRunwayIdent);
        }
      }),
      this.airport.watch((ident) => {
        if (ident) {
          this.loadRunways(ident);
        } else {
          this.runways.set([]);
          this.runwayIdents.clear();
          this.selectedRunway.set(null);
        }
      }),
      this.runway.watch(() => this.refreshConditions()),
      this.units.watch(() => {
        this.refreshWeights();
        if (this.computed.get()) {
          this.showTlr(this.runway.get()?.ident ?? null);
        }
      }),
    );

    this.refreshConditions();
    this.refreshWeights();
  }

  private loadRunways(ident: string): void {
    this.databaseMessage.set('LOADING RUNWAY DATA...');
    loadAirport(ident)
      .then((airport) => {
        if (this.airport.get() !== ident) {
          return;
        }
        if (airport === null) {
          this.runways.set([]);
          this.runwayIdents.clear();
          this.selectedRunway.set(null);
          this.databaseMessage.set(`${ident} NOT IN NAVIGATION DATABASE`);
          return;
        }
        const ends = runwayEnds(airport);
        this.runways.set(ends);
        this.runwayIdents.set(ends.map((r) => r.ident));
        this.databaseMessage.set(ends.length === 0 ? `${ident} HAS NO RUNWAY DATA` : '');

        this.selectRunway(this.fmsDepartureRunwayIdent);
      })
      .catch(() => this.databaseMessage.set('NAVIGATION DATABASE UNAVAILABLE'));
  }

  private fmsOriginValue: string | null = null;

  private fmsDepartureRunwayValue: string | null = null;

  private get fmsDepartureRunwayIdent(): string | null {
    const raw = this.fmsDepartureRunwayValue;
    return raw && raw.length > 4 ? raw.substring(4) : null;
  }

  private selectRunway(ident: string | null): void {
    const index = ident === null ? -1 : this.runways.get().findIndex((r) => r.ident === ident);
    this.selectedRunway.set(index >= 0 ? index : null);
  }

  private syncFms(): void {
    if (this.fmsOriginValue) {
      if (this.airport.get() === this.fmsOriginValue) {
        this.selectRunway(this.fmsDepartureRunwayIdent);
      } else {
        this.airport.set(this.fmsOriginValue);
      }
    } else {
      this.databaseMessage.set('FMS HAS NO DEPARTURE');
    }
    this.refreshConditions();
    this.refreshWeights();
  }

  private async syncSimbrief(): Promise<void> {
    this.databaseMessage.set('SIMBRIEF REQUEST SENT');
    let ofp: ISimbriefData;
    try {
      ofp = await getSimBriefOfp();
    } catch (e) {
      this.databaseMessage.set(`SIMBRIEF: ${String(e instanceof Error ? e.message : e).toUpperCase()}`);
      return;
    }

    const origin = ofp.origin?.icao ?? null;
    if (!origin) {
      this.databaseMessage.set('SIMBRIEF OFP HAS NO DEPARTURE');
      return;
    }
    if (this.airport.get() === origin) {
      this.selectRunway(ofp.origin.runway || null);
    } else {
      this.airport.set(origin);
    }

    this.tlrTakeoff = ofp.tlr?.takeoff ?? null;
    this.tlrWeightUnits = ofp.units === 'lbs' ? 'LB' : 'KG';

    const planned = Number(ofp.weights?.estTakeOffWeight);
    this.databaseMessage.set(
      Number.isFinite(planned) && planned > 0
        ? `SIMBRIEF ${origin}/${ofp.origin.runway || '--'} - PLANNED TOW ${Math.round(planned / 1000)} T (WEIGHTS BELOW ARE THE AIRCRAFT'S OWN)`
        : `SIMBRIEF ${origin}/${ofp.origin.runway || '--'}`,
    );
    this.refreshConditions();
    this.refreshWeights();
  }

  private refreshConditions(): void {
    const weather = readWeather();
    const units = this.units.get();
    this.oat.set(Math.round(weather.oat));
    this.qnh.set(weather.qnh);
    this.weatherSource.set('SIMULATED AMBIENT AT AIRCRAFT');
    this.windText.set(
      `${Math.round(weather.windDirectionTrue).toString().padStart(3, '0')}°T/${Math.round(weather.windKt)
        .toString()
        .padStart(3, '0')}KT`,
    );

    const rwy = this.runway.get();
    if (rwy === null) {
      this.headwindText.set('---');
      this.crosswindText.set('---');
      this.pressureAltText.set('-----');
      return;
    }
    const { headwind, crosswind } = windComponents(rwy.bearingTrue, weather);
    this.headwindText.set(`${headwind >= 0 ? 'HEAD' : 'TAIL'} ${Math.abs(headwind).toFixed(0)}KT`);
    this.crosswindText.set(`${crosswind >= 0 ? 'R' : 'L'} ${Math.abs(crosswind).toFixed(0)}KT`);
    this.pressureAltText.set(`${Math.round(pressureAltitude(rwy.elevationFt, weather.qnh))} FT`);
    void units;
  }

  private refreshWeights(): void {
    refreshWeightVariant();
    const units = this.units.get();
    const w = readWeights();
    this.zfwText.set(formatWeight(w.zeroFuelWeightKg, units));
    this.zfwCgText.set(Number.isFinite(w.zeroFuelCgPercentMac) ? w.zeroFuelCgPercentMac.toFixed(1) : '--.-');
    this.fobText.set(formatWeight(w.fobKg, units));
    this.towText.set(formatWeight(w.grossWeightKg, units));
    this.toCgText.set(w.cgPercentMac === null ? '--.-' : w.cgPercentMac.toFixed(1));

    const warnings: string[] = [];
    if (w.zeroFuelWeightKg > A380_MAX_ZFW_KG) {
      warnings.push('ZFW ABOVE STRUCTURAL LIMIT');
    }
    if (w.grossWeightKg !== null && w.grossWeightKg > A380_MAX_GW_KG) {
      warnings.push('GW ABOVE STRUCTURAL LIMIT');
    }
    this.weightWarning.set(warnings.join(' - '));

    this.thrRedText.set(this.altitudeText(arincValue('L:A32NX_FM1_THR_RED_ALT')));
    this.eoAccelText.set(this.altitudeText(arincValue('L:A32NX_FM1_EO_ACC_ALT')));
  }

  private altitudeText(value: number | null): string {
    return value === null ? '-----' : `${Math.round(value)} FT`;
  }

  private compute(): void {
    this.refreshConditions();
    this.refreshWeights();
    const rwy = this.runway.get();
    this.resultRunwayText.set(rwy === null ? '----/---' : `${this.airport.get() ?? '----'}/${rwy.ident}`);
    this.computed.set(rwy !== null);
    this.showTlr(rwy === null ? null : rwy.ident);
  }

  private showTlr(ident: string | null): void {
    const reset = () => {
      for (const s of [this.v1Text, this.vrText, this.v2Text, this.flapsText, this.flexText, this.stopMarginText, this.thsText, this.mtowText]) {
        s.set(UNAVAILABLE);
      }
      this.fromSimbrief.set(false);
      this.fromModel.set(false);
    };
    reset();
    if (this.tlrTakeoff === null) {
      this.showModel('NO SIMBRIEF TAKE-OFF REPORT');
      return;
    }
    const runways = this.tlrTakeoff.runway;
    const list = runways === undefined ? [] : Array.isArray(runways) ? runways : [runways];
    const entry = ident === null ? undefined : list.find((r) => r.identifier === ident);
    if (!entry) {
      this.showModel(list.length === 0 ? 'THE SIMBRIEF OFP HAS NO TAKE-OFF REPORT' : `THE SIMBRIEF TAKE-OFF REPORT HAS NO RWY ${ident ?? '---'}`);
      return;
    }
    const num = (v: string | undefined): number | null => {
      const n = v === undefined || v.trim() === '' ? NaN : Number(v);
      return Number.isFinite(n) ? n : null;
    };
    const units = this.units.get();
    const kt = (v: string | undefined) => {
      const n = num(v);
      return n === null ? UNAVAILABLE : `${Math.round(n)} KT`;
    };
    this.v1Text.set(kt(entry.speeds_v1));
    this.vrText.set(kt(entry.speeds_vr));
    this.v2Text.set(kt(entry.speeds_v2));

    const flaps = entry.flap_setting?.trim();
    this.flapsText.set(!flaps ? UNAVAILABLE : /^\d/.test(flaps) ? `CONF ${flaps}` : flaps);

    const flex = num(entry.flex_temperature);
    const thrust = entry.thrust_setting?.trim().toUpperCase();
    this.flexText.set(flex !== null ? `${Math.round(flex)}°C` : thrust === 'FULL' || thrust === 'TOGA' ? 'TOGA' : thrust || UNAVAILABLE);

    const rwy = this.runway.get();
    const tlrLength = num(entry.length_tora) ?? num(entry.length);
    let toMetres: number | null = null;
    if (rwy !== null && tlrLength !== null && rwy.tora > 0) {
      const ratio = tlrLength / rwy.tora;
      toMetres = Math.abs(ratio - 1) < 0.2 ? 1 : Math.abs(ratio - 3.28084) < 0.66 ? 0.3048 : null;
    }
    const margin = num(entry.distance_margin);
    this.stopMarginText.set(
      margin === null || toMetres === null ? UNAVAILABLE : `${formatDistance(margin * toMetres, units)} ${distanceUnitLabel(units)}`,
    );

    this.fromSimbrief.set(true);
    const c = this.tlrTakeoff.conditions ?? {};
    const plannedWeight = num(c.planned_weight);
    const weightKg = plannedWeight === null ? null : this.tlrWeightUnits === 'LB' ? plannedWeight * 0.45359237 : plannedWeight;
    const parts = [`SIMBRIEF TAKE-OFF REPORT, RWY ${entry.identifier}`];
    if (weightKg !== null) {
      parts.push(`AT ${formatWeight(weightKg, units)} ${weightUnitLabel(units)}`);
    }
    const assumed = [
      c.temperature ? `${c.temperature}°C` : '',
      c.wind_direction && c.wind_speed ? `WIND ${c.wind_direction.padStart(3, '0')}/${c.wind_speed}KT` : '',
      c.surface_condition ? c.surface_condition.toUpperCase() : '',
    ].filter((x) => x !== '');
    this.resultNote.set(
      `${parts.join(' ')}${assumed.length ? ` (SIMBRIEF ASSUMED ${assumed.join(', ')})` : ''}: SIMBRIEF PLANNING DATA, NOT AIRBUS-CERTIFIED A380 PERFORMANCE. THS AND PERF MTOW ARE NOT IN THE REPORT.`,
    );
  }

  render(): ViewNode {
    const units = this.units;
    return (
      <>
        <div class="oit-page-container oit-perf-page">
          <div class="oit-labeled-box-container">
            <span class="oit-labeled-box-label">DEPARTURE</span>
            <div class="fr oit-perf-row">
              <div class="oit-label bigger oit-perf-key">AIRPORT</div>
              <InputField<string>
                dataEntryFormat={new AirportFormat()}
                mandatory={$cell(true)}
                value={this.airport}
                containerStyle="width: 160px; margin-right: 20px;"
                alignText="center"
                hEventConsumer={this.props.container.hEventConsumer}
                interactionMode={this.props.container.interactionMode}
                errorHandler={() => {}}
              />
              <div class="oit-label bigger oit-perf-field-key">RWY</div>
              <DropdownMenu
                idPrefix={`${this.props.uiService.captOrFo}_OIT_toPerfRunway`}
                selectedIndex={this.selectedRunway}
                values={this.runwayIdents}
                freeTextAllowed={false}
                containerStyle="width: 180px; margin-right: 20px;"
                alignLabels="center"
                numberOfDigitsForInputField={4}
                tmpyActive={$cell(false)}
                hEventConsumer={this.props.container.hEventConsumer}
                interactionMode={this.props.container.interactionMode}
              />
              <Button
                label="SYNC FMS"
                onClick={() => this.syncFms()}
                containerStyle="width: 175px; margin-right: 10px;"
              />
              <Button
                label="SYNC SIMBRIEF"
                onClick={() => {
                  this.syncSimbrief().catch(() => this.databaseMessage.set('SIMBRIEF REQUEST FAILED'));
                }}
                containerStyle="width: 220px;"
              />
            </div>
            <div class="oit-label amber oit-perf-note">{this.databaseMessage}</div>
            <div class="oit-perf-grid">
              <div class="oit-label bigger">ELEVN</div>
              <div class="oit-label bigger oit-green-text">
                {this.runway.derive((r) => (r === null ? '-----' : `${Math.round(r.elevationFt)} FT`))}
              </div>
              <div class="oit-label bigger">SLOPE</div>
              <div class="oit-label bigger oit-green-text">
                {this.runway.derive((r) => (r === null ? '----' : `${r.slopePercent >= 0 ? 'UP' : 'DOWN'} ${Math.abs(r.slopePercent).toFixed(2)}%`))}
              </div>

              <div class="oit-label bigger">TORA</div>
              <div class="oit-label bigger oit-green-text">
                {$derive(([t, u]) => `${formatDistance(t, u)} ${distanceUnitLabel(u)}`, this.usableTora, units)}
              </div>
              <div class="oit-label bigger">ASDA</div>
              <div class="oit-label bigger oit-green-text">
                {$derive(([a, u]) => `${formatDistance(a, u)} ${distanceUnitLabel(u)}`, this.usableAsda, units)}
              </div>

              <div class="oit-label bigger">TODA</div>
              <div class="oit-label bigger amber">{UNAVAILABLE}</div>
              <div class="oit-label bigger">WIDTH</div>
              <div class="oit-label bigger oit-green-text">
                {$derive(
                  ([r, u]) => (r === null ? '----' : `${formatDistance(r.width, u)} ${distanceUnitLabel(u)}`),
                  this.runway,
                  units,
                )}
              </div>
            </div>
            <div class="fr oit-perf-row">
              <div class="oit-label bigger oit-perf-key">ENTRY ANGLE</div>
              <DropdownMenu
                idPrefix={`${this.props.uiService.captOrFo}_OIT_toPerfEntryAngle`}
                selectedIndex={this.entryAngle}
                values={$list(ENTRY_ANGLES)}
                freeTextAllowed={false}
                containerStyle="width: 150px; margin-right: 20px;"
                alignLabels="center"
                numberOfDigitsForInputField={3}
                tmpyActive={$cell(false)}
                hEventConsumer={this.props.container.hEventConsumer}
                interactionMode={this.props.container.interactionMode}
              />
              <div class="oit-label bigger oit-perf-key">T.O SHIFT</div>
              <InputField<number>
                dataEntryFormat={new LengthFormat($cell(0), $cell(6000))}
                mandatory={$cell(false)}
                value={this.takeoffShift}
                containerStyle="width: 190px;"
                alignText="center"
                hEventConsumer={this.props.container.hEventConsumer}
                interactionMode={this.props.container.interactionMode}
                errorHandler={() => {}}
              />
            </div>
            <div class="oit-label oit-perf-note">
              INTERSECTION NAMES ARE NOT IN THE NAVIGATION DATABASE; ENTER THE INTERSECTION AS A T.O SHIFT FROM THE
              RUNWAY THRESHOLD. TODA NEEDS A PUBLISHED CLEARWAY, WHICH THE DATABASE DOES NOT CARRY.
            </div>
          </div>

          <div class="oit-labeled-box-container">
            <span class="oit-labeled-box-label">CONDITIONS</span>
            <div class="oit-perf-grid">
              <div class="oit-label bigger">OAT</div>
              <div class="oit-label bigger oit-green-text">{this.oat.derive((v) => (v === null ? '---' : `${v}°C`))}</div>
              <div class="oit-label bigger">QNH</div>
              <div class="oit-label bigger oit-green-text">
                {$derive(
                  ([q, u]) => (q === null ? '----' : `${formatPressure(q, u)} ${pressureUnitLabel(u)}`),
                  this.qnh,
                  units,
                )}
              </div>

              <div class="oit-label bigger">WIND</div>
              <div class="oit-label bigger oit-green-text">{this.windText}</div>
              <div class="oit-label bigger">PRESS ALT</div>
              <div class="oit-label bigger oit-green-text">{this.pressureAltText}</div>

              <div class="oit-label bigger">HEADWIND</div>
              <div class="oit-label bigger oit-green-text">{this.headwindText}</div>
              <div class="oit-label bigger">CROSSWIND</div>
              <div class="oit-label bigger oit-green-text">{this.crosswindText}</div>
            </div>
            <div class="fr oit-perf-row">
              {this.selector('RWY COND', RUNWAY_CONDITIONS, this.runwayCondition, 'toPerfRwyCond', 130)}
              {this.selector('AIR COND', AIR_COND, this.airCond, 'toPerfAirCond', 200)}
              {this.selector('FLAPS', FLAPS, this.flaps, 'toPerfFlaps', 190)}
            </div>
            <div class="fr oit-perf-row">
              {this.selector('ANTI ICE', ANTI_ICE, this.antiIce, 'toPerfAntiIce', 310)}
              {this.selector('THRUST', THRUST, this.thrust, 'toPerfThrust', 230)}
              <Button label="SYNC WX" onClick={() => this.refreshConditions()} containerStyle="width: 160px;" />
            </div>
            <div class="oit-label oit-perf-note">WEATHER SOURCE: {this.weatherSource}</div>
          </div>

          <div class="oit-labeled-box-container">
            <span class="oit-labeled-box-label">WEIGHTS</span>
            <div class="oit-perf-grid">
              <div class="oit-label bigger">ZFW</div>
              <div class="oit-label bigger oit-green-text">
                {$derive(([t, u]) => `${t} ${weightUnitLabel(u)}`, this.zfwText, units)}
              </div>
              <div class="oit-label bigger">ZFWCG</div>
              <div class="oit-label bigger oit-green-text">{this.zfwCgText.derive((v) => `${v} %`)}</div>

              <div class="oit-label bigger">FOB</div>
              <div class="oit-label bigger oit-green-text">
                {$derive(([t, u]) => `${t} ${weightUnitLabel(u)}`, this.fobText, units)}
              </div>
              <div class="oit-label bigger">TOW</div>
              <div class="oit-label bigger oit-green-text">
                {$derive(([t, u]) => `${t} ${weightUnitLabel(u)}`, this.towText, units)}
              </div>

              <div class="oit-label bigger">TOCG</div>
              <div class="oit-label bigger oit-green-text">{this.toCgText.derive((v) => `${v} %`)}</div>
              <div class="oit-label bigger">STRUCT MTOW</div>
              <div class="oit-label bigger oit-green-text">
                {units.derive((u) => `${formatWeight(A380_MAX_GW_KG, u)} ${weightUnitLabel(u)}`)}
              </div>
            </div>
            <div class="oit-label amber oit-perf-note">{this.weightWarning}</div>
          </div>

          <div class="oit-labeled-box-container">
            <span class="oit-labeled-box-label">RESULTS</span>
            <div class="fr oit-perf-row">
              <Button label="COMPUTE" onClick={() => this.compute()} containerStyle="width: 200px; margin-right: 20px;" />
              <Button
                label="RECHECK WITH AVNCS"
                onClick={() => {}}
                disabled={$cell(true)}
                containerStyle="width: 320px;"
              />
              <div style="flex-grow: 1" />
              <div class="oit-label bigger">RWY</div>
              <div class="oit-label bigger oit-green-text" style="margin-left: 15px;">
                {this.resultRunwayText}
              </div>
            </div>
            <div class="oit-perf-grid">
              <div class="oit-label bigger">V1</div>
              <div class={this.resultClass()}>{this.v1Text}</div>
              <div class="oit-label bigger">FLAPS</div>
              <div class={this.resultClass()}>{this.flapsText}</div>

              <div class="oit-label bigger">VR</div>
              <div class={this.resultClass()}>{this.vrText}</div>
              <div class="oit-label bigger">FLEX</div>
              <div class={this.resultClass()}>{this.flexText}</div>

              <div class="oit-label bigger">V2</div>
              <div class={this.resultClass()}>{this.v2Text}</div>
              <div class="oit-label bigger">THS</div>
              <div class={this.resultClass()}>{this.thsText}</div>

              <div class="oit-label bigger">MTOW (PERF)</div>
              <div class={this.resultClass()}>{this.mtowText}</div>
              <div class="oit-label bigger">STOP MARGIN</div>
              <div class={this.resultClass()}>{this.stopMarginText}</div>

              <div class="oit-label bigger">THR RED</div>
              <div class="oit-label bigger oit-green-text">{this.thrRedText}</div>
              <div class="oit-label bigger">EO ACCEL</div>
              <div class="oit-label bigger oit-green-text">{this.eoAccelText}</div>
            </div>
            <div class="oit-label amber oit-perf-note">{this.resultNote}</div>
          </div>

          <OitFltOpsPerfOptions units={this.units} />
        </div>
      </>
    );
  }

  private resultClass() {
    return {
      'oit-label': true,
      bigger: true,
      cyan: this.fromSimbrief,
      'oit-green-text': this.fromModel,
      amber: $derive(([a, b]) => !a && !b, this.fromSimbrief, this.fromModel),
    };
  }

  private async showModel(why: string): Promise<void> {
    const rwy = this.runway.get();
    const w = readWeights();
    if (rwy === null || w.grossWeightKg === null) {
      this.resultNote.set(`${why}. ${rwy === null ? 'SELECT A RUNWAY' : 'AIRCRAFT WEIGHT NOT YET COMPUTED'} FOR THE OIS MODEL.`);
      return;
    }
    const cal = await loadCalibration();
    const weather = readWeather();
    const oat = this.oat.get() ?? Math.round(weather.oat);
    const qnh = this.qnh.get() ?? weather.qnh;
    const { headwind } = windComponents(rwy.bearingTrue, weather);
    const flaps = ['AUTO', '1+F', '2', '3'][this.flaps.get() ?? 0] as ToConf | 'AUTO';
    const ai = (['OFF', 'ENG', 'ENG+WING'] as AntiIce[])[this.antiIce.get() ?? 0];
    const rating = Vars.read('L:A32NX_A380X_ENGINE_RATING_TO_KN', 'number') || 356.81;
    const angle = ENTRY_ANGLES[this.entryAngle.get() ?? 0] ?? '90';
    const tora = this.usableTora.get() ?? rwy.tora;
    const asda = this.usableAsda.get() ?? rwy.asda;
    const r = takeoff({
      atm: atmos(rwy.elevationFt, qnh, oat),
      oat,
      towT: w.grossWeightKg / 1000,
      cg: w.cgPercentMac ?? 33,
      conf: flaps,
      flex: (this.thrust.get() ?? 0) === 0,
      deratePct: 0,
      ratingKn: rating,
      packs: (this.airCond.get() ?? 0) === 0,
      ai,
      slope: rwy.slopePercent,
      cond: (this.runwayCondition.get() ?? 0) === 1 ? TO_COND.WET : TO_COND.DRY,
      toda: tora,
      asda,
      lineup: LINEUP[angle] ?? LINEUP['90'],
      hw: headwind,
      cal,
    });
    const units = this.units.get();
    this.v1Text.set(`${r.v1} KT`);
    this.vrText.set(`${r.vr} KT`);
    this.v2Text.set(`${r.v2} KT`);
    this.flapsText.set(`CONF ${r.conf}`);
    this.flexText.set(r.flex !== null ? `${r.flex}°C` : 'TOGA');
    this.thsText.set(r.trim >= 0 ? `UP${r.trim.toFixed(1)}` : `DN${Math.abs(r.trim).toFixed(1)}`);
    this.mtowText.set(`${formatWeight(Math.min(r.limitTowT * 1000, A380_MAX_GW_KG), units)} ${weightUnitLabel(units)}`);
    this.stopMarginText.set(`${formatDistance(r.asda - r.asd, units)} ${distanceUnitLabel(units)}`);
    this.fromModel.set(true);
    this.resultNote.set(
      `${why}: OIS MODEL RESULT, NOT AIRBUS-CERTIFIED.${r.ok ? '' : ` TAKE-OFF NOT POSSIBLE: ${r.limits.join(', ')}.`} ${calibrationText(cal)}`,
    );
  }

  private selector(
    label: string,
    values: string[],
    selected: Signal<number | null>,
    id: string,
    width: number,
  ): ViewNode {
    return (
      <div class="oit-perf-field">
        <div class="oit-label bigger oit-perf-field-key">{label}</div>
        <DropdownMenu
          idPrefix={`${this.props.uiService.captOrFo}_OIT_${id}`}
          selectedIndex={selected}
          values={$list(values)}
          freeTextAllowed={false}
          containerStyle={`width: ${width}px;`}
          alignLabels="center"
          numberOfDigitsForInputField={Math.max(...values.map((v) => v.length))}
          tmpyActive={$cell(false)}
          hEventConsumer={this.props.container.hEventConsumer}
          interactionMode={this.props.container.interactionMode}
        />
      </div>
    );
  }
}

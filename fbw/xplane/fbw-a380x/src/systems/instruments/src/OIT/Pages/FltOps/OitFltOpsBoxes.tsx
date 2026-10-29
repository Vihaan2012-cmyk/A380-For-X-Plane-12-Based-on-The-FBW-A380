/** @jsx UI.node */
/** @jsxFrag UI.Group */
//  Copyright (c) 2026 FlyByWire Simulations
//  SPDX-License-Identifier: GPL-3.0

import { Part as View, Parts as UI, State as Signal, PartNode as ViewNode } from 'xplane-js/ui';
import { getSimBriefOfp, ISimbriefData } from '@flybywiresim/fbw-sdk';
import { AbstractOitFltOpsPageProps } from '../../OIT';
import { Button } from '../../../MsfsAvionicsCommon/UiWidgets/Button';
import { buildLoadsheet } from '../../System/Loadsheet';
import { AocMessageStore } from '../../System/AocMessageStore';
import { MaintenanceData } from '../../System/MaintenanceData';
import { exportText, listDocuments } from '../../System/PluginLink';
import { readWeights } from './OitFltOpsPerfCommon';

let lastOfp: ISimbriefData | null = null;

function row(label: string, status: Signal<string>, action: string, onClick: () => void): ViewNode {
  return (
    <div class="fr oit-perf-row" style="align-items: center; margin-bottom: 18px;">
      <div class="oit-label bigger" style="width: 330px;">
        {label}
      </div>
      <div class="oit-label bigger oit-green-text" style="flex-grow: 1;">
        {status}
      </div>
      <Button label={action} onClick={onClick} containerStyle="width: 200px;" />
    </div>
  );
}

function ofpLines(o: ISimbriefData): string[] {
  const fuel = o.fuel;
  const w = o.weights;
  return [
    `FLIGHT ${o.callsign || o.flightNumber}  ${o.origin.icao}-${o.destination.icao}  ALTN ${o.alternate?.icao ?? '----'}`,
    `ROUTE ${o.route}`,
    `CRZ FL${Math.round((o.cruiseAltitude || 0) / 100)}  CI ${o.costIndex}  DIST ${o.distance} NM`,
    `UNITS ${o.units}  RAMP ${fuel.planRamp}  T/O ${fuel.planTakeOff}  TRIP ${fuel.enrouteBurn}  LDG ${fuel.planLanding}`,
    `EZFW ${w.estZeroFuelWeight}  ETOW ${w.estTakeOffWeight}  ELW ${w.estLandingWeight}  PAX ${w.passengerCount}`,
  ];
}

export class OitFltOpsLoadBox extends View<AbstractOitFltOpsPageProps> {
  private readonly ofpStatus = Signal.of(lastOfp ? `${lastOfp.origin.icao}-${lastOfp.destination.icao} LOADED` : 'NOT LOADED');

  private readonly docsStatus = Signal.of('NOT CHECKED');

  private readonly detailRef = UI.ref<HTMLDivElement>();

  private showDetail(lines: string[]): void {
    const d = this.detailRef.peek();
    if (!d) {
      return;
    }
    while (d.firstChild) {
      d.removeChild(d.firstChild);
    }
    UI.mount(<div>{lines.map((l) => <div class="oit-label bigger" style="white-space: pre-wrap;">{l}</div>)}</div>, d);
  }

  private async loadOfp(): Promise<void> {
    this.ofpStatus.set('LOADING...');
    try {
      const o = await getSimBriefOfp();
      lastOfp = o;
      const data = this.props.container.laptopData;
      data.fltNumber.set((o.callsign || o.flightNumber || '').toUpperCase() || null);
      data.fromAirport.set(o.origin.icao);
      data.toAirport.set(o.destination.icao);
      this.ofpStatus.set(`${o.origin.icao}-${o.destination.icao} LOADED`);
      AocMessageStore.instance(this.props.bus).add('OPS', `OFP ${o.origin.icao}-${o.destination.icao}`, 'LOAD BOX', ofpLines(o));
      this.showDetail(ofpLines(o));
    } catch {
      this.ofpStatus.set('LOAD FAILED: CHECK SIMBRIEF ID ON THE EFB');
    }
  }

  private async loadDocuments(): Promise<void> {
    this.docsStatus.set('CHECKING...');
    const docs = await listDocuments();
    if (!docs) {
      this.docsStatus.set('LIBRARY SERVER NOT AVAILABLE');
      return;
    }
    this.docsStatus.set(`${docs.files.length} DOCUMENTS IN OPS LIBRARY`);
    this.showDetail(docs.files.slice(0, 30).map((f) => `${f.folder ? `${f.folder}/` : ''}${f.name}  (${f.type.toUpperCase()})`));
  }

  render(): ViewNode {
    return (
      <div class="oit-page-container framed" style="flex-direction: column;">
        <div class="oit-labeled-box-container">
          <span class="oit-labeled-box-label">LOADABLE DATA</span>
          {row('SIMBRIEF OFP', this.ofpStatus, 'LOAD', () => this.loadOfp())}
          {row('OPS LIBRARY DOCUMENTS', this.docsStatus, 'CHECK', () => this.loadDocuments())}
          <div class="oit-label oit-perf-note">
            THE OFP FILLS FLT NUMBER, FROM AND TO ON THE STATUS PAGE AND IS FILED IN THE AOC OPS FOLDER. DOCUMENTS ARE READ FROM THE AIRCRAFT DOCUMENTS FOLDER.
          </div>
        </div>
        <div ref={this.detailRef} style="flex-grow: 1; overflow-y: auto; margin-top: 10px;" />
      </div>
    );
  }
}

interface ExportItem {
  label: string;
  file: () => string;
  text: () => string[] | null;
}

export class OitFltOpsExportBox extends View<AbstractOitFltOpsPageProps> {
  private readonly items: ExportItem[] = [
    { label: 'LOADSHEET PRELIM', file: () => 'LOADSHEET PRELIM', text: () => buildLoadsheet('PRELIM')?.lines ?? null },
    { label: 'LOADSHEET FINAL', file: () => 'LOADSHEET FINAL', text: () => buildLoadsheet('FINAL')?.lines ?? null },
    {
      label: 'AOC MESSAGES',
      file: () => 'AOC MESSAGES',
      text: () => {
        const all = AocMessageStore.instance(this.props.bus).messages.all();
        return all.length ? all.flatMap((m) => [`${m.zulu}Z ${m.folder} ${m.title} (${m.from})`, ...m.lines, '']) : null;
      },
    },
    {
      label: 'TECHNICAL LOG',
      file: () => 'TECHNICAL LOG',
      text: () => {
        const md = MaintenanceData.instance();
        const ev = md.events.all();
        return ev.length ? ev.map((e) => `${MaintenanceData.formatZulu(e.zuluS)}Z  ${MaintenanceData.kindLabel(e.kind).padEnd(12)} ATA ${md.ataFor(e)}  ${md.messageFor(e)}`) : null;
      },
    },
    {
      label: 'FLIGHT SUMMARY',
      file: () => 'FLIGHT SUMMARY',
      text: () => {
        const d = this.props.container.laptopData;
        const w = readWeights();
        const kg = (v: number | null) => (v === null ? '----' : `${(v / 1000).toFixed(1)} T`);
        return [
          `FLT ${d.fltNumber.get() ?? '----'}  ${d.fromAirport.get() ?? '----'}-${d.toAirport.get() ?? '----'}`,
          `GW ${kg(w.grossWeightKg)}  FOB ${kg(w.fobKg)}  ZFW ${kg(w.zeroFuelWeightKg)}  ZFWCG ${w.zeroFuelCgPercentMac.toFixed(1)} %`,
          ...(lastOfp ? ['', ...ofpLines(lastOfp)] : []),
        ];
      },
    },
  ];

  private readonly statuses = this.items.map(() => Signal.of('READY'));

  private async export(i: number): Promise<void> {
    const item = this.items[i];
    const lines = item.text();
    if (!lines) {
      this.statuses[i].set('NOTHING TO EXPORT');
      return;
    }
    this.statuses[i].set('EXPORTING...');
    const zulu = new Date();
    const stamp = `${zulu.getUTCFullYear()}-${String(zulu.getUTCMonth() + 1).padStart(2, '0')}-${String(zulu.getUTCDate()).padStart(2, '0')} ${String(zulu.getUTCHours()).padStart(2, '0')}${String(zulu.getUTCMinutes()).padStart(2, '0')}Z`;
    const r = await exportText(`${item.file()} ${stamp}`, `${item.label}\n${stamp}\n\n${lines.join('\n')}\n`);
    this.statuses[i].set(r?.ok ? `SAVED ${r.path ?? ''}`.toUpperCase() : 'EXPORT FAILED');
  }

  render(): ViewNode {
    return (
      <div class="oit-page-container framed" style="flex-direction: column;">
        <div class="oit-labeled-box-container">
          <span class="oit-labeled-box-label">EXPORTABLE DATA</span>
          {this.items.map((it, i) => row(it.label, this.statuses[i], 'EXPORT', () => this.export(i)))}
          <div class="oit-label oit-perf-note">EXPORTS ARE SAVED IN THE AIRCRAFT DOCUMENTS/EXPORTS FOLDER AND APPEAR ON THE EFB UNDER MY DOCUMENTS.</div>
        </div>
      </div>
    );
  }
}

/** @jsx UI.node */
/** @jsxFrag UI.Group */
//  Copyright (c) 2026 FlyByWire Simulations
//  SPDX-License-Identifier: GPL-3.0

import { Part as View, Parts as UI, State as Signal, Handle as Bond, PartNode as ViewNode } from 'xplane-js/ui';
import { AbstractOitFltOpsPageProps } from '../../OIT';
import { Button } from '../../../MsfsAvionicsCommon/UiWidgets/Button';
import { OisDocument, OisMelItem, listDocuments, melChapter, readDocument } from '../../System/PluginLink';

type Shelf = 'DOCUMENTS' | 'MEL';

const ATA: [string, string][] = [
  ['21', 'AIR CONDITIONING'],
  ['22', 'AUTO FLIGHT'],
  ['23', 'COMMUNICATIONS'],
  ['24', 'ELECTRICAL POWER'],
  ['25', 'EQUIPMENT/FURNISHINGS'],
  ['26', 'FIRE PROTECTION'],
  ['27', 'FLIGHT CONTROLS'],
  ['28', 'FUEL'],
  ['29', 'HYDRAULIC POWER'],
  ['30', 'ICE AND RAIN PROTECTION'],
  ['31', 'INDICATING/RECORDING'],
  ['32', 'LANDING GEAR'],
  ['33', 'LIGHTS'],
  ['34', 'NAVIGATION'],
  ['35', 'OXYGEN'],
  ['36', 'PNEUMATIC'],
  ['38', 'WATER/WASTE'],
  ['46', 'INFORMATION SYSTEMS'],
  ['49', 'AIRBORNE AUXILIARY POWER'],
  ['52', 'DOORS'],
  ['70', 'POWER PLANT'],
];

export class OitFltOpsOpsLibrary extends View<AbstractOitFltOpsPageProps> {
  private readonly subs = [] as Bond[];

  private readonly shelf = Signal.of<Shelf>('DOCUMENTS');

  private readonly listRef = UI.ref<HTMLDivElement>();

  private readonly viewRef = UI.ref<HTMLDivElement>();

  private readonly title = Signal.of('');

  private clear(el: HTMLElement | null | undefined): HTMLElement | null {
    if (!el) {
      return null;
    }
    while (el.firstChild) {
      el.removeChild(el.firstChild);
    }
    return el;
  }

  private entry(label: string, sub: string, onClick: () => void): ViewNode {
    return (
      <div class="oit-avncs-navigator-file" style="cursor: pointer; padding: 8px 4px; border-bottom: 1px solid #333;" onClick={onClick}>
        <div class="oit-label bigger">{label}</div>
        {sub ? <div class="oit-label" style="color: #a0a0a0;">{sub}</div> : <div />}
      </div>
    );
  }

  private showText(lines: ViewNode[]): void {
    const view = this.clear(this.viewRef.peek());
    if (!view) {
      return;
    }
    view.scrollTop = 0;
    UI.mount(<div>{lines}</div>, view);
  }

  private async loadShelf(): Promise<void> {
    const list = this.clear(this.listRef.peek());
    if (!list) {
      return;
    }
    this.clear(this.viewRef.peek());
    this.title.set('');
    if (this.shelf.get() === 'MEL') {
      UI.mount(<div>{ATA.map(([n, name]) => this.entry(`ATA ${n}`, name, () => this.openChapter(n, name)))}</div>, list);
      return;
    }
    UI.mount(<div class="oit-label bigger">LOADING...</div>, list);
    const docs = await listDocuments();
    this.clear(list);
    if (!docs || docs.files.length === 0) {
      UI.mount(
        <div class="oit-label bigger">{docs ? 'NO DOCUMENTS. PUT .MD, .TXT, PDF OR PICTURE FILES IN THE AIRCRAFT DOCUMENTS FOLDER.' : 'LIBRARY SERVER NOT AVAILABLE'}</div>,
        list,
      );
      return;
    }
    UI.mount(<div>{docs.files.map((d) => this.entry(d.name.toUpperCase(), `${d.folder ? `${d.folder.toUpperCase()} - ` : ''}${d.type.toUpperCase()}`, () => this.openDocument(d)))}</div>, list);
  }

  private async openDocument(d: OisDocument): Promise<void> {
    this.title.set(d.name.toUpperCase());
    if (d.type !== 'text') {
      this.showText([<div class="oit-label bigger">{`${d.type.toUpperCase()} FILE: OPEN IT ON THE EFB, DOCUMENTS > MY DOCUMENTS.`}</div>]);
      return;
    }
    this.showText([<div class="oit-label bigger">LOADING...</div>]);
    const doc = await readDocument(d.path);
    if (!doc) {
      this.showText([<div class="oit-label bigger amber">DOCUMENT NOT AVAILABLE</div>]);
      return;
    }
    this.showText(
      doc.text.split(/\r?\n/).map((line) => {
        const h = /^(#{1,3})\s+(.*)$/.exec(line);
        if (h) {
          return <div class={`oit-label ${h[1].length === 1 ? 'bigger cyan' : 'bigger'}`} style="margin-top: 14px; font-weight: bold;">{h[2].toUpperCase()}</div>;
        }
        const text = line.replace(/\*\*|__|`/g, '').replace(/^\s*[-*]\s+\[[ xX]\]\s*/, '□ ').replace(/^\s*[-*]\s+/, '• ');
        return <div class="oit-label" style="white-space: pre-wrap; min-height: 22px;">{text}</div>;
      }),
    );
  }

  private async openChapter(ata: string, name: string): Promise<void> {
    this.title.set(`MEL ATA ${ata} ${name}`);
    this.showText([<div class="oit-label bigger">LOADING...</div>]);
    const mel = await melChapter(ata);
    if (!mel) {
      this.showText([<div class="oit-label bigger amber">MEL NOT AVAILABLE</div>]);
      return;
    }
    if (mel.items.length === 0) {
      this.showText([
        <div class="oit-label bigger">NO MEL ITEM IN THIS CHAPTER.</div>,
        <div class="oit-label">THE OPERATOR MEL IS NOT SHIPPED. PARSE YOUR OWN COPY WITH TOOLS/PARSE_MEL.PY INTO OUTPUT/PREFERENCES/FBW_A380X_MEL.JSON.</div>,
      ]);
      return;
    }
    this.showText(mel.items.flatMap((it: OisMelItem) => [
      <div class="oit-label bigger cyan" style="margin-top: 14px;">{`${it.id}  ${it.title.toUpperCase()}`}</div>,
      <div class="oit-label bigger">
        {`CAT ${it.category ?? '-'}   INSTALLED ${it.installed ?? '-'}   REQUIRED ${it.required ?? '-'}   ${it.placard ? 'PLACARD' : ''}   ${it.opsProcedureRequired ? '(O)' : ''} ${it.maintenanceProcedureRequired ? '(M)' : ''}`}
      </div>,
      <div class="oit-label" style="white-space: pre-wrap;">{it.conditions}</div>,
      it.opsProcedure ? <div class="oit-label" style="white-space: pre-wrap; color: #a0a0a0;">{it.opsProcedure}</div> : <div />,
    ]));
  }

  private scroll(by: number): void {
    const v = this.viewRef.peek();
    if (v) {
      v.scrollTop += by;
    }
  }

  public afterMount(node: ViewNode): void {
    super.afterMount(node);
    this.subs.push(this.shelf.watch(() => this.loadShelf()));
    this.loadShelf();
  }

  public destroy(): void {
    for (const s of this.subs) {
      s.destroy();
    }
    super.destroy();
  }

  render(): ViewNode {
    return (
      <div class="oit-page-container framed" style="flex-direction: column;">
        <div class="fr ass" style="margin-bottom: 15px; align-items: center;">
          <Button label="DOCUMENTS" onClick={() => this.shelf.set('DOCUMENTS')} selected={this.shelf.derive((s) => s === 'DOCUMENTS')} containerStyle="width: 220px; margin-right: 20px;" />
          <Button label="MEL" onClick={() => this.shelf.set('MEL')} selected={this.shelf.derive((s) => s === 'MEL')} containerStyle="width: 220px; margin-right: 20px;" />
          <div class="oit-label bigger cyan" style="flex-grow: 1; text-align: center;">
            {this.title}
          </div>
          <Button label="PAGE UP" onClick={() => this.scroll(-600)} containerStyle="width: 170px; margin-right: 10px;" />
          <Button label="PAGE DOWN" onClick={() => this.scroll(600)} containerStyle="width: 170px;" />
        </div>
        <div class="fr" style="flex-grow: 1; min-height: 0;">
          <div ref={this.listRef} style="width: 420px; overflow-y: auto; border-right: 2px solid #555; padding-right: 10px;" />
          <div ref={this.viewRef} style="flex-grow: 1; overflow-y: auto; padding-left: 20px;" />
        </div>
      </div>
    );
  }
}

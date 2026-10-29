/** @jsx UI.node */
/** @jsxFrag UI.Group */
//  Copyright (c) 2026 FlyByWire Simulations
//  SPDX-License-Identifier: GPL-3.0

import { Part as View, Parts as UI, State as Signal, PartNode as ViewNode } from 'xplane-js/ui';
import { AbstractOitFltOpsPageProps } from '../../OIT';
import { Ofp, OfpSection, fetchOfp, lastOfp } from '../../System/SimbriefOfp';

export function oefbElement(tag: string, cls: string, text?: string): HTMLElement {
  const e = document.createElement(tag);
  if (cls) {
    e.className = cls;
  }
  if (text !== undefined) {
    e.textContent = text;
  }
  return e;
}

export class OitFltOpsFltFolder extends View<AbstractOitFltOpsPageProps> {
  private readonly treeRef = UI.ref<HTMLDivElement>();

  private readonly viewRef = UI.ref<HTMLDivElement>();

  private readonly summary = Signal.of('');

  private readonly treeLabel = Signal.of('HIDE TREE');

  private treeShown = true;

  private selected: HTMLElement | null = null;

  private toggleTree(): void {
    this.treeShown = !this.treeShown;
    this.treeLabel.set(this.treeShown ? 'HIDE TREE' : 'SHOW TREE');
    const tree = this.treeRef.peek();
    if (tree) {
      tree.style.display = this.treeShown ? '' : 'none';
    }
  }

  private message(text: string, cls = 'oefb-msg'): void {
    const view = this.viewRef.peek();
    if (view) {
      view.replaceChildren(oefbElement('div', cls, text));
    }
  }

  private show(s: OfpSection, item: HTMLElement): void {
    const view = this.viewRef.peek();
    if (!view) {
      return;
    }
    this.selected?.classList.remove('sel');
    item.classList.add('sel');
    this.selected = item;
    view.scrollTop = 0;
    view.scrollLeft = 0;
    if (s.image) {
      const img = oefbElement('img', 'oefb-picture') as HTMLImageElement;
      img.src = s.image;
      img.alt = s.title;
      view.replaceChildren(oefbElement('div', 'oefb-view-title', s.title), img);
    } else {
      view.replaceChildren(oefbElement('div', 'oefb-mono', s.text ?? ''));
    }
  }

  private renderOfp(ofp: Ofp): void {
    const tree = this.treeRef.peek();
    if (!tree) {
      return;
    }
    this.summary.set(
      [ofp.flight, ofp.origin && ofp.destination ? `${ofp.origin} - ${ofp.destination}` : '', ofp.alternate ? `ALTN ${ofp.alternate}` : '', ofp.generated]
        .filter((x) => x)
        .join('   '),
    );
    tree.replaceChildren();
    let group = '';
    let first: [OfpSection, HTMLElement] | null = null;
    for (const s of ofp.sections) {
      if (s.group !== group) {
        group = s.group;
        tree.append(oefbElement('div', 'oefb-group', group));
      }
      if (!s.text && !s.image) {
        tree.append(oefbElement('div', 'oefb-heading', s.title));
        continue;
      }
      const item = oefbElement('div', `oefb-item${s.level > 0 ? ' l1' : ''}`, s.title);
      item.addEventListener('click', () => this.show(s, item));
      tree.append(item);
      if (!first) {
        first = [s, item];
      }
    }
    if (first) {
      this.show(first[0], first[1]);
    } else {
      this.message('THE OFP HAS NO SECTIONS');
    }
  }

  private async fetch(): Promise<void> {
    this.message('FETCHING THE OFP FROM SIMBRIEF...');
    try {
      this.renderOfp(await fetchOfp());
    } catch (e) {
      this.treeRef.peek()?.replaceChildren();
      this.summary.set('');
      this.message(e instanceof Error ? e.message : String(e), 'oefb-msg warn');
    }
  }

  public afterMount(node: ViewNode): void {
    super.afterMount(node);
    const ofp = lastOfp();
    if (ofp) {
      this.renderOfp(ofp);
    } else {
      this.fetch();
    }
  }

  render(): ViewNode {
    return (
      <div class="oit-page-container oefb">
        <div class="oefb-bar">
          <div class="oefb-title">FLT FOLDER</div>
          <div class="oefb-sub">{this.summary}</div>
          <div class="oefb-spacer" />
          <div class="oefb-btn" onClick={() => this.toggleTree()}>
            {this.treeLabel}
          </div>
          <div class="oefb-btn primary" onClick={() => this.fetch()}>
            FETCH OFP
          </div>
        </div>
        <div class="oefb-body">
          <div ref={this.treeRef} class="oefb-tree" />
          <div ref={this.viewRef} class="oefb-view" />
        </div>
      </div>
    );
  }
}

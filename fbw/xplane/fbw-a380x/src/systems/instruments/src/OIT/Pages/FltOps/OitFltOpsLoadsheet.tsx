/** @jsx UI.node */
/** @jsxFrag UI.Group */
//  Copyright (c) 2026 FlyByWire Simulations
//  SPDX-License-Identifier: GPL-3.0

import { TickTopics as ClockTopics, Part as View, Parts as UI, State as Signal, Handle as Bond, PartNode as ViewNode } from 'xplane-js/ui';
import { AbstractOitFltOpsPageProps } from '../../OIT';
import { Button } from '../../../MsfsAvionicsCommon/UiWidgets/Button';
import { buildLoadsheet } from '../../System/Loadsheet';
import { AocMessageStore } from '../../System/AocMessageStore';

interface OitFltOpsLoadsheetPageProps extends AbstractOitFltOpsPageProps {}

export class OitFltOpsLoadsheet extends View<OitFltOpsLoadsheetPageProps> {
  private readonly subs = [] as Bond[];

  private readonly kind = Signal.of<'PRELIM' | 'FINAL'>('PRELIM');

  private readonly contentRef = UI.ref<HTMLDivElement>();

  private readonly store = AocMessageStore.instance(this.props.bus);

  private render_(): void {
    const root = this.contentRef.peek();
    if (!root) {
      return;
    }
    while (root.firstChild) {
      root.removeChild(root.firstChild);
    }

    const result = buildLoadsheet(this.kind.get());

    if (!result) {
      UI.mount(
        <div class="oit-label bigger">
          NO LOADSHEET AVAILABLE
          <br />
          {this.kind.get() === 'FINAL'
            ? 'MAIN DOOR MUST BE CLOSED FOR A FINAL LOADSHEET'
            : 'AIRCRAFT WEIGHT NOT YET COMPUTED'}
        </div>,
        root,
      );
      return;
    }

    UI.mount(
      <div style="white-space: pre-wrap; font-family: monospace;">
        <div class="oit-label bigger cyan">{result.title}</div>
        {result.lines.map((l) => (
          <div class="oit-label bigger">{l}</div>
        ))}
      </div>,
      root,
    );
  }

  public afterMount(node: ViewNode): void {
    super.afterMount(node);

    const sub = this.props.bus.receiver<ClockTopics>();
    this.subs.push(
      sub
        .topic('realTime')
        .sampled(0.5)
        .listen(() => this.render_()),
      this.kind.watch(() => this.render_()),
    );

    this.render_();
  }

  public destroy(): void {
    for (const s of this.subs) {
      s.destroy();
    }
    super.destroy();
  }

  render(): ViewNode {
    return (
      <>
        <div class="oit-page-container framed" style="flex-direction: column;">
          <div class="fr ass" style="margin-bottom: 15px;">
            <Button
              label="PRELIM"
              onClick={() => this.kind.set('PRELIM')}
              selected={this.kind.derive((k) => k === 'PRELIM')}
              containerStyle="width: 200px; margin-right: 20px;"
            />
            <Button
              label="FINAL"
              onClick={() => this.kind.set('FINAL')}
              selected={this.kind.derive((k) => k === 'FINAL')}
              containerStyle="width: 200px; margin-right: 20px;"
            />
            <div style="flex-grow: 1" />
            <Button
              label="SEND TO AOC"
              onClick={() => {
                const result = buildLoadsheet(this.kind.get());
                if (result) {
                  this.store.add('LOADSHEET', result.title, 'LOADSHEET', result.lines);
                }
              }}
              containerStyle="width: 220px;"
            />
          </div>
          <div ref={this.contentRef} style="overflow-y: auto; flex-grow: 1;" />
        </div>
      </>
    );
  }
}

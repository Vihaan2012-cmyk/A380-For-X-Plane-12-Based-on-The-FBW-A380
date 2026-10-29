//  Copyright (c) 2025 FlyByWire Simulations
//  SPDX-License-Identifier: GPL-3.0

import {
  View,
  UI,
  Signal,
  Readable,
  Readables,
  Bond,
  ViewNode,
} from 'xplane-js/ui';
import { PageSelectorDropdownMenu } from '../../../MsfsAvionicsCommon/UiWidgets/PageSelectorDropdownMenu';
import { Button } from '../../../MsfsAvionicsCommon/UiWidgets/Button';
import { IconButton } from '../../../MsfsAvionicsCommon/UiWidgets/IconButton';
import { OitUiService } from '../../OitUiService';
import { OisDomain } from '../../OIT';
import { exportText } from '../../System/PluginLink';

interface OitFltOpsHeaderProps {
  readonly uiService: OitUiService;
  readonly avncsOrFltOps: Readable<OisDomain>;
}

/*
 * Complete header for the OIS system, both AVNCS and FLT OPS.
 */
export abstract class OitFltOpsHeader extends View<OitFltOpsHeaderProps> {
  // Make sure to collect all subscriptions here, otherwise page navigation doesn't work.
  protected readonly subs = [] as Bond[];

  private readonly oitHeading = this.props.uiService.activeUri.derive((uri) => heading[uri.uri] ?? uri.uri.toUpperCase());

  private readonly rootRef = UI.ref<HTMLDivElement>();

  private pageElement(): HTMLElement | null {
    let node: HTMLElement | null = this.rootRef.peek();
    while (node) {
      const page = node.querySelector<HTMLElement>('.oit-page-container');
      if (page) {
        return page;
      }
      node = node.parentElement;
    }
    return null;
  }

  private savePage(prefix: string): void {
    const page = this.pageElement();
    if (!page) {
      return;
    }
    const title = this.oitHeading.get();
    const now = new Date();
    const stamp = `${now.getUTCFullYear()}-${String(now.getUTCMonth() + 1).padStart(2, '0')}-${String(now.getUTCDate()).padStart(2, '0')} ${String(now.getUTCHours()).padStart(2, '0')}${String(now.getUTCMinutes()).padStart(2, '0')}Z`;
    const text = page.innerText
      .split(/\n+/)
      .map((l) => l.trim())
      .filter((l) => l.length > 0)
      .join('\n');
    exportText(`${prefix} ${title} ${stamp}`, `${title}\n${stamp}\n\n${text}\n`);
  }

  private toggleSwitchingBar(): void {
    const header = this.rootRef.peek();
    let node: HTMLElement | null = header?.parentElement ?? null;
    while (node) {
      const bars = Array.from(node.querySelectorAll<HTMLElement>('.oit-header-row')).filter((e) => e !== header);
      if (bars.length > 0) {
        const bar = bars[bars.length - 1];
        bar.style.display = bar.style.display === 'none' ? '' : 'none';
        return;
      }
      node = node.parentElement;
    }
  }

  public afterRender(node: ViewNode): void {
    super.afterRender(node);

    this.subs.push(this.oitHeading);
  }

  public destroy(): void {
    // Destroy all subscriptions to remove all references to this instance.
    for (const s of this.subs) {
      s.destroy();
    }

    super.destroy();
  }

  render(): ViewNode {
    return (
      <div class="oit-header-row" ref={this.rootRef}>
        <PageSelectorDropdownMenu
          isActive={Readables.toReadable(false, true)}
          label="MENU"
          menuItems={[
            { label: 'FLT OPS MENU', action: () => this.props.uiService.navigateTo('flt-ops'), separatorBelow: true },
            {
              label: 'FLT FOLDER',
              action: () => this.props.uiService.navigateTo('flt-ops/flt-folder'),
            },
            {
              label: 'TERML CHART',
              action: () => this.props.uiService.navigateTo('flt-ops/charts'),
              separatorBelow: true,
            },
            {
              label: 'OPS LIBRARY',
              action: () => this.props.uiService.navigateTo('flt-ops/ops-library'),
              separatorBelow: true,
            },
            { label: 'T.O PERF', action: () => this.props.uiService.navigateTo('flt-ops/to-perf') },
            {
              label: 'LOADSHEET',
              action: () => this.props.uiService.navigateTo('flt-ops/loadsheet'),
              disabled: false,
            },
            { label: 'LDG PERF', action: () => this.props.uiService.navigateTo('flt-ops/ldg-perf') },
            {
              label: 'IN-FLT PERF',
              action: () => this.props.uiService.navigateTo('flt-ops/in-flt-perf'),
              separatorBelow: true,
            },
            { label: 'AOC', action: () => this.props.uiService.navigateTo('flt-ops/aoc'), separatorBelow: true },
            { label: 'FLT OPS STS', action: () => this.props.uiService.navigateTo('flt-ops/sts') },
            { label: 'LOAD BOX', action: () => this.props.uiService.navigateTo('flt-ops/load-box') },
            {
              label: 'EXPORT BOX',
              action: () => this.props.uiService.navigateTo('flt-ops/export-box'),
              separatorBelow: true,
            },
            {
              label: 'EXIT SESSION',
              action: () => {
                this.props.uiService.navigateTo('flt-ops');
                this.props.uiService.fltOpsLoginScreenVisible.set(true);
              },
              disabled: false,
            },
          ]}
          idPrefix={`${this.props.uiService.captOrFo}_OIT_menu_menu`}
          dropdownMenuStyle="width: 300px;"
        />
        <div class="oit-heading">{this.oitHeading}</div>
        <div style="flex-grow: 1" />
        <PageSelectorDropdownMenu
          isActive={Readables.toReadable(false, true)}
          label="FUNCTIONS"
          menuItems={[
            {
              label: 'HOME',
              action: () => {
                if (this.props.avncsOrFltOps.get() === 'flt-ops') {
                  this.props.uiService.navigateTo('flt-ops');
                }
              },
            },
            {
              label: 'PREVIOUS',
              action: () => {
                if (this.props.uiService.canGoBack()) {
                  this.props.uiService.navigateTo('back');
                }
              },
            },
            {
              label: 'NEXT',
              action: () => {
                if (this.props.uiService.canGoForward()) {
                  this.props.uiService.navigateTo('forward');
                }
              },
              separatorBelow: true,
            },
            { label: 'PRINT', action: () => this.savePage('PRINT') },
            { label: 'STORE', action: () => this.savePage('STORED') },
            {
              label: 'UPDATE',
              action: () => this.props.uiService.activeUri.set(this.props.uiService.parseUri(this.props.uiService.activeUri.get().uri)),
              separatorBelow: true,
            },
            { label: 'UNDO', action: () => {}, disabled: true },
            { label: 'REDO', action: () => {}, disabled: true, separatorBelow: true },
            {
              label: 'HELP',
              action: () => {
                if (this.props.avncsOrFltOps.get() === 'flt-ops') {
                  this.props.uiService.navigateTo('flt-ops/ops-library');
                }
              },
              separatorBelow: true,
            },
            { label: 'HIDE SWITCHING BAR', action: () => this.toggleSwitchingBar(), separatorBelow: true },
            {
              label: 'CLOSE APPLICATION',
              action: () => {
                if (this.props.avncsOrFltOps.get() === 'flt-ops') {
                  this.props.uiService.navigateTo('flt-ops');
                }
              },
              separatorBelow: true,
            },
          ]}
          idPrefix={`${this.props.uiService.captOrFo}_OIT_menu_functions`}
          dropdownMenuStyle="width: 300px;"
        />
        <div class="oit-msg-header">0 MSG</div>
        <div class="oit-msg-box"></div>
        <IconButton
          icon={'flat-single-down'}
          containerStyle="width: 60px; height: 60px;"
          disabled={Signal.of(true)}
        />
        <Button
          label={'CLEAR'}
          onClick={() => {}}
          buttonStyle="font-size: 28px; height: 60px;"
          disabled={Signal.of(true)}
        />
      </div>
    );
  }
}

const heading: Record<string, string> = {
  'flt-ops': 'FLT OPS MENU',
  'flt-ops/flt-folder': 'FLT FOLDER',
  'flt-ops/charts': 'CHARTS',
  'flt-ops/sts': 'STATUS PAGE',
  'flt-ops/to-perf': 'T.O PERF',
  'flt-ops/ldg-perf': 'LDG PERF',
  'flt-ops/load-box': 'LOAD BOX',
  'flt-ops/export-box': 'EXPORT BOX',
  'flt-ops/aoc': 'AOC',
  'flt-ops/loadsheet': 'LOADSHEET',
  'flt-ops/in-flt-perf': 'IN-FLT PERF',
  'flt-ops/ops-library': 'OPS LIBRARY',
};

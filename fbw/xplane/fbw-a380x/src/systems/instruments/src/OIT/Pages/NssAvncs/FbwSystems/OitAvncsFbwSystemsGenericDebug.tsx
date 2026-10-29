//  Copyright (c) 2025 FlyByWire Simulations
//  SPDX-License-Identifier: GPL-3.0

import { UI, Readable, ViewNode, BusSignal, Signal, SignalList } from 'xplane-js/ui';
import { AbstractOitAvncsPageProps } from '../../../OIT';
import { DestroyableComponent } from '@flybywiresim/msfs-avionics-common';
import {
  OisDebugDataControlEvents,
  OisDebugDataEvents,
} from '../../../../MsfsAvionicsCommon/providers/OisDebugDataPublisher';
import { DropdownMenu } from '../../../../MsfsAvionicsCommon/UiWidgets/DropdownMenu';

interface OitAvncsFbwSystemsGenericDebugProps extends AbstractOitAvncsPageProps {
  /** Title which should be displayed at the top of the page */
  title: string;
  /** Event Bus event which is used to switch debug data on/off */
  controlEventName: keyof OisDebugDataControlEvents;
  /** Event Bus event which is used to receive debug data */
  dataEventName: keyof OisDebugDataEvents;
}

export abstract class OitAvncsFbwSystemsGenericDebug extends DestroyableComponent<OitAvncsFbwSystemsGenericDebugProps> {
  protected readonly sci = this.props.container.ansu.sci;

  protected readonly sub = this.props.bus.inbox<OisDebugDataEvents & OisDebugDataControlEvents>();
  protected readonly pub = this.props.bus.outbox<OisDebugDataControlEvents>();

  protected readonly controlEventStatus = BusSignal.from(this.sub.topic(this.props.controlEventName), false);
  protected readonly debugDataTable = BusSignal.from(this.sub.topic(this.props.dataEventName), []);

  protected readonly messageContainerRef = UI.ref<HTMLDivElement>();

  protected readonly debugDataControlDropdownIndex = Signal.of<number | null>(0);

  protected readonly labelSubjects: Signal<string>[] = [];
  protected readonly valueSubjects: Signal<string>[] = [];

  public afterRender(node: ViewNode): void {
    super.afterRender(node);

    this.subscriptions.push(
      this.controlEventStatus.watch((status) => {
        this.debugDataControlDropdownIndex.set(status ? 1 : 0);
      }, true),
      this.debugDataControlDropdownIndex.watch((index) => {
        this.pub.post(this.props.controlEventName, index === 1, true);
      }),
      this.debugDataTable.watch((data) => {
        // Handle the debug data table updates here
        if (this.labelSubjects.length === 0 && data.length > 0) {
          // Initial population
          for (const row of data) {
            this.labelSubjects.push(Signal.of(row.label));
            this.valueSubjects.push(Signal.of(row.value));

            UI.mount(
              <OitAvncsFbwSystemsDebugTableLine
                label={this.labelSubjects[this.labelSubjects.length - 1]}
                value={this.valueSubjects[this.valueSubjects.length - 1]}
              />,
              this.messageContainerRef.target,
            );
          }
        } else {
          // Update existing subjects
          for (let i = 0; i < data.length; i++) {
            this.labelSubjects[i].set(data[i].label);
            this.valueSubjects[i].set(data[i].value);
          }
        }
      }, true),
    );
  }

  destroy(): void {
    super.destroy();
  }

  render(): ViewNode {
    return (
      <>
        <div class="oit-ccom-headline">{this.props.title}</div>
        <div class="oit-ccom-headline">
          <div style="flex-grow: 1" />
          <div class="fr aic">
            <span style="margin-right: 15px;">Transmission Status</span>
            <DropdownMenu
              values={SignalList.of(['OFF', 'ON'])}
              selectedIndex={this.debugDataControlDropdownIndex}
              idPrefix={`${this.props.container.uiService.captOrFo}_OIT_${this.props.controlEventName}_control`}
              freeTextAllowed={false}
              containerStyle="width: 175px; margin-right: 15px;"
              numberOfDigitsForInputField={5}
              alignLabels="center"
              hEventConsumer={this.props.container.hEventConsumer}
              interactionMode={this.props.container.interactionMode}
            />
          </div>
        </div>
        <div class="oit-ccom-inbox-msg-table">
          <div class="fr ass">
            <div class="oit-ccom-inbox-msg-table-header oit-table-ib f1">Label</div>
            <div class="oit-ccom-inbox-msg-table-header oit-table-ib f1">Value</div>
          </div>
          <div ref={this.messageContainerRef}></div>
        </div>
        <div style="flex-grow: 1" />
      </>
    );
  }
}

interface OitAvncsFbwSystemsDebugTableLineProps {
  readonly label: Readable<string>;
  readonly value: Readable<string>;
}

export class OitAvncsFbwSystemsDebugTableLine extends DestroyableComponent<OitAvncsFbwSystemsDebugTableLineProps> {
  public afterRender(node: ViewNode): void {
    super.afterRender(node);
  }

  render(): ViewNode {
    return (
      <div class="fr ass">
        <div class={{ 'oit-ccom-inbox-msg-table-line': true, tc: true, f1: true }}>{this.props.label}</div>
        <div
          class={{
            'oit-ccom-inbox-msg-table-line': true,
            'oit-green-text': true,
            f1: true,
          }}
        >
          {this.props.value}
        </div>
      </div>
    );
  }
}

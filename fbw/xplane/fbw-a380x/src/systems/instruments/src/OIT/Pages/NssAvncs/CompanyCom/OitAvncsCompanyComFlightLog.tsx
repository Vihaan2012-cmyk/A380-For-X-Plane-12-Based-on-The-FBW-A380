//  Copyright (c) 2025 FlyByWire Simulations
//  SPDX-License-Identifier: GPL-3.0

import { ClockTopics, UI, Derived, Signal, ViewNode } from 'xplane-js/ui';
import { AbstractOitAvncsPageProps } from '../../../OIT';
import { RadioButtonGroup } from '../../../../MsfsAvionicsCommon/UiWidgets/RadioButtonGroup';
import { DestroyableComponent } from '@flybywiresim/msfs-avionics-common';
import { FmsData, NXDataStore, NXUnits } from '@flybywiresim/fbw-sdk';
import { OitSimvars } from '../../../OitSimvarPublisher';
import { AnsuOps } from '../../../System/AnsuOps';

interface OitAvncsCompanyComFlightLogProps extends AbstractOitAvncsPageProps {}

export abstract class OitAvncsCompanyComFlightLog extends DestroyableComponent<OitAvncsCompanyComFlightLogProps> {
  private readonly sci = this.props.container.ansu.sci;

  private readonly sub = this.props.bus.inbox<FmsData & ClockTopics & OitSimvars>();

  private readonly flightSelectionSelectedIndex = Signal.of<number | null>(0);

  // FIXME maybe this should come from the ATC/ATSU when it's integrated
  private readonly fltNumberText = this.sci.fltNumber.derive((number) => (number ? number : '----------'));

  private readonly fltOriginText = this.sci.fltOrigin.derive((origin) => (origin ? origin : '----'));

  private readonly fltDestinationText = this.sci.fltDestination.derive((destination) =>
    destination ? destination : '----',
  );

  private readonly scheduledDateText = Signal.of('----');

  private readonly outBlockTimeText = this.props.container.ansu.outBlockTime.derive((t) => AnsuOps.formatDateTime(t));
  private readonly offBlockTimeText = this.props.container.ansu.offBlockTime.derive((t) => AnsuOps.formatDateTime(t));
  private readonly onBlockTimeText = this.props.container.ansu.onBlockTime.derive((t) => AnsuOps.formatDateTime(t));
  private readonly inBlockTimeText = this.props.container.ansu.inBlockTime.derive((t) => AnsuOps.formatDateTime(t));

  private readonly userWeight = NXDataStore.getSetting('CONFIG_USING_METRIC_UNIT').derive((v) => (v ? 'KG' : 'LBS'));

  private readonly outBlockFobText = Derived.from(
    ([weight, unit]) =>
      weight !== null
        ? `${(NXUnits.kgToUser(weight) / 1_000).toFixed(1).padStart(5, '\xa0')} ${unit === 'KG' ? 'T' : 'KLB'}`
        : '----- -',
    this.props.container.ansu.outBlockFob,
    this.userWeight,
  );
  private readonly offBlockFobText = Derived.from(
    ([weight, unit]) =>
      weight !== null
        ? `${(NXUnits.kgToUser(weight) / 1_000).toFixed(1).padStart(5, '\xa0')} ${unit === 'KG' ? 'T' : 'KLB'}`
        : '----- -',
    this.props.container.ansu.offBlockFob,
    this.userWeight,
  );
  private readonly onBlockFobText = Derived.from(
    ([weight, unit]) =>
      weight !== null
        ? `${(NXUnits.kgToUser(weight) / 1_000).toFixed(1).padStart(5, '\xa0')} ${unit === 'KG' ? 'T' : 'KLB'}`
        : '----- -',
    this.props.container.ansu.onBlockFob,
    this.userWeight,
  );
  private readonly inBlockFobText = Derived.from(
    ([weight, unit]) =>
      weight !== null
        ? `${(NXUnits.kgToUser(weight) / 1_000).toFixed(1).padStart(5, '\xa0')} ${unit === 'KG' ? 'T' : 'KLB'}`
        : '----- -',
    this.props.container.ansu.inBlockFob,
    this.userWeight,
  );

  private readonly currentStateText = Derived.from(
    ([out, off, on, inb]) => {
      if (out === null) {
        return 'INITIAL';
      } else if (off === null) {
        return 'PRE-FLIGHT';
      } else if (on === null) {
        return 'IN-FLIGHT';
      } else if (inb === null) {
        return 'POST-FLIGHT';
      }
      return 'IN';
    },
    this.props.container.ansu.outBlockTime,
    this.props.container.ansu.offBlockTime,
    this.props.container.ansu.onBlockTime,
    this.props.container.ansu.inBlockTime,
  );

  private readonly flightTimeText = this.props.container.ansu.flightTime.derive((t) => {
    if (t === null) {
      return '--:--:--';
    }

    const duration = new Date(t);
    return `${String(duration.getUTCDate() * duration.getUTCHours()).padStart(2, '0')}:${String(duration.getUTCMinutes()).padStart(2, '0')}:${String(duration.getUTCSeconds()).padStart(2, '0')}`;
  });

  private readonly blockTimeText = this.props.container.ansu.blockTime.derive((t) => {
    if (t === null) {
      return '--:--:--';
    }

    const duration = new Date(t);
    return `${String(duration.getUTCDate() * duration.getUTCHours()).padStart(2, '0')}:${String(duration.getUTCMinutes()).padStart(2, '0')}:${String(duration.getUTCSeconds()).padStart(2, '0')}`;
  });

  public afterRender(node: ViewNode): void {
    super.afterRender(node);

    this.subscriptions.push(
      this.userWeight,
      this.fltNumberText,
      this.fltOriginText,
      this.fltDestinationText,
      this.outBlockTimeText,
      this.offBlockTimeText,
      this.onBlockTimeText,
      this.inBlockTimeText,
      this.outBlockFobText,
      this.offBlockFobText,
      this.onBlockFobText,
      this.inBlockFobText,
      this.currentStateText,
      this.flightTimeText,
      this.blockTimeText,
    );
    this.subscriptions.push(
      this.sub
        .topic('simTime')
        .limit(1)
        .listen((time) => {
          const date = new Date(time);
          this.scheduledDateText.set(
            `${String(date.getUTCDate()).padStart(2, '0')}-${AnsuOps.months[date.getUTCMonth()]}-${String(date.getUTCFullYear()).substring(2)}`,
          );
        }),
    );
  }

  destroy(): void {
    super.destroy();
  }

  render(): ViewNode {
    return (
      <>
        <div class="oit-ccom-headline">Flight Log</div>
        <div class="oit-labeled-box-container">
          <span class="oit-labeled-box-label">Flight</span>
          <div style="display: flex; flex-direction: row; justify-content: flex-start; break-before: always;">
            <RadioButtonGroup
              values={['Current FLT']}
              selectedIndex={this.flightSelectionSelectedIndex}
              idPrefix="oit-flight-log-flight"
            />
          </div>
        </div>
        <div class="oit-labeled-box-container">
          <div class="oit-ccom-flight-log-grid">
            <div class="oit-align-right-sp25">From</div>
            <div>{this.fltOriginText}</div>
            <div class="oit-align-right-sp25">To</div>
            <div>{this.fltDestinationText}</div>
            <div class="oit-align-right-sp25 mt20">Flight Identifier</div>
            <div class="oit-green-text mt20">{this.fltNumberText}</div>
            <div class="oit-align-right-sp25 mt20">Scheduled Date</div>
            <div class="oit-green-text mt20">{this.scheduledDateText}</div>
          </div>
        </div>
        <div class="oit-ccom-flight-log-bottom-structure">
          <div class="oit-ccom-flight-log-bottom-structure-left">
            <div class="oit-ccom-flight-log-oooi-table">
              <div class="oit-ccom-flight-log-oooi-table-header oit-table-ib">State</div>
              <div class="oit-ccom-flight-log-oooi-table-header oit-table-ib">Time (UTC)</div>
              <div class="oit-ccom-flight-log-oooi-table-header oit-table-ib">FOB</div>
              <div class="oit-table-ib">OUT</div>
              <div class="oit-green-text oit-table-ib">{this.outBlockTimeText}</div>
              <div class="oit-green-text oit-table-ib">{this.outBlockFobText}</div>
              <div class="oit-table-ib">OFF</div>
              <div class="oit-green-text oit-table-ib">{this.offBlockTimeText}</div>
              <div class="oit-green-text oit-table-ib">{this.offBlockFobText}</div>
              <div class="oit-table-ib">ON</div>
              <div class="oit-green-text oit-table-ib">{this.onBlockTimeText}</div>
              <div class="oit-green-text oit-table-ib">{this.onBlockFobText}</div>
              <div class="oit-table-ib">IN</div>
              <div class="oit-green-text oit-table-ib">{this.inBlockTimeText}</div>
              <div class="oit-green-text oit-table-ib">{this.inBlockFobText}</div>
            </div>
          </div>
          <div class="oit-ccom-flight-log-bottom-structure-right">
            <div class="oit-ccom-flight-log-bottom-structure-right1">
              <div class="mr15">Current State</div>
              <div class="oit-green-text">{this.currentStateText}</div>
            </div>
            <div class="oit-ccom-flight-log-bottom-structure-right2">
              <div class="mr15">Flight Time</div>
              <div>Block Time</div>
              <div class="oit-green-text mr15 tc">{this.flightTimeText}</div>
              <div class="oit-green-text tc">{this.blockTimeText}</div>
            </div>
          </div>
        </div>
        <div style="flex-grow: 1" />
      </>
    );
  }
}

// Copyright (c) 2026 FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0

import {
  ViewProps,
  View,
  UI,
  Derived,
  DerivedReadable,
  Signal,
  Readable,
  Logic,
  Bond,
  ViewNode,
} from 'xplane-js/ui';
import { FmcInterface } from '../../FMC/FmcInterface';
import { Button, ButtonMenuItem } from '../../../MsfsAvionicsCommon/UiWidgets/Button';
import { FmgcFlightPhase } from '@shared/flightphase';

export interface CpnyWindRequestButtonProps extends ViewProps {
  fmc: FmcInterface;
  flightPlanIndex: Readable<number>;
  tmpyExists: Readable<boolean>;
  isActiveOrCopiedFromActive: Readable<boolean>;
}

export class CpnyWindRequestButton extends View<CpnyWindRequestButtonProps> {
  private readonly uplinkAvailable = Signal.of(false);

  private readonly uplinkAvailableForAnyPlan = Signal.of(false);

  private readonly subs: Bond[] = [];

  private readonly disabledDueToFlightPhase = Derived.from(
    ([isActiveOrCopiedFromActive, flightPhase]) => isActiveOrCopiedFromActive && flightPhase > FmgcFlightPhase.Cruise,
    this.props.isActiveOrCopiedFromActive,
    this.props.fmc.fmgc.data.flightPhase,
  );

  /* Disabled when: Temporary exists, uplink in progress, pending insertion in another plan or flight phase does not allow it */
  private readonly disabled = Derived.from(
    // TODO Disable when connection to ATSU is lost.
    ([disabledDueToFlightPhase, tmpy, uplinkInProgress, uplinkAvailableForPlan, uplinkAvailableForAnyPlan]) => {
      return (
        disabledDueToFlightPhase || tmpy || uplinkInProgress || (!uplinkAvailableForPlan && uplinkAvailableForAnyPlan)
      );
    },
    this.disabledDueToFlightPhase,
    this.props.tmpyExists,
    this.props.fmc.getUplinkInProgress(),
    this.uplinkAvailable,
    this.uplinkAvailableForAnyPlan,
  );

  private readonly topLabel = Derived.from(
    ([avail, uplinkInProgress]) => {
      if (uplinkInProgress) {
        return 'REQUEST';
      }

      return avail ? 'RECEIVED' : 'CPNY WIND';
    },
    this.uplinkAvailable,
    this.props.fmc.getCpnyWindUplinkInProgress(),
  );

  private readonly bottomLabel = Derived.from(
    ([avail, uplinkInProgress]) => {
      if (uplinkInProgress) {
        return 'PENDING...';
      }

      return avail ? 'CPNY WIND' : 'REQUEST';
    },
    this.uplinkAvailable,
    this.props.fmc.getCpnyWindUplinkInProgress(),
  );

  private readonly showAsterisk = Derived.from(
    ([avail, uplinkInProgress]) => !avail && !uplinkInProgress,
    this.uplinkAvailable,
    this.props.fmc.getCpnyWindUplinkInProgress(),
  );

  private readonly hideAsterisk = this.showAsterisk.derive(Logic.not());

  private readonly label = (
    <div style="display: flex; flex-direction: row; justify-content: space-between;">
      <span style="text-align: center; vertical-align: center; margin-right: 10px;">
        {this.topLabel}
        <br />
        {this.bottomLabel}
      </span>
      <span class={{ hidden: this.hideAsterisk }} style="display: flex; align-items: center; justify-content: center;">
        *
      </span>
    </div>
  );

  private readonly menuItems: DerivedReadable<ButtonMenuItem[]> = Derived.from(
    ([avail, tmpy]) =>
      avail && !tmpy
        ? [
            {
              label: 'INSERT*',
              action: () => this.props.fmc.insertCpnyWind(this.props.flightPlanIndex.get()),
            },
            {
              label: 'CLEAR*',
              action: () => {
                this.props.fmc.deleteCpnyWind(this.props.flightPlanIndex.get());
              },
            },
          ]
        : [],
    this.uplinkAvailable,
    this.props.tmpyExists,
  );

  afterRender(_node: ViewNode): void {
    this.subs.push(
      this.props.flightPlanIndex.watch((v) => {
        this.subs.push(this.props.fmc.getWindUplinkAvailableForPlan(v).forward(this.uplinkAvailable));
      }, true),
      this.props.fmc.getWindUplinkAvailableForPlan().forward(this.uplinkAvailableForAnyPlan),
    );
  }

  public render(): ViewNode {
    return (
      <Button
        disabled={this.disabled}
        label={this.label}
        onClick={() => {
          if (!this.uplinkAvailable.get()) {
            this.props.fmc.requestCpnyWind(this.props.flightPlanIndex.get());
          }
        }}
        idPrefix="mfd-fms-wind-cpny-wind-request"
        menuItems={this.menuItems}
        buttonStyle="margin-right: 10px; justify-self: flex-end; width: 178px; height: 58px;"
        showArrow={false}
      />
    );
  }

  public destroy(): void {
    this.disabledDueToFlightPhase.destroy();
    this.disabled.destroy();
    this.topLabel.destroy();
    this.bottomLabel.destroy();
    this.showAsterisk.destroy();
    this.hideAsterisk.destroy();
    this.menuItems.destroy();
    this.subs.forEach((s) => s.destroy());
    super.destroy();
  }
}

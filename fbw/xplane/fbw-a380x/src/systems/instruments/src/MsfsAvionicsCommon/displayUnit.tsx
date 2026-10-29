// Copyright (c) 2021-2023 FlyByWire Simulations
//
// SPDX-License-Identifier: GPL-3.0

import { UI, ClockTopics, View, Bus, Readable, ViewNode } from 'xplane-js/ui';
import { NXDataStore } from '@flybywiresim/fbw-sdk';
// import { getSupplier } from '@flybywiresim/fbw-sdk';
import { DisplayVars } from './SimVarTypes';

import './common.scss';
import { Vars } from 'xplane-js/data';

export const getDisplayIndex = () => {
  const url = Array.from(document.querySelectorAll('vcockpit-panel > *'))
    .find((it) => it.tagName.toLowerCase() !== 'wasm-instrument')
    .getAttribute('url');

  return url ? parseInt(url.substring(url.length - 1), 10) : 0;
};

type DisplayUnitProps = {
  bus: Bus;
  normDmc: number;
  failed?: Readable<boolean>;
  powered?: Readable<boolean>;
  brightness?: Readable<number>;
  test?: Readable<number>;
};

enum DisplayUnitState {
  On,
  Off,
  Selftest,
  Standby,
  MaintenanceMode,
  EngineeringTest,
}

export class DisplayUnit extends View<DisplayUnitProps> {
  private state: DisplayUnitState = Vars.read('L:A32NX_COLD_AND_DARK_SPAWN', 'Bool')
    ? DisplayUnitState.Off
    : DisplayUnitState.Standby;

  private timeOut: number = 0;

  private selfTestRef = UI.ref<SVGElement>();

  private maintenanceModeRef = UI.ref<SVGElement>();

  private engineeringTestModeRef = UI.ref<SVGElement>();

  private pfdRef = UI.ref<HTMLDivElement>();

  // private supplyingDmc: number = 3;

  private brightness: number = 0;

  private failed = false;

  private powered = false;

  public afterRender(node: ViewNode): void {
    super.afterRender(node);

    const sub = this.props.bus.inbox<DisplayVars & ClockTopics>();

    sub
      .topic('realTime')
      .limit(1)
      .listen((_t) => {
        // override MSFS menu animations setting for this instrument
        if (!document.documentElement.classList.contains('animationsEnabled')) {
          document.documentElement.classList.add('animationsEnabled');
        }
      });

    this.props.brightness?.watch((f) => {
      this.brightness = f;
      this.updateState();
    }, true);

    this.props.failed?.watch((f) => {
      this.failed = f;
      this.updateState();
    }, true);

    this.props.powered?.watch((f) => {
      this.powered = f;
      this.updateState();
    }, true);
  }

  setTimer(time: number) {
    this.timeOut = window.setTimeout(() => {
      if (this.state === DisplayUnitState.Standby) {
        this.state = DisplayUnitState.Off;
      }
      if (this.state === DisplayUnitState.Selftest) {
        this.state = DisplayUnitState.On;
      }
      this.updateState();
    }, time * 1000);
  }

  // TODO: Fix and reenable
  /*
    checkMaintMode() {
        const dmcKnob = SimVar.GetSimVarValue('L:A32NX_EIS_DMC_SWITCHING_KNOB', 'Enum');
        this.supplyingDmc = getSupplier(this.props.normDmc, dmcKnob);
        const dmcDisplayTestMode = SimVar.GetSimVarValue(`L:A32NX_DMC_DISPLAYTEST:${this.supplyingDmc}`, 'Enum');
        switch (dmcDisplayTestMode) {
        case 1:
            this.state = DisplayUnitState.MaintenanceMode;
            break;
        case 2:
            this.state = DisplayUnitState.EngineeringTest;
            break;
        default:
            this.state = DisplayUnitState.On;
        }
    }
    */

  updateState() {
    if (this.state !== DisplayUnitState.Off && this.failed) {
      this.state = DisplayUnitState.Off;
      clearTimeout(this.timeOut);
    } else if (this.state === DisplayUnitState.On && (this.brightness === 0 || !this.powered)) {
      this.state = DisplayUnitState.Standby;
      this.setTimer(10);
    } else if (this.state === DisplayUnitState.Standby && this.brightness !== 0 && this.powered) {
      this.state = DisplayUnitState.On;
      clearTimeout(this.timeOut);
    } else if (this.state === DisplayUnitState.Off && this.brightness !== 0 && this.powered && !this.failed) {
      this.state = DisplayUnitState.Selftest;
      this.setTimer(parseInt(NXDataStore.getLegacy('CONFIG_SELF_TEST_TIME', '15')));
    } else if (this.state === DisplayUnitState.Selftest && (this.brightness === 0 || !this.powered)) {
      this.state = DisplayUnitState.Off;
      clearTimeout(this.timeOut);
    }

    if (this.state === DisplayUnitState.Selftest) {
      this.selfTestRef.target.style.display = 'block';
      this.maintenanceModeRef.target.style.display = 'none';
      this.engineeringTestModeRef.target.style.display = 'none';
      this.pfdRef.target.style.display = 'none';
    } else if (this.state === DisplayUnitState.On) {
      this.selfTestRef.target.style.display = 'none';
      this.maintenanceModeRef.target.style.display = 'none';
      this.engineeringTestModeRef.target.style.display = 'none';
      this.pfdRef.target.style.display = 'block';
    } else if (this.state === DisplayUnitState.MaintenanceMode) {
      this.selfTestRef.target.style.display = 'none';
      this.maintenanceModeRef.target.style.display = 'block';
      this.engineeringTestModeRef.target.style.display = 'none';
      this.pfdRef.target.style.display = 'none';
    } else if (this.state === DisplayUnitState.EngineeringTest) {
      this.selfTestRef.target.style.display = 'none';
      this.maintenanceModeRef.target.style.display = 'none';
      this.engineeringTestModeRef.target.style.display = 'block';
      this.pfdRef.target.style.display = 'none';
    } else {
      this.selfTestRef.target.style.display = 'none';
      this.maintenanceModeRef.target.style.display = 'none';
      this.engineeringTestModeRef.target.style.display = 'none';
      this.pfdRef.target.style.display = 'none';
    }
  }

  render(): ViewNode {
    return (
      <>
        <svg style="display:none" ref={this.selfTestRef} class="SelfTest" viewBox="0 0 600 600">
          <rect class="SelfTestBackground" x="0" y="0" width="100%" height="100%" />
          <text class="SelfTestText" x="50%" y="50%">
            SELF TEST IN PROGRESS
          </text>
          <text class="SelfTestText" x="50%" y="56%">
            (MAX 40 SECONDS)
          </text>
        </svg>

        <svg style="display:none" ref={this.maintenanceModeRef} class="MaintenanceMode" viewBox="0 0 600 600">
          <text class="SelfTestText" x="50%" y="50%">
            MAINTENANCE MODE
          </text>
        </svg>

        <svg style="display:none" ref={this.engineeringTestModeRef} class="EngineeringTestMode" viewBox="0 0 600 600">
          <text class="EngineeringTestModeText" x={9} y={250}>
            P/N : C19755BA01
          </text>
          <text class="EngineeringTestModeText" x={10} y={270}>
            S/N : C1975517334
          </text>
          <text class="EngineeringTestModeText" x={10} y={344}>
            EIS SW
          </text>
          <text class="EngineeringTestModeText" x={10} y={366}>
            P/N : C1975517332
          </text>
          <text class="EngineeringTestModeText" text-anchor="end" x="90%" y={250}>
            THALES AVIONICS
          </text>
          <text class="EngineeringTestModeText" text-anchor="end" x="98%" y={366}>
            LCDU 725
          </text>
        </svg>

        <div style="display:none" ref={this.pfdRef}>
          {this.props.children}
        </div>
      </>
    );
  }
}

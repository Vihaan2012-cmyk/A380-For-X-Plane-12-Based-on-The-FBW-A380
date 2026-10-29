// Copyright (c) 2021-2023 FlyByWire Simulations
//
// SPDX-License-Identifier: GPL-3.0

import {
  ClockTopics,
  View,
  Bus,
  UI,
  Derived,
  Signal,
  Readable,
  ViewNode,
} from 'xplane-js/ui';
import { NXDataStore } from '@flybywiresim/fbw-sdk';
// import { getSupplier } from '@flybywiresim/fbw-sdk';
import { DcElectricalBus } from '@shared/electrical';
import { DisplayVars } from './SimVarTypes';

import './common.scss';
import { Vars } from 'xplane-js/data';

export const getDisplayIndex = () => {
  const url = Array.from(document.querySelectorAll('vcockpit-panel > *'))
    ?.find((it) => it.tagName.toLowerCase() !== 'wasm-instrument')
    ?.getAttribute('url');

  return url ? parseInt(url.substring(url.length - 1), 10) : 0;
};

export enum DisplayUnitID {
  CaptPfd,
  CaptNd,
  CaptMfd,
  FoPfd,
  FoNd,
  FoMfd,
  Ewd,
  Sd,
  CaptHud,
  FoHud,
}

const DisplayUnitToDCBus: { [k in DisplayUnitID]: DcElectricalBus[] } = {
  [DisplayUnitID.CaptPfd]: [DcElectricalBus.DcEssInFlight], // powered by 409PP
  [DisplayUnitID.CaptNd]: [DcElectricalBus.DcEssInFlight, DcElectricalBus.Dc1], // powered by 415PP or 105PP
  [DisplayUnitID.CaptMfd]: [DcElectricalBus.DcEss, DcElectricalBus.Dc1], // powered by 423PP or 111PP
  [DisplayUnitID.FoPfd]: [DcElectricalBus.Dc2],
  [DisplayUnitID.FoNd]: [DcElectricalBus.Dc1, DcElectricalBus.Dc2],
  [DisplayUnitID.FoMfd]: [DcElectricalBus.Dc1, DcElectricalBus.Dc2],
  [DisplayUnitID.Ewd]: [DcElectricalBus.DcEss], // powered by 423PP
  [DisplayUnitID.Sd]: [DcElectricalBus.Dc2],
  [DisplayUnitID.CaptHud]: [DcElectricalBus.Dc1],
  [DisplayUnitID.FoHud]: [DcElectricalBus.Dc2], //guess
};

const DisplayUnitToPotentiometer: { [k in DisplayUnitID]: number } = {
  [DisplayUnitID.CaptPfd]: 88,
  [DisplayUnitID.CaptNd]: 89,
  [DisplayUnitID.CaptMfd]: 98,
  [DisplayUnitID.FoPfd]: 90,
  [DisplayUnitID.FoNd]: 91,
  [DisplayUnitID.FoMfd]: 99,
  [DisplayUnitID.Ewd]: 92,
  [DisplayUnitID.Sd]: 93,
  [DisplayUnitID.CaptHud]: 71,
  [DisplayUnitID.FoHud]: 72,
};

interface DisplayUnitProps {
  bus: Bus;
  displayUnitId: DisplayUnitID;
  failed?: Readable<boolean>;
  test?: Readable<number>;
  brightness?: Readable<number>;
}

enum DisplayUnitState {
  On,
  MaintenanceMode,
  EngineeringTest,
  Off,
  ThalesBootup,
  Selftest,
  Standby,
}

export class CdsDisplayUnit extends View<DisplayUnitProps> {
  private state: DisplayUnitState = Vars.read('L:A32NX_COLD_AND_DARK_SPAWN', 'Bool')
    ? DisplayUnitState.Off
    : DisplayUnitState.Standby;

  private timeOut: number = 0;

  private selfTestRef = UI.ref<SVGElement>();

  private thalesBootupRef = UI.ref<SVGElement>();

  private maintenanceModeRef = UI.ref<SVGElement>();

  private engineeringTestModeRef = UI.ref<SVGElement>();

  private pfdRef = UI.ref<HTMLDivElement>();

  // private supplyingDmc: number = 3;

  private readonly brightness = Signal.of(0);

  private failed = false;

  private readonly powered = Signal.of(false);

  private readonly isHudDisplay =
    this.props.displayUnitId === DisplayUnitID.CaptHud || this.props.displayUnitId === DisplayUnitID.FoHud
      ? true
      : false;

  public afterRender(node: ViewNode): void {
    super.afterRender(node);

    const sub = this.props.bus.inbox<DisplayVars & ClockTopics>();

    sub.topic('realTime').listen(() => this.update());
    sub
      .topic('realTime')
      .limit(1)
      .listen((_t) => {
        // override MSFS menu animations setting for this instrument
        if (!document.documentElement.classList.contains('animationsEnabled')) {
          document.documentElement.classList.add('animationsEnabled');
        }
      });

    Derived.from(
      () => {
        this.updateState();
      },
      this.brightness,
      this.powered,
    );

    this.props.failed?.watch((f) => {
      this.failed = f;
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

  public setHudBrightness(potentiometer: number) {
    const pos =
      getDisplayIndex() === 8
        ? Vars.read('L:A320_Neo_HUD_L_POS', 'number')
        : Vars.read('L:A320_Neo_HUD_R_POS', 'number');
    if (pos !== 0) {
      setTimeout(() => {
        this.brightness.set(0);
      }, 10);
    } else {
      setTimeout(() => {
        this.brightness.set(potentiometer);
      }, 1250);
    }
  }

  public update() {
    const potentiometer = Vars.read(
      `fbw/LIGHT_POTENTIOMETER_${DisplayUnitToPotentiometer[this.props.displayUnitId]}`,
      'percent over 100',
    );
    const poweredByBus1 = Vars.read(
      `L:A32NX_ELEC_${DisplayUnitToDCBus[this.props.displayUnitId][0]}_BUS_IS_POWERED`,
      'Bool',
    );
    const poweredByBus2 = Vars.read(
      `L:A32NX_ELEC_${DisplayUnitToDCBus[this.props.displayUnitId][1]}_BUS_IS_POWERED`,
      'Bool',
    );

    if (!this.isHudDisplay) {
      this.brightness.set(potentiometer);
    } else {
      this.setHudBrightness(potentiometer);
    }
    this.powered.set(poweredByBus1 || poweredByBus2);
  }

  updateState() {
    if (this.state !== DisplayUnitState.Off && this.failed) {
      this.state = DisplayUnitState.Off;
      clearTimeout(this.timeOut);
    } else if (this.state === DisplayUnitState.On && (this.brightness.get() === 0 || !this.powered.get())) {
      this.state = DisplayUnitState.Standby;
      this.setTimer(10);
    } else if (this.state === DisplayUnitState.Standby && this.brightness.get() !== 0 && this.powered.get()) {
      this.state = DisplayUnitState.On;
      clearTimeout(this.timeOut);
    } else if (
      this.state === DisplayUnitState.Off &&
      this.brightness.get() !== 0 &&
      this.powered.get() &&
      !this.failed
    ) {
      this.state = DisplayUnitState.ThalesBootup;
      this.setTimer(0.25 + Math.random() * 0.2);
    } else if (
      this.state === DisplayUnitState.ThalesBootup &&
      this.brightness.get() !== 0 &&
      this.powered.get() &&
      !this.failed
    ) {
      this.state = DisplayUnitState.Selftest;
      // Aircraft Presets set L:A32NX_AIRCRAFT_PRESET_QUICK_MODE while a preset is
      // being applied: skip the safety test's wait, same convention as
      // ISISDisplayUnit.tsx's aircraftPresetQuickMode.
      this.setTimer(
        Vars.read('L:A32NX_AIRCRAFT_PRESET_QUICK_MODE', 'Bool')
          ? 0
          : parseInt(NXDataStore.getLegacy('CONFIG_SELF_TEST_TIME', '15')),
      );
    } else if (
      (this.state === DisplayUnitState.Selftest || this.state === DisplayUnitState.ThalesBootup) &&
      (this.brightness.get() === 0 || !this.powered.get())
    ) {
      this.state = DisplayUnitState.Off;
      clearTimeout(this.timeOut);
    }

    if (this.state === DisplayUnitState.Selftest) {
      this.selfTestRef.target.style.display = 'block';
      this.thalesBootupRef.target.style.display = 'none';
      this.maintenanceModeRef.target.style.display = 'none';
      this.engineeringTestModeRef.target.style.display = 'none';
      this.pfdRef.target.style.display = 'none';
    } else if (this.state === DisplayUnitState.ThalesBootup) {
      this.selfTestRef.target.style.display = 'none';
      this.thalesBootupRef.target.style.display = 'block';
      this.maintenanceModeRef.target.style.display = 'none';
      this.engineeringTestModeRef.target.style.display = 'none';
      this.pfdRef.target.style.display = 'none';
    } else if (this.state === DisplayUnitState.On) {
      this.selfTestRef.target.style.display = 'none';
      this.thalesBootupRef.target.style.display = 'none';
      this.maintenanceModeRef.target.style.display = 'none';
      this.engineeringTestModeRef.target.style.display = 'none';
      this.pfdRef.target.style.display = 'block';
    } else if (this.state === DisplayUnitState.MaintenanceMode) {
      this.selfTestRef.target.style.display = 'none';
      this.thalesBootupRef.target.style.display = 'none';
      this.maintenanceModeRef.target.style.display = 'block';
      this.engineeringTestModeRef.target.style.display = 'none';
      this.pfdRef.target.style.display = 'none';
    } else if (this.state === DisplayUnitState.EngineeringTest) {
      this.selfTestRef.target.style.display = 'none';
      this.thalesBootupRef.target.style.display = 'none';
      this.maintenanceModeRef.target.style.display = 'none';
      this.engineeringTestModeRef.target.style.display = 'block';
      this.pfdRef.target.style.display = 'none';
    } else {
      this.selfTestRef.target.style.display = 'none';
      this.thalesBootupRef.target.style.display = 'none';
      this.maintenanceModeRef.target.style.display = 'none';
      this.engineeringTestModeRef.target.style.display = 'none';
      this.pfdRef.target.style.display = 'none';
    }
  }

  render(): ViewNode {
    return (
      <>
        <svg style="display:none" ref={this.selfTestRef} class="SelfTest" viewBox="0 0 768 1024">
          <rect class="SelfTestBackground" x="0" y="0" width="100%" height="100%" />

          <text class="SelfTestText" x="50%" y="50%">
            SAFETY TEST IN PROGRESS
          </text>
          <text class="SelfTestText" x="50%" y="54%">
            (MAX 30 SECONDS)
          </text>
        </svg>

        <svg style="display:none" ref={this.thalesBootupRef} class="SelfTest" viewBox="0 0 768 1024">
          <rect class="SelfTestBackground" x="0" y="0" width="100%" height="100%" />

          <rect x={84} y={880} width={600} height={70} fill="#ffffff" />
          <rect x={89} y={885} width={35} height={60} fill="#4d4dff" />
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

// Copyright (c) 2025 FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0

import {
  ClockTopics,
  ViewProps,
  BusSignal,
  View,
  Bus,
  UI,
  Derived,
  Signal,
  Readable,
  Logic,
  Bond,
  ViewNode,
} from 'xplane-js/ui';
import { FailuresConsumer, NXDataStore } from '@flybywiresim/fbw-sdk';
import { AcElectricalBus, DcElectricalBus } from '@shared/electrical';

import './oit-display-unit.scss';
import { OitSimvars } from './OitSimvarPublisher';
import { A380Failure } from '@failures';
import { OisDomain } from './OIT';
import { Vars } from 'xplane-js/data';

export const getDisplayIndex = () => {
  const url = Array.from(document.querySelectorAll('vcockpit-panel > *'))
    ?.find((it) => it.tagName.toLowerCase() !== 'wasm-instrument')
    ?.getAttribute('url');

  return url ? parseInt(url.substring(url.length - 1), 10) : 0;
};

export enum OitDisplayUnitID {
  CaptOit,
  FoOit,
}

const DisplayUnitToDCBus: { [k in OitDisplayUnitID]: (DcElectricalBus | AcElectricalBus)[] } = {
  [OitDisplayUnitID.CaptOit]: [AcElectricalBus.Ac2, DcElectricalBus.Dc2],
  [OitDisplayUnitID.FoOit]: [AcElectricalBus.AcEss, DcElectricalBus.DcEssInFlight, DcElectricalBus.Dc1],
};

interface DisplayUnitProps {
  readonly bus: Bus;
  readonly displayUnitId: OitDisplayUnitID;
  readonly failuresConsumer: FailuresConsumer;
  readonly avncsOrFltOps: Readable<OisDomain>;
  readonly test?: Readable<number>;
}

enum DisplayUnitState {
  On,
  Off,
  Bootup,
  Selftest,
  Standby,
  Failed,
}

export class OitDisplayUnit extends View<DisplayUnitProps & ViewProps> {
  private readonly subs: Bond[] = [];

  private readonly sub = this.props.bus.inbox<OitSimvars & ClockTopics>();

  private readonly state = Signal.of<DisplayUnitState>(
    Vars.read('L:A32NX_COLD_AND_DARK_SPAWN', 'Bool') ? DisplayUnitState.Off : DisplayUnitState.Standby,
  );

  private readonly failureKey =
    this.props.displayUnitId === OitDisplayUnitID.CaptOit ? A380Failure.CaptainOit : A380Failure.FirstOfficerOit;

  private timeOut: number = 0;

  private readonly selfTestRef = UI.ref<HTMLDivElement>();

  private oitRef = UI.ref<HTMLDivElement>();

  public readonly powered = Signal.of(false);

  private readonly brightness = BusSignal.from(
    this.sub.topic(this.props.displayUnitId === OitDisplayUnitID.CaptOit ? 'potentiometerCaptain' : 'potentiometerFo'),
    75,
  );

  private readonly nssAnsu1Failed = BusSignal.from(this.sub.topic('nssAnsu1Healthy'), true).derive(
    Logic.not(),
  );

  private readonly nssAnsu2Failed = BusSignal.from(this.sub.topic('nssAnsu2Healthy'), true).derive(
    Logic.not(),
  );

  private readonly allNssAnsuFailed = Derived.from(
    Logic.and(),
    this.nssAnsu1Failed,
    this.nssAnsu2Failed,
  );

  private readonly fltOpsAnsuFailed = BusSignal.from(this.sub.topic('fltOpsAnsu1Healthy'), true).derive(
    Logic.not(),
  );

  private readonly fltOpsLaptopFailed = BusSignal.from(
    this.sub.topic(this.props.displayUnitId === OitDisplayUnitID.CaptOit ? 'laptopCaptHealthy' : 'laptopFoHealthy'),
    true,
  ).derive(Logic.not());

  private readonly fltOpsFailed = Derived.from(
    Logic.or(),
    this.fltOpsAnsuFailed,
    this.fltOpsLaptopFailed,
  );

  public readonly failed = Derived.from(
    ([opMode, state, nssFail, fltOpsFail]) =>
      state === DisplayUnitState.On && ((opMode === 'nss-avncs' && nssFail) || (opMode === 'flt-ops' && fltOpsFail)),
    this.props.avncsOrFltOps,
    this.state,
    this.allNssAnsuFailed,
    this.fltOpsFailed,
  );

  private readonly failedDisplay = this.failed.derive((v) => (v ? 'block' : 'none'));

  private readonly bus1Simvar = `L:A32NX_ELEC_${DisplayUnitToDCBus[this.props.displayUnitId][0]}_BUS_IS_POWERED`;
  private readonly bus2Simvar = `L:A32NX_ELEC_${DisplayUnitToDCBus[this.props.displayUnitId][1]}_BUS_IS_POWERED`;
  // DisplayUnitToDCBus[CaptOit] has only 2 entries (no third bus is
  // documented for it, unlike FoOit's 3), so indexing [2] unconditionally
  // used to build the bogus name "L:A32NX_ELEC_undefined_BUS_IS_POWERED".
  // update() below already guards the actual GetSimVarValue(bus3Simvar, ...)
  // call on this same array-length condition, so that bogus name was never
  // actually read live -- but guard its construction too, so it can't leak
  // out via a future direct use of bus3Simvar or a change to that guard.
  private readonly bus3Simvar = DisplayUnitToDCBus[this.props.displayUnitId][2]
    ? `L:A32NX_ELEC_${DisplayUnitToDCBus[this.props.displayUnitId][2]}_BUS_IS_POWERED`
    : '';

  public afterRender(node: ViewNode): void {
    super.afterRender(node);

    this.props.failuresConsumer.register(this.failureKey);

    this.subs.push(this.sub.topic('realTime').listen(() => this.update()));
    this.subs.push(
      this.sub
        .topic('realTime')
        .limit(1)
        .listen(() => {
          // override MSFS menu animations setting for this instrument
          if (!document.documentElement.classList.contains('animationsEnabled')) {
            document.documentElement.classList.add('animationsEnabled');
          }
        }),
    );

    this.subs.push(
      Derived.from(
        () => {
          this.updateState();
        },
        this.brightness,
        this.powered,
        this.failed,
      ),
    );

    this.subs.push(
      this.brightness,
      this.nssAnsu1Failed,
      this.nssAnsu2Failed,
      this.allNssAnsuFailed,
      this.fltOpsAnsuFailed,
      this.fltOpsLaptopFailed,
      this.fltOpsFailed,
      this.failed,
      this.failedDisplay,
    );
  }

  setTimer(time: number) {
    this.timeOut = window.setTimeout(() => {
      if (this.state.get() === DisplayUnitState.Standby) {
        this.state.set(DisplayUnitState.Off);
      }
      if (this.state.get() === DisplayUnitState.Selftest) {
        this.state.set(DisplayUnitState.On);
      }
      this.updateState();
    }, time * 1000);
  }

  public update() {
    const poweredByBus1 = DisplayUnitToDCBus[this.props.displayUnitId][0]
      ? Vars.read(this.bus1Simvar, 'Bool')
      : false;
    const poweredByBus2 = DisplayUnitToDCBus[this.props.displayUnitId][1]
      ? Vars.read(this.bus2Simvar, 'Bool')
      : false;
    const poweredByBus3 = DisplayUnitToDCBus[this.props.displayUnitId][2]
      ? Vars.read(this.bus3Simvar, 'Bool')
      : false;
    this.powered.set(
      (poweredByBus1 || poweredByBus2 || poweredByBus3) && !this.props.failuresConsumer.isActive(this.failureKey),
    );
  }

  updateState() {
    if (this.state.get() === DisplayUnitState.On && (this.brightness.get() === 0 || !this.powered.get())) {
      this.state.set(DisplayUnitState.Standby);
      this.setTimer(10);
    } else if (this.state.get() === DisplayUnitState.Standby && this.brightness.get() !== 0 && this.powered.get()) {
      this.state.set(DisplayUnitState.On);
      clearTimeout(this.timeOut);
    } else if (this.state.get() === DisplayUnitState.Off && this.brightness.get() !== 0 && this.powered.get()) {
      this.state.set(DisplayUnitState.Bootup);
      this.setTimer(0.25 + Math.random() * 0.2);
    } else if (this.state.get() === DisplayUnitState.Bootup && this.brightness.get() !== 0 && this.powered.get()) {
      this.state.set(DisplayUnitState.Selftest);
      // See CdsDisplayUnit.tsx: Aircraft Presets set this while a preset is being
      // applied, so skip the safety test's wait.
      this.setTimer(
        Vars.read('L:A32NX_AIRCRAFT_PRESET_QUICK_MODE', 'Bool')
          ? 0
          : parseInt(NXDataStore.getLegacy('CONFIG_SELF_TEST_TIME', '15')),
      );
    } else if (
      (this.state.get() === DisplayUnitState.Selftest || this.state.get() === DisplayUnitState.Bootup) &&
      (this.brightness.get() === 0 || !this.powered.get())
    ) {
      this.state.set(DisplayUnitState.Off);
      clearTimeout(this.timeOut);
    }

    if (this.state.get() === DisplayUnitState.Selftest) {
      this.selfTestRef.target.style.display = 'block';
      this.oitRef.target.style.display = 'none';
    } else if (this.state.get() === DisplayUnitState.Bootup) {
      this.selfTestRef.target.style.display = 'block';
      this.oitRef.target.style.display = 'none';
    } else if (this.state.get() === DisplayUnitState.On) {
      this.selfTestRef.target.style.display = 'none';
      this.oitRef.target.style.display = 'block';
    } else if (this.state.get() === DisplayUnitState.Failed) {
      this.selfTestRef.target.style.display = 'none';
      this.oitRef.target.style.display = 'none';
    } else {
      this.selfTestRef.target.style.display = 'none';
      this.oitRef.target.style.display = 'none';
    }
  }

  destroy(): void {
    for (const s of this.subs) {
      s.destroy();
    }

    super.destroy();
  }

  render(): ViewNode {
    return (
      <>
        <div class="SelfTest" ref={this.selfTestRef}></div>

        <div style="display:none" ref={this.oitRef}>
          {this.props.children}
        </div>

        <svg
          style={{
            display: this.failedDisplay,
          }}
          class="AnsuFailed"
          viewBox="0 0 1333 1000"
        >
          <text class="AnsuFailedText" x="50%" y="50%">
            Not available.
          </text>
        </svg>
      </>
    );
  }
}

//  Copyright (c) 2023-2024 FlyByWire Simulations
//  SPDX-License-Identifier: GPL-3.0

import {
  BusSignal,
  View,
  Bus,
  UI,
  Derived,
  Logic,
  ViewNode,
} from 'xplane-js/ui';
import { FcuSimvars } from '../Publishers/FcuSimvarPublisher';

interface BaroRefDisplayProps {
  isCaptSide: boolean;
  bus: Bus;
}

export class BaroRefDisplay extends View<BaroRefDisplayProps> {
  private sub = this.props.bus.inbox<FcuSimvars>();

  private baroIsInhg = BusSignal.from(
    this.sub.topic(`eisDisplay${this.props.isCaptSide ? 'Left' : 'Right'}BaroIsInhg`),
    false,
  );

  private baroIsStd = BusSignal.from(
    this.sub.topic(`eisDisplay${this.props.isCaptSide ? 'Left' : 'Right'}BaroIsStd`),
    false,
  );

  private baroValue = BusSignal.from(
    this.sub.topic(`eisDisplay${this.props.isCaptSide ? 'Left' : 'Right'}BaroValue`),
    0,
  );

  private baroMode = BusSignal.from(
    this.sub.topic(`eisDisplay${this.props.isCaptSide ? 'Left' : 'Right'}BaroMode`),
    0,
  );

  private baroPresetVisible = BusSignal.from(
    this.sub.topic(`eisDisplay${this.props.isCaptSide ? 'Left' : 'Right'}BaroPresetVisible`),
    false,
  );

  private lightsTest = BusSignal.from(this.sub.topic('lightsTest'), false);

  private baroValueSub = Derived.from(
    ([lightsTest, baroIsStd, baroIsInhg, baroValue]) => {
      if (lightsTest) {
        return '8.8.8.8';
      } else if (baroIsStd) {
        return 'Std';
      } else if (!baroIsInhg) {
        return Math.round(baroValue).toString().padStart(4, '0');
      } else {
        return baroValue.toFixed(2);
      }
    },
    this.lightsTest,
    this.baroIsStd,
    this.baroIsInhg,
    this.baroValue,
  );

  private baroPreselValueSub = Derived.from(
    ([lightsTest, baroIsInhg, baroIsStd, baroValue, baroMode]) => {
      if (lightsTest) {
        return '8.p.8.8'; // p is used as a standin character for the Q test character
      } else if (!baroIsStd && baroMode === 2) {
        return 'qfe';
      } else if (!baroIsInhg) {
        return Math.round(baroValue).toString().padStart(4, '0');
      } else {
        return baroValue.toFixed(2);
      }
    },
    this.lightsTest,
    this.baroIsInhg,
    this.baroIsStd,
    this.baroValue,
    this.baroMode,
  );

  private baroPreselSub = Derived.from(
    ([lightsTest, baroPresetVisible]) => {
      return lightsTest || baroPresetVisible;
    },
    this.lightsTest,
    this.baroPresetVisible,
  );

  private qnhLabelSub = Derived.from(
    ([lightsTest, baroMode]) => {
      if (lightsTest) {
        return true;
      } else if (baroMode === 0) {
        return false;
      } else if (baroMode === 1) {
        return true;
      } else {
        return false;
      }
    },
    this.lightsTest,
    this.baroMode,
  );

  public render(): ViewNode {
    return (
      <div id={`baro-screen-${this.props.isCaptSide ? 1 : 2}`} class="baro-screen">
        <div id="Selected">
          <svg width="100%" height="100%" class="Baro">
            <text
              id="QNH"
              class={{
                Common: true,
                Active: this.qnhLabelSub,
                Inactive: this.qnhLabelSub.derive(Logic.not()),
              }}
              x="6%"
              y="23%"
            >
              QNH
            </text>
            <text
              id="PreSelBaroValue"
              class={{
                Common: true,
                Value: this.baroPreselSub,
                Inactive: this.baroPreselSub.derive(Logic.not()),
              }}
              x="97%"
              y="30%"
              text-anchor="end"
            >
              {this.baroPreselValueSub}
            </text>
            <text id="Value" class="Common Value" x="4%" y="95%">
              {this.baroValueSub}
            </text>
          </svg>
        </div>
      </div>
    );
  }
}

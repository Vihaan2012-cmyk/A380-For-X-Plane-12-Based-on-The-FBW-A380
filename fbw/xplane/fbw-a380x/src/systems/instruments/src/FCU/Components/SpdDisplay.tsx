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

export class SpdDisplay extends View<{ bus: Bus }> {
  private sub = this.props.bus.inbox<FcuSimvars>();

  private value = BusSignal.from(this.sub.topic('afsDisplaySpdMachValue'), 0);

  private dashes = BusSignal.from(this.sub.topic('afsDisplaySpdMachDashes'), false);

  private machActive = BusSignal.from(this.sub.topic('afsDisplayMachMode'), false);

  private lightsTest = BusSignal.from(this.sub.topic('lightsTest'), false);

  private spdLabelSub = Derived.from(
    ([machActive, lightsTest]) => !machActive || lightsTest,
    this.machActive,
    this.lightsTest,
  );
  private machLabelSub = Derived.from(
    ([machActive, lightsTest]) => machActive || lightsTest,
    this.machActive,
    this.lightsTest,
  );

  private unitLabelSub = Derived.from(
    ([dashes, lightsTest, machActive]) => !machActive && (!dashes || lightsTest),
    this.dashes,
    this.lightsTest,
    this.machActive,
  );

  private valueSub = Derived.from(
    ([lightsTest, dashes, value, machActive]) => {
      if (lightsTest) {
        return '8.8.8';
      } else if (machActive && !dashes) {
        return value.toFixed(2);
      } else if (dashes) {
        return '---';
      } else {
        return Math.round(value).toString().padStart(3, '0');
      }
    },
    this.lightsTest,
    this.dashes,
    this.value,
    this.machActive,
  );

  public render(): ViewNode {
    return (
      <div id="Speed">
        <svg width="100%" height="100%">
          <text
            id="SPD"
            class={{
              Common: true,
              Active: this.spdLabelSub,
              Inactive: this.spdLabelSub.derive(Logic.not()),
            }}
            x="47%"
            y="20%"
          >
            SPD
          </text>
          <text
            id="MACH"
            class={{
              Common: true,
              Active: this.machLabelSub,
              Inactive: this.machLabelSub.derive(Logic.not()),
            }}
            x="0%"
            y="20%"
          >
            MACH
          </text>
          <text id="Value" class="Common Value" x="6%" y="97%">
            {this.valueSub}
          </text>
          <text
            id="KNOTS"
            class={{
              Common: true,
              Active: this.unitLabelSub,
              Inactive: this.unitLabelSub.derive(Logic.not()),
            }}
            x="79%"
            y="97%"
          >
            KT
          </text>
        </svg>
      </div>
    );
  }
}

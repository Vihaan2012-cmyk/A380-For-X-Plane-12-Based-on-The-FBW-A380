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

export class HdgDisplay extends View<{ bus: Bus }> {
  private sub = this.props.bus.inbox<FcuSimvars>();

  private value = BusSignal.from(this.sub.topic('afsDisplayHdgTrkValue'), 0);

  private dashes = BusSignal.from(this.sub.topic('afsDisplayHdgTrkDashes'), false);

  private trkFpaMode = BusSignal.from(this.sub.topic('afsDisplayTrkFpaMode'), false);

  private trueActive = BusSignal.from(this.sub.topic('afsDisplayTrueMode'), false);

  private lightsTest = BusSignal.from(this.sub.topic('lightsTest'), false);

  private trkLabelSub = Derived.from(
    ([trkFpa, lightsTest]) => trkFpa || lightsTest,
    this.trkFpaMode,
    this.lightsTest,
  );

  private hdgLabelSub = Derived.from(
    ([trkFpa, lightsTest]) => !trkFpa || lightsTest,
    this.trkFpaMode,
    this.lightsTest,
  );

  private trueLabelSub = Derived.from(
    ([trueActive, lightsTest]) => trueActive || lightsTest,
    this.trueActive,
    this.lightsTest,
  );

  private unitLabelSub = Derived.from(
    ([dashes, lightsTest]) => !dashes || lightsTest,
    this.dashes,
    this.lightsTest,
  );

  private valueSub = Derived.from(
    ([lightsTest, dashes, value]) => {
      if (lightsTest) {
        return '8.8.8';
      } else if (dashes) {
        return '---';
      } else {
        return Math.round(value).toString().padStart(3, '0');
      }
    },
    this.lightsTest,
    this.dashes,
    this.value,
  );

  public render(): ViewNode {
    return (
      <div id="Heading">
        <svg width="100%" height="100%">
          <text
            id="TRUE"
            class={{
              Common: true,
              Active: this.trueLabelSub,
              Inactive: this.trueLabelSub.derive(Logic.not()),
            }}
            x="23%"
            y="20%"
          >
            TRUE
          </text>
          <text
            id="HDG"
            class={{
              Common: true,
              Active: this.hdgLabelSub,
              Inactive: this.hdgLabelSub.derive(Logic.not()),
            }}
            x="48%"
            y="20%"
          >
            HDG
          </text>
          <text
            id="TRK"
            class={{
              Common: true,
              Active: this.trkLabelSub,
              Inactive: this.trkLabelSub.derive(Logic.not()),
            }}
            x="70%"
            y="20%"
          >
            TRK
          </text>
          <text id="Value" class="Common Value" x="47%" y="97%">
            {this.valueSub}
          </text>
          <text
            id="DEGREES"
            class={{
              Common: true,
              Value: true,
              Active: this.unitLabelSub,
              Inactive: this.unitLabelSub.derive(Logic.not()),
            }}
            x="89%"
            y="97%"
          >
            &#176;
          </text>
        </svg>
      </div>
    );
  }
}

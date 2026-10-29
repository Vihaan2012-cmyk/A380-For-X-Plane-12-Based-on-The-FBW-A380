import {
  BusSignal,
  View,
  Bus,
  UI,
  Derived,
  Maths,
  Logic,
  ViewNode,
} from 'xplane-js/ui';
import { FcuSimvars } from '../Publishers/FcuSimvarPublisher';

export class VsDisplay extends View<{ bus: Bus }> {
  private sub = this.props.bus.inbox<FcuSimvars>();

  private vsValue = BusSignal.from(this.sub.topic('afsDisplayVsFpaValue'), 0);

  private vsDashes = BusSignal.from(this.sub.topic('afsDisplayVsFpaDashes'), false);

  private trkFpaMode = BusSignal.from(this.sub.topic('afsDisplayTrkFpaMode'), false);

  private lightsTest = BusSignal.from(this.sub.topic('lightsTest'), false);

  private fpaLabelSub = Derived.from(
    ([trkFpa, lightsTest]) => trkFpa || lightsTest,
    this.trkFpaMode,
    this.lightsTest,
  );
  private vsLabelSub = Derived.from(
    ([trkFpa, lightsTest]) => !trkFpa || lightsTest,
    this.trkFpaMode,
    this.lightsTest,
  );

  private vsValueSub = Derived.from(
    ([lightsTest, vsDashes, vsValue, trkFpaMode]) => {
      const sign = vsValue < 0 ? '-' : '+';
      const absValue = Math.abs(vsValue);

      if (lightsTest) {
        return '+*.8.8.8';
      } else if (vsDashes) {
        return '~----';
      } else if (trkFpaMode && !vsDashes) {
        return `~${sign}${Maths.round(absValue, 0.1).toFixed(1)}`;
      } else if (!trkFpaMode && !vsDashes) {
        return `${sign}${Math.floor(absValue * 0.01)
          .toString()
          .padStart(2, '0')}oo`;
      } else {
        return '~00oo';
      }
    },
    this.lightsTest,
    this.vsDashes,
    this.vsValue,
    this.trkFpaMode,
  );

  public render(): ViewNode {
    return (
      <div id="VerticalSpeed">
        <svg width="100%" height="100%">
          <text
            id="VS"
            x="78%"
            y="20%"
            text-anchor="end"
            class={{ Active: this.vsLabelSub, Inactive: this.vsLabelSub.derive(Logic.not()) }}
          >
            V/S
          </text>
          <text
            id="FPA"
            x="54%"
            y="20%"
            text-anchor="end"
            class={{ Active: this.fpaLabelSub, Inactive: this.fpaLabelSub.derive(Logic.not()) }}
          >
            FPA
          </text>
          <text id="Value" class="Value" x="14%" y="97%">
            {this.vsValueSub}
          </text>
        </svg>
      </div>
    );
  }
}

import { BusSignal, View, Bus, UI, Derived, ViewNode } from 'xplane-js/ui';
import { FcuSimvars } from '../Publishers/FcuSimvarPublisher';

export class AltDisplay extends View<{ bus: Bus }> {
  private sub = this.props.bus.inbox<FcuSimvars>();

  private altValue = BusSignal.from(this.sub.topic('afsDisplayAltValue'), 0);

  private lightsTest = BusSignal.from(this.sub.topic('lightsTest'), false);

  private altValueSub = Derived.from(
    ([lightsTest, altValue]) => {
      if (lightsTest) {
        return '8.8.8.8.8';
      } else {
        return Math.round(altValue).toString().padStart(5, '0');
      }
    },
    this.lightsTest,
    this.altValue,
  );

  public render(): ViewNode {
    return (
      <div id="Altitude">
        <svg width="125%" height="100%">
          <text id="ALT" class="Common Active" x="27%" y="20%">
            ALT
          </text>
          <text id="Value" class="Common Value" x="0%" y="97%">
            {this.altValueSub}
          </text>
        </svg>
      </div>
    );
  }
}

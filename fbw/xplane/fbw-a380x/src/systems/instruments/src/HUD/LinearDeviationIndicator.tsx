import {
  BusSignal,
  View,
  Bus,
  UI,
  Derived,
  Signal,
  ViewNode,
} from 'xplane-js/ui';
import { FmsVars } from '../MsfsAvionicsCommon/providers/FmsDataPublisher';
import { Arinc429Values } from '../PFD/shared/ArincValueProvider';
import { ONE_DEG, ALT_TAPE_XPOS, ALT_TAPE_YPOS, HudElems, XWIND_TO_AIR_REF_OFFSET } from './HUDUtils';

let DisplayRange = 600;
const ValueSpacing = 100;
const DistanceSpacing = 33.3;
type LinearDeviationIndicatorProps = {
  bus: Bus;
};

export class LinearDeviationIndicator extends View<LinearDeviationIndicatorProps> {
  private shouldShowLinearDeviation = false;

  private componentTransform = Signal.of('');

  private upperLinearDeviationReadoutText = Signal.of('00');

  private upperLinearDeviationReadoutVisibility = Signal.of<'visible' | 'hidden'>('hidden');

  private lowerLinearDeviationReadoutText = Signal.of('00');

  private lowerLinearDeviationReadoutVisibility = Signal.of<'visible' | 'hidden'>('hidden');

  private linearDeviationDotVisibility = Signal.of<'visible' | 'hidden'>('hidden');

  private linearDeviationDotUpperHalfVisibility = Signal.of<'visible' | 'hidden'>('hidden');

  private linearDeviationDotLowerHalfVisibility = Signal.of<'visible' | 'hidden'>('hidden');

  private latchSymbolVisibility = Signal.of<'visible' | 'hidden'>('hidden');

  private linearDevRef = UI.ref<SVGGElement>();

  private upperLinearDevTextRef = UI.ref<SVGTextElement>();

  private lowerLinearDevTextRef = UI.ref<SVGTextElement>();

  private crosswindMode = true;

  private sub = this.props.bus.inbox<Arinc429Values & FmsVars & HudElems>();

  private altTape = BusSignal.from(this.sub.topic('altTape').distinct(), '');
  private xwindAltTape = BusSignal.from(this.sub.topic('xWindAltTape').distinct(), '');
  private isVisible = Derived.from(
    ([altTape, xwindAltTape]) => {
      if (altTape === 'block' || xwindAltTape === 'block') {
        return 'block';
      } else {
        return 'none';
      }
    },
    this.altTape,
    this.xwindAltTape,
  );

  // TODO: Use ARINC value for this
  private flightPathAltitude: Feet = 0;

  afterRender(node: ViewNode): void {
    super.afterRender(node);

    this.sub
      .topic('cWndMode')
      .distinct()
      .listen((value) => {
        this.crosswindMode = value;
        if (this.crosswindMode) {
          DisplayRange = 250;
          this.linearDevRef.target.style.transform = `translate3d(${ALT_TAPE_XPOS}px, ${ALT_TAPE_YPOS + XWIND_TO_AIR_REF_OFFSET}px, 0px)`;
          this.upperLinearDevTextRef.target.setAttribute('y', `303`);
          this.lowerLinearDevTextRef.target.setAttribute('y', `440`);
        } else {
          DisplayRange = 600;
          this.linearDevRef.target.style.transform = `translate3d(${ALT_TAPE_XPOS}px, ${ALT_TAPE_YPOS}px, 0px)`;
          this.upperLinearDevTextRef.target.setAttribute('y', '186');
          this.lowerLinearDevTextRef.target.setAttribute('y', '556');
        }
      });

    this.sub.topic('altitudeAr').listen((alt) => {
      if (!alt.isNormalOperation() || !this.shouldShowLinearDeviation) {
        this.upperLinearDeviationReadoutVisibility.set('hidden');
        this.lowerLinearDeviationReadoutVisibility.set('hidden');
        this.linearDeviationDotLowerHalfVisibility.set('hidden');
        this.linearDeviationDotUpperHalfVisibility.set('hidden');
        this.linearDeviationDotVisibility.set('hidden');

        return;
      }

      const deviation = alt.value - this.flightPathAltitude;
      const pixelOffset = this.pixelOffsetFromDeviation(Math.max(Math.min(deviation, DisplayRange), -DisplayRange));

      this.componentTransform.set(`translate(0 ${pixelOffset})`);

      const linearDeviationReadoutText = Math.min(99, Math.round(Math.abs(deviation) / 100))
        .toFixed(0)
        .padStart(2, '0');

      if (this.upperLinearDeviationReadoutVisibility.get() === 'visible') {
        this.upperLinearDeviationReadoutText.set(linearDeviationReadoutText);
      }

      if (this.lowerLinearDeviationReadoutVisibility.get() === 'visible') {
        this.lowerLinearDeviationReadoutText.set(linearDeviationReadoutText);
      }

      if (deviation > DisplayRange - 60) {
        this.lowerLinearDeviationReadoutVisibility.set('visible');
        this.linearDeviationDotLowerHalfVisibility.set('visible');

        this.upperLinearDeviationReadoutVisibility.set('hidden');
        this.linearDeviationDotUpperHalfVisibility.set('hidden');

        this.linearDeviationDotVisibility.set('hidden');
      } else if (deviation > -(DisplayRange - 100) && deviation < DisplayRange - 100) {
        this.lowerLinearDeviationReadoutVisibility.set('hidden');
        this.linearDeviationDotLowerHalfVisibility.set('hidden');

        this.upperLinearDeviationReadoutVisibility.set('hidden');
        this.linearDeviationDotUpperHalfVisibility.set('hidden');

        this.linearDeviationDotVisibility.set('visible');
      } else if (deviation < -DisplayRange + 60) {
        this.lowerLinearDeviationReadoutVisibility.set('hidden');
        this.linearDeviationDotLowerHalfVisibility.set('hidden');

        this.upperLinearDeviationReadoutVisibility.set('visible');
        this.linearDeviationDotUpperHalfVisibility.set('visible');

        this.linearDeviationDotVisibility.set('hidden');
      }
    });

    this.sub
      .topic('linearDeviationActive')
      .distinct()
      .listen((isActive) => (this.shouldShowLinearDeviation = isActive));

    this.sub
      .topic('verticalProfileLatched')
      .distinct()
      .listen((s) => this.latchSymbolVisibility.set(s ? 'visible' : 'hidden'));

    this.sub
      .topic('targetAltitude')
      .limit(1000 / 60)
      .listen((targetAltitude) => {
        this.flightPathAltitude = targetAltitude;
      });
  }

  render(): ViewNode {
    return (
      <g id="LinearDeviationIndicator" ref={this.linearDevRef} display={this.isVisible}>
        <text
          ref={this.upperLinearDevTextRef}
          visibility={this.upperLinearDeviationReadoutVisibility}
          x="493"
          y="186"
          class="FontSmallest Green"
        >
          {this.upperLinearDeviationReadoutText}
        </text>
        <g id="LinearDeviationDot" transform={this.componentTransform}>
          <path
            id="EntireDot"
            visibility={this.linearDeviationDotVisibility}
            d="m534.881 362.37a6.777 6.78 0 1 0 -13.554 0 6.777 6.78 0 1 0 13.554 0z"
            class="Fill Green"
          />
          <path
            id="DotUpperHalf"
            visibility={this.linearDeviationDotUpperHalfVisibility}
            d="m521.336 362.37c0.003 -3.743 3.035 -6.777 6.777 -6.777 3.741 0 6.775 3.034 6.777 6.777h-6.777z"
            class="Fill Green"
          />
          <path
            id="DotLowerHalf"
            visibility={this.linearDeviationDotLowerHalfVisibility}
            d="m521.336 362.37c0.003 3.743 3.035 6.777 6.777 6.777 3.741 0 6.775 -3.034 6.777 -6.777h-6.777z"
            class="Fill Green"
          />
          <path visibility={this.latchSymbolVisibility} d="m106.743 70.235h-2.691v4.485h2.691 " class="Magenta" />
        </g>
        <text
          ref={this.lowerLinearDevTextRef}
          visibility={this.lowerLinearDeviationReadoutVisibility}
          x="493"
          y="556"
          class="FontSmallest Green"
        >
          {this.lowerLinearDeviationReadoutText}
        </text>
      </g>
    );
  }

  private pixelOffsetFromDeviation(deviation: number) {
    const sign = Math.sign(deviation);
    if (this.crosswindMode) {
      return (
        sign *
        Math.min((Math.abs(deviation) * 120) / DisplayRange, ((DisplayRange - 50) / ValueSpacing) * DistanceSpacing)
      );
    } else {
      return (
        sign *
        Math.min((Math.abs(deviation) * 220) / DisplayRange, ((DisplayRange - 50) / ValueSpacing) * DistanceSpacing)
      );
    }
  }
}

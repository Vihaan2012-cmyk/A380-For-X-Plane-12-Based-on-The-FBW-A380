import { View, Bus, UI, Ref, Readable, ViewNode } from 'xplane-js/ui';
import { HudElems } from './HUDUtils';
interface VerticalTapeProps {
  displayRange: number;
  valueSpacing: number;
  distanceSpacing: number;
  tapeValue: Readable<number>;
  lowerLimit: number;
  upperLimit: number;
  type: 'altitude' | 'speed';
}

export class VerticalTape extends View<VerticalTapeProps> {
  private crosswindMode = false;
  private bus = new Bus();
  private refElement = UI.ref<SVGGElement>();

  private tickRefs: Ref<SVGGElement>[] = [];

  private buildSpeedGraduationPoints(): Ref<SVGGElement>[] {
    const numTicks = Math.round((this.props.displayRange * 2) / this.props.valueSpacing);

    const clampedValue = Math.max(Math.min(this.props.tapeValue.get(), this.props.upperLimit), this.props.lowerLimit);

    let lowestValue = Math.max(
      Math.round((clampedValue - this.props.displayRange) / this.props.valueSpacing) * this.props.valueSpacing,
      this.props.lowerLimit,
    );
    if (lowestValue < this.props.tapeValue.get() - this.props.displayRange) {
      lowestValue += this.props.valueSpacing;
    }

    const graduationPoints = [];

    for (let i = 0; i < numTicks; i++) {
      const elementValue = lowestValue + i * this.props.valueSpacing;
      if (elementValue <= (this.props.upperLimit ?? Infinity)) {
        const offset = (-elementValue * this.props.distanceSpacing) / this.props.valueSpacing;
        const element = { elementValue, offset };
        if (element) {
          let text = '';
          if (elementValue % 20 === 0) {
            text = Math.abs(elementValue).toString().padStart(3, '0');
          }

          const tickRef = UI.ref<SVGGElement>();
          graduationPoints.push(
            <g ref={tickRef} transform={`translate(0 ${offset})`}>
              <path class="NormalStroke Green" d="m85.354 362.469h-12.65" />
              <text class="FontMedium MiddleAlign Green" x="42.6075" y="372.255">
                {text}
              </text>
            </g>,
          );
          this.tickRefs.push(tickRef);
        }
      }
    }
    return graduationPoints;
  }

  private buildAltitudeGraduationPoints(): Ref<SVGGElement>[] {
    const numTicks = Math.round((this.props.displayRange * 2) / this.props.valueSpacing);

    const clampedValue = Math.max(Math.min(this.props.tapeValue.get(), this.props.upperLimit), this.props.lowerLimit);

    let lowestValue = Math.max(
      Math.round((clampedValue - this.props.displayRange) / this.props.valueSpacing) * this.props.valueSpacing,
      this.props.lowerLimit,
    );
    if (lowestValue < this.props.tapeValue.get() - this.props.displayRange) {
      lowestValue += this.props.valueSpacing;
    }

    const graduationPoints = [];

    for (let i = 0; i < numTicks; i++) {
      const elementValue = lowestValue + i * this.props.valueSpacing;
      if (elementValue <= (this.props.upperLimit ?? Infinity)) {
        const offset = (-elementValue * this.props.distanceSpacing) / this.props.valueSpacing;
        const element = { elementValue, offset };
        if (element) {
          let text = '';
          if (elementValue % 500 === 0) {
            text = (Math.abs(elementValue) / 100).toString().padStart(3, '0');
          }
          const tickRef = UI.ref<SVGGElement>();

          graduationPoints.push(
            <g ref={tickRef} transform={`translate(0 ${offset}`}>
              <path class="NormalStroke Green" d="m586.862 362.473h-9.035" />
              <text class="FontMedium MiddleAlign Green" x="552.9478461" y="370.6406691">
                {text}
              </text>
            </g>,
          );
          this.tickRefs.push(tickRef);
        }
      }
    }
    return graduationPoints;
  }

  afterRender(node: ViewNode): void {
    super.afterRender(node);

    const sub = this.bus.inbox<HudElems>();

    sub.topic('cWndMode').listen((value) => {
      this.crosswindMode = value;
      if (this.props.type === 'altitude') {
        if (this.crosswindMode) {
          for (let i = 0; i < this.tickRefs.length - 1; i++) {
            this.tickRefs[i].target.getElementsByTagName('text')[0].classList.add('HiddenElement');
          }
        } else {
          for (let i = 0; i < this.tickRefs.length - 1; i++) {
            if ((parseInt(this.tickRefs[i].target.textContent as string) * 100) % 500 === 0) {
              this.tickRefs[i].target.getElementsByTagName('text')[0].classList.remove('HiddenElement');
            }
          }
        }
      }
    });

    this.props.tapeValue.watch((newValue) => {
      const multiplier = 100;
      const currentValueAtPrecision = Math.round(newValue * multiplier) / multiplier;
      const clampedValue = Math.max(
        Math.min(currentValueAtPrecision, this.props.upperLimit ?? Infinity),
        this.props.lowerLimit ?? -Infinity,
      );

      let lowestValue = Math.max(
        Math.round((clampedValue - this.props.displayRange) / this.props.valueSpacing) * this.props.valueSpacing,
        this.props.lowerLimit,
      );
      if (lowestValue < currentValueAtPrecision - this.props.displayRange) {
        lowestValue += this.props.valueSpacing;
      }

      for (let i = 0; i < this.tickRefs.length - 1; i++) {
        const elementValue = lowestValue + i * this.props.valueSpacing;
        if (elementValue <= (this.props.upperLimit ?? Infinity)) {
          const offset = (-elementValue * this.props.distanceSpacing) / this.props.valueSpacing;
          const element = { elementValue, offset };
          if (element) {
            this.tickRefs[i].target.setAttribute('transform', `translate(0 ${offset})`);

            let text = '';
            if (this.props.type === 'speed') {
              if (elementValue % 20 === 0) {
                text = Math.abs(elementValue).toString().padStart(3, '0');
              }
            } else if (this.props.type === 'altitude') {
              if (elementValue % 500 === 0) {
                text = (Math.abs(elementValue) / 100).toString().padStart(3, '0');
              }
            }

            if (this.tickRefs[i].target.getElementsByTagName('text')[0].textContent !== text) {
              this.tickRefs[i].target.getElementsByTagName('text')[0].textContent = text;
            }
          }
        }
      }

      this.refElement.target.style.transform = `translate3d(0px, ${(clampedValue * this.props.distanceSpacing) / this.props.valueSpacing}px, 0px)`;
    }, true);
  }

  render(): ViewNode {
    return (
      <g ref={this.refElement}>
        {this.props.type === 'altitude' && this.buildAltitudeGraduationPoints()}
        {this.props.type === 'speed' && this.buildSpeedGraduationPoints()}
        {this.props.children}
      </g>
    );
  }
}

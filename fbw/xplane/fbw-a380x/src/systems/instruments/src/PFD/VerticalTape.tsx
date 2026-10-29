import { View, UI, Ref, Readable, ViewNode } from 'xplane-js/ui';

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
              <path class="NormalStroke White" d="m19.031 80.818h-2.8206" />
              <text class="FontIntermediate MiddleAlign White" x="9.5348943" y="82.936722">
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
              <path class="NormalStroke White HiddenElement" d="m115.79 81.889 1.3316-1.0783-1.3316-1.0783" />
              <path class="NormalStroke White" d="m130.85 80.819h-2.0147" />
              <text class="FontMedium MiddleAlign White" x="123.28826" y="82.64006">
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
                this.tickRefs[i].target.getElementsByTagName('path')[0].classList.remove('HiddenElement');
              } else {
                this.tickRefs[i].target.getElementsByTagName('path')[0].classList.add('HiddenElement');
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

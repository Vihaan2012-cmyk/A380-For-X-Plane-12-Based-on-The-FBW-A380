import { Bus, View, UI, Ref, ViewNode, Readable } from 'xplane-js/ui';
import { Arinc429Values } from './shared/ArincValueProvider';

interface HorizontalTapeProps {
  displayRange: number;
  valueSpacing: number;
  distanceSpacing: number;
  type: 'horizon' | 'headingTape';
  bus: Bus;
  yOffset?: Readable<number>;
}
export class HorizontalTape extends View<HorizontalTapeProps> {
  private refElement = UI.ref<SVGGElement>();

  private tapeOffset = 0;

  private tickNumberRefs: Ref<SVGTextElement>[] = [];

  private currentDrawnHeading = 0;

  private yOffset = 0;

  private buildHorizonTicks(): { ticks: SVGPathElement[]; labels?: SVGTextElement[] } {
    const result = { ticks: [] as SVGPathElement[], labels: [] as SVGTextElement[] };

    result.ticks.push(<path transform="translate(0 0)" class="NormalStroke White" d="m68.906 80.823v1.8" />);

    for (let i = 0; i < 6; i++) {
      const headingOffset = (1 + i) * this.props.valueSpacing;
      const dX = (this.props.distanceSpacing / this.props.valueSpacing) * headingOffset;

      if (headingOffset % 10 === 0) {
        result.ticks.push(<path class="NormalStroke White" d="m68.906 80.823v1.8" transform={`translate(${dX} 0)`} />);
        result.ticks.unshift(
          <path class="NormalStroke White" d="m68.906 80.823v1.8" transform={`translate(${-dX} 0)`} />,
        );
      }
    }

    return result;
  }

  private buildHeadingTicks(): { ticks: SVGLineElement[]; labels: SVGTextElement[] } {
    const result = {
      ticks: [] as SVGLineElement[],
      labels: [] as SVGTextElement[],
    };

    const tickLength = 4;
    let textRef = UI.ref<SVGTextElement>();

    result.ticks.push(
      <path class="NormalStroke White" d={`m68.913 145.34v${tickLength}`} transform="translate(0 0)" />,
    );

    result.labels.push(
      <text
        id="HeadingLabel"
        class="White MiddleAlign FontMedium"
        ref={textRef}
        x="68.979425"
        y="154.64206"
        transform={`translate(${0} 0)`}
      >
        360
      </text>,
    );
    this.tickNumberRefs.push(textRef);

    for (let i = 0; i < 6; i++) {
      const headingOffset = (1 + i) * this.props.valueSpacing;
      const dX = (this.props.distanceSpacing / this.props.valueSpacing) * headingOffset;

      if (headingOffset % 10 === 0) {
        result.ticks.push(
          <path class="NormalStroke White" d={`m68.913 145.34v${tickLength}`} transform={`translate(${dX} 0)`} />,
        );
        result.ticks.unshift(
          <path class="NormalStroke White" d={`m68.913 145.34v${tickLength}`} transform={`translate(${-dX} 0)`} />,
        );
      } else {
        result.ticks.push(
          <path
            class="NormalStroke White"
            d={`m68.913 145.34v${tickLength * 0.42}`}
            transform={`translate(${dX} 0)`}
          />,
        );
        result.ticks.unshift(
          <path
            class="NormalStroke White"
            d={`m68.913 145.34v${tickLength * 0.42}`}
            transform={`translate(${-dX} 0)`}
          />,
        );
      }

      if (headingOffset % 10 === 0) {
        textRef = UI.ref<SVGTextElement>();

        result.labels.unshift(
          <text
            id="HeadingLabel"
            class={`White MiddleAlign ${i % 3 === 0 ? 'FontSmallest' : 'FontMedium'}`}
            ref={textRef}
            x="68.979425"
            y="154.64206"
            transform={`translate(${-dX} 0)`}
          >
            {headingOffset}
          </text>,
        );
        this.tickNumberRefs.unshift(textRef);
        textRef = UI.ref<SVGTextElement>();
        result.labels.push(
          <text
            id="HeadingLabel"
            class={`White MiddleAlign ${i % 3 === 0 ? 'FontSmallest' : 'FontMedium'}`}
            ref={textRef}
            x="68.979425"
            y="154.64206"
            transform={`translate(${dX} 0)`}
          >
            {360 - headingOffset}
          </text>,
        );
        this.tickNumberRefs.push(textRef);
      }
    }

    return result;
  }

  afterRender(node: ViewNode): void {
    super.afterRender(node);

    const pf = this.props.bus.inbox<Arinc429Values>();

    this.props.yOffset?.watch((yOffset) => {
      this.yOffset = yOffset;
      this.refElement.target.style.transform = `translate3d(${this.tapeOffset}px, ${yOffset}px, 0px)`;
    });

    pf.topic('headingAr').listen((newVal) => {
      const multiplier = 100;
      const currentValueAtPrecision = Math.round(newVal.value * multiplier) / multiplier;
      const tapeOffset = ((-currentValueAtPrecision % 10) * this.props.distanceSpacing) / this.props.valueSpacing;

      if (
        currentValueAtPrecision / 10 >= this.currentDrawnHeading + 1 ||
        currentValueAtPrecision / 10 <= this.currentDrawnHeading
      ) {
        this.currentDrawnHeading = Math.floor(currentValueAtPrecision / 10);

        const start = 330 + this.currentDrawnHeading * 10;

        this.tickNumberRefs.forEach((t, index) => {
          const scrollerValue = t.target;
          if (scrollerValue !== null) {
            const hdg = (start + index * 10) % 360;
            if (hdg % 10 === 0) {
              const content = hdg !== 0 ? (hdg / 10).toFixed(0) : '0';
              if (scrollerValue.textContent !== content) {
                scrollerValue.textContent = content;
              }
            } else {
              scrollerValue.textContent = '';
            }
            if (hdg % 30 === 0) {
              scrollerValue.classList.remove('FontSmallest');
              scrollerValue.classList.add('FontMedium');
            } else {
              scrollerValue.classList.add('FontSmallest');
              scrollerValue.classList.remove('FontMedium');
            }
          }
        });
      }
      this.tapeOffset = tapeOffset;

      this.refElement.target.style.transform = `translate3d(${tapeOffset}px, ${this.yOffset}px, 0px)`;
    });
  }

  render(): ViewNode {
    const tapeContent = this.props.type === 'horizon' ? this.buildHorizonTicks() : this.buildHeadingTicks();

    return (
      <g id="HeadingTick" ref={this.refElement}>
        {tapeContent.ticks}
        {this.props.type === 'headingTape' && tapeContent.labels}
      </g>
    );
  }
}

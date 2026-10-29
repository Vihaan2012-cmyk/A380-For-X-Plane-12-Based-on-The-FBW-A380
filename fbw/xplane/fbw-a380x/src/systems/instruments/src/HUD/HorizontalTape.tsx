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

  private buildHeadingTicks(): { ticks: SVGLineElement[]; labels: SVGTextElement[] } {
    const result = {
      ticks: [] as SVGLineElement[],
      labels: [] as SVGTextElement[],
    };

    const tickLength = 5;
    let textRef = UI.ref<SVGTextElement>();

    result.ticks.push(<path class="NormalStroke Green" d={`m640 512 v${tickLength}`} transform="translate(0 0)" />);

    result.labels.push(
      <text
        id="HeadingLabel"
        class="Green MiddleAlign FontTiny"
        ref={textRef}
        x="640"
        y="545"
        transform={`translate(${0} 0)`}
      >
        0
      </text>,
    );
    this.tickNumberRefs.push(textRef);

    for (let i = 0; i < 6; i++) {
      const headingOffset = (1 + i) * this.props.valueSpacing;
      const dX = (this.props.distanceSpacing / this.props.valueSpacing) * headingOffset;

      result.ticks.push(
        <path class="NormalStroke Green" d={`m640 512 v${tickLength}`} transform={`translate(${dX} 0)`} />,
      );
      result.ticks.unshift(
        <path class="NormalStroke Green" d={`m640 512 v${tickLength}`} transform={`translate(${-dX} 0)`} />,
      );

      if (headingOffset % 10 === 0) {
        textRef = UI.ref<SVGTextElement>();

        result.labels.unshift(
          <text
            id="HeadingLabel"
            class={`Green MiddleAlign FontTiny`}
            ref={textRef}
            x="640"
            y="545"
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
            class={`Green MiddleAlign FontTiny`}
            ref={textRef}
            x="640"
            y="545"
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
              const content = hdg !== 0 ? hdg / 10 : 0;
              const text = content < 10 ? '0' + content.toFixed(0) : content.toFixed(0);
              if (scrollerValue.textContent !== text) {
                scrollerValue.textContent = text;
              }
            } else {
              scrollerValue.textContent = '';
            }
            // if (hdg % 30 === 0) {
            //   scrollerValue.classList.remove('FontSmallest');
            //   scrollerValue.classList.add('FontMedium');
            // } else {
            //   scrollerValue.classList.add('FontSmallest');
            //   scrollerValue.classList.remove('FontMedium');
            // }
          }
        });
      }
      this.tapeOffset = tapeOffset;

      this.refElement.target.style.transform = `translate3d(${tapeOffset}px, ${this.yOffset}px, 0px)`;
    });
  }

  render(): ViewNode {
    const tapeContent = this.buildHeadingTicks();

    return (
      <g id="HeadingTick" ref={this.refElement}>
        {tapeContent.ticks}
        {this.props.type === 'headingTape' && tapeContent.labels}
      </g>
    );
  }
}

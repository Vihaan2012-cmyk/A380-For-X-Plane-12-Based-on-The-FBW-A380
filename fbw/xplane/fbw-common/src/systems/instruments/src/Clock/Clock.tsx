import { ViewProps, View, Bus, UI, ViewNode } from 'xplane-js/ui';
import { ClockSimvars } from './shared/ClockSimvarPublisher';
import { Chrono } from './Components/Chrono';
import { Clock } from './Components/Clock';
import { ElapsedTime } from './Components/ElapsedTime';

interface ClockProps extends ViewProps {
  bus: Bus;
}

export class ClockRoot extends View<ClockProps> {
  private readonly gElementRef = UI.ref<SVGGElement>();

  private readonly svgElementRef = UI.ref<SVGSVGElement>();

  afterRender(node: ViewNode): void {
    super.afterRender(node);

    const sub = this.props.bus.inbox<ClockSimvars>();
    sub
      .topic('dcEssIsPowered')
      .distinct()
      .listen((dccEssIsPowered) => {
        this.svgElementRef.target.classList.toggle('powered', dccEssIsPowered);
        this.svgElementRef.target.classList.toggle('unpowered', !dccEssIsPowered);
      });

    sub
      .topic('timeOfDay')
      .distinct()
      .listen((timeOfDay) => {
        this.gElementRef.target.classList.toggle('day', timeOfDay === 1 || timeOfDay === 2);
        this.gElementRef.target.classList.toggle('night', !(timeOfDay === 1 || timeOfDay === 2));
      });
  }

  render(): ViewNode {
    return (
      <svg ref={this.svgElementRef} version="1.1" viewBox="0 0 256 256" xmlns="http://www.w3.org/2000/svg">
        <g ref={this.gElementRef}>
          <Chrono bus={this.props.bus} />
          <Clock bus={this.props.bus} />
          <ElapsedTime bus={this.props.bus} />
        </g>
      </svg>
    );
  }
}

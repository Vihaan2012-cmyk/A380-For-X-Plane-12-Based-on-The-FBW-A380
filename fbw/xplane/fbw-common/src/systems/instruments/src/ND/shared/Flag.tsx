import { View, UI, Readable, ViewNode } from 'xplane-js/ui';

export interface FlagProps {
  x: Readable<number> | number;
  y: Readable<number> | number;
  class: string;
  visible: Readable<boolean>;
}

export class Flag extends View<FlagProps> {
  private readonly flagRef = UI.ref<SVGTextElement>();

  afterRender(node: ViewNode) {
    super.afterRender(node);

    this.props.visible.watch((visible) => {
      this.flagRef.target.style.visibility = visible ? 'inherit' : 'hidden';
    }, true);
  }

  render(): ViewNode | null {
    return (
      <text x={this.props.x} y={this.props.y} ref={this.flagRef} class={`${this.props.class} MiddleAlign shadow`}>
        {this.props.children}
      </text>
    );
  }
}

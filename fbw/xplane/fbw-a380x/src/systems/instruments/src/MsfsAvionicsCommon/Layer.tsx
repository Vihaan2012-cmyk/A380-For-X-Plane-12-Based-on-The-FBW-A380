import { UI, View, ViewNode, Readable, Derived, ViewProps } from 'xplane-js/ui';

export interface LayerProps extends ViewProps {
  x: number | Readable<number>;
  y: number | Readable<number>;
  visible?: Readable<boolean>;
}

export class Layer extends View<LayerProps> {
  render(): ViewNode | null {
    const { x, y } = this.props;
    const ref = this.props.ref ?? UI.ref();

    let value: Readable<string> | string;
    if (typeof x !== 'number' && typeof y !== 'number') {
      value = Derived.from(([x, y]) => `translate(${x}, ${y})`, x, y);
    } else if (typeof x !== 'number' || typeof y !== 'number') {
      throw new Error('Both attributes of Layer must be of the same type (number or Subscribable)');
    } else {
      value = `translate(${x}, ${y})`;
    }

    return (
      <g
        ref={ref}
        transform={value}
        visibility={this.props.visible?.derive((v) => (v ? 'inherit' : 'hidden')) ?? 'inherit'}
      >
        {this.props.children}
      </g>
    );
  }
}

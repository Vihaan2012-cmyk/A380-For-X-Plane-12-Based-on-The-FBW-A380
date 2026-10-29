//  Copyright (c) 2024-2025 FlyByWire Simulations
//  SPDX-License-Identifier: GPL-3.0

import { ViewProps, View, UI, Readable, Bond, ViewNode } from 'xplane-js/ui';

interface ConditionalComponentProps extends ViewProps {
  componentIfTrue: ViewNode;
  componentIfFalse: ViewNode;
  condition: Readable<boolean>;
  width?: number; // Fixed width in pixel. If not set, positioning might not be consistent
  height?: number; // Fixed height in pixels. If not set, positioning might not be consistent
  noStyle?: boolean; // If true, the component will not be styled (display: flex, justify-content: center, align-items: center)
}

/*
 * Renders components based on the prop's condition (either componentIfTrue or componentIfFalse is rendered)
 */
export class ConditionalComponent extends View<ConditionalComponentProps> {
  // Make sure to collect all subscriptions here, otherwise page navigation doesn't work.
  private readonly subs = [] as Bond[];

  private readonly containerRef = UI.ref<HTMLDivElement>();

  private readonly trueComponentRef = UI.ref<HTMLDivElement>();

  private readonly falseComponentRef = UI.ref<HTMLDivElement>();

  public afterRender(node: ViewNode): void {
    super.afterRender(node);

    this.subs.push(
      this.props.condition.watch((v) => {
        this.trueComponentRef.target.style.display = v ? 'block' : 'none';
        this.falseComponentRef.target.style.display = v ? 'none' : 'block';
      }, true),
    );
  }

  public destroy(): void {
    // Destroy all subscriptions to remove all references to this instance.
    this.subs.forEach((x) => x.destroy());

    super.destroy();
  }

  render(): ViewNode {
    return (
      <div
        ref={this.containerRef}
        style={`${this.props.noStyle ? '' : 'display: flex; justify-content: center; align-items: center;'}
                ${this.props.width !== undefined ? ` width: ${this.props.width.toFixed(0)}px;` : ''}
                ${this.props.height !== undefined ? ` height: ${this.props.height.toFixed(0)}px;` : ''}`}
      >
        <div ref={this.trueComponentRef}>{this.props.componentIfTrue}</div>
        <div ref={this.falseComponentRef}>{this.props.componentIfFalse}</div>
      </div>
    );
  }
}

//  Copyright (c) 2024-2025 FlyByWire Simulations
//  SPDX-License-Identifier: GPL-3.0

import {
  ViewProps,
  View,
  UI,
  Derived,
  Signal,
  Readable,
  Bond,
  ViewNode,
} from 'xplane-js/ui';

export interface AdscButtonProps extends ViewProps {
  /** True = Upper state; False = Lower state */
  state: Readable<boolean>;
  /** Upper label */
  labelTrue: string;
  /** Lower label */
  labelFalse: string;
  disabled?: Readable<boolean>;
  buttonStyle?: string;
  onChanged(val: boolean): void;
}

/*
 * Button for MFD pages. If menuItems is set, a dropdown menu will be displayed when button is clicked
 */
export class AdscButton extends View<AdscButtonProps> {
  // Make sure to collect all subscriptions here, otherwise page navigation doesn't work.
  private readonly subs = [] as Bond[];

  private readonly topRef = UI.ref<HTMLDivElement>();

  private readonly upperLabelGreen = Derived.from(
    ([state, disabled]) => state && !disabled,
    this.props.state,
    this.props.disabled ?? Signal.of(false),
  );

  private readonly upperLabelHidden = Derived.from(
    ([state, disabled]) => !state && disabled,
    this.props.state,
    this.props.disabled ?? Signal.of(false),
  );

  private readonly lowerLabelGreen = Derived.from(
    ([state, disabled]) => !state && !disabled,
    this.props.state,
    this.props.disabled ?? Signal.of(false),
  );

  private readonly lowerLabelHidden = Derived.from(
    ([state, disabled]) => state && disabled,
    this.props.state,
    this.props.disabled ?? Signal.of(false),
  );

  private onClick() {
    if (!this.props.disabled?.get()) {
      this.props.onChanged(!this.props.state.get());
    }
  }

  private onClickHandler = this.onClick.bind(this);

  public afterRender(node: ViewNode): void {
    super.afterRender(node);

    if (this.props.disabled === undefined) {
      this.props.disabled = Signal.of(false);
    }
    this.topRef.target.addEventListener('click', this.onClickHandler);

    this.subs.push(
      this.props.disabled.watch((val) => {
        if (val) {
          this.topRef.peek()?.classList.add('disabled');
        } else {
          this.topRef.peek()?.classList.remove('disabled');
        }
      }, true),
    );
  }

  public destroy(): void {
    // Destroy all subscriptions to remove all references to this instance.
    this.subs.forEach((x) => x.destroy());

    this.topRef.peek()?.removeEventListener('click', this.onClickHandler);

    super.destroy();
  }

  public render(): ViewNode {
    return (
      <div class={{ 'mfd-adsc-button': true, disabled: this.props.disabled ?? false }} ref={this.topRef}>
        <div
          class={{
            'mfd-adsc-label-off': this.upperLabelGreen.derive((it) => !it),
            'mfd-value': this.upperLabelGreen,
            bigger: this.upperLabelGreen,
          }}
          style={{ visibility: this.upperLabelHidden.derive((it) => (it ? 'hidden' : 'visible')) }}
        >
          {this.props.labelTrue}
        </div>
        <div
          class={{
            'mfd-adsc-label-off': this.lowerLabelGreen.derive((it) => !it),
            'mfd-label': this.lowerLabelGreen,
            bigger: this.lowerLabelGreen,
          }}
          style={{ visibility: this.lowerLabelHidden.derive((it) => (it ? 'hidden' : 'visible')) }}
        >
          {this.props.labelFalse}
        </div>
      </div>
    );
  }
}

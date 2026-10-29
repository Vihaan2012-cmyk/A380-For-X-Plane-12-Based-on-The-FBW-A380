//  Copyright (c) 2024-2025 FlyByWire Simulations
//  SPDX-License-Identifier: GPL-3.0

import {
  ViewProps,
  View,
  UI,
  Writable,
  Signal,
  Readable,
  Readables,
  Bond,
  ViewNode,
} from 'xplane-js/ui';

export enum RadioButtonColor {
  Yellow,
  Green,
  Amber,
  Cyan,
  White,
}

interface RadioButtonGroupProps extends ViewProps {
  values: string[];
  valuesDisabled?: Readable<boolean[]>;
  selectedIndex: Readable<number | null> | Writable<number | null>;
  idPrefix: string;
  /** If this function is defined, selectedIndex is not automatically updated. This function should take care of that. */
  onModified?: (newSelectedIndex: number) => void;
  additionalVerticalSpacing?: number;
  color?: Readable<RadioButtonColor>;
}
export class RadioButtonGroup extends View<RadioButtonGroupProps> {
  // Make sure to collect all subscriptions here, otherwise page navigation doesn't work.
  private subs = [] as Bond[];

  private readonly labelRefs = Array.from(Array(this.props.values.length), () =>
    UI.ref<HTMLLabelElement>(),
  );
  private readonly inputRefs = Array.from(Array(this.props.values.length), () =>
    UI.ref<HTMLInputElement>(),
  );

  private changeEventHandler(i: number) {
    if (this.props.onModified) {
      this.props.onModified(i);
    } else if (Readables.isWritable(this.props.selectedIndex)) {
      this.props.selectedIndex.set(i);
    }
  }

  afterRender(node: ViewNode): void {
    super.afterRender(node);

    if (this.props.color === undefined) {
      this.props.color = Signal.of<RadioButtonColor>(RadioButtonColor.Cyan);
    }
    if (this.props.valuesDisabled === undefined) {
      this.props.valuesDisabled = Signal.of<boolean[]>(this.props.values.map(() => false));
    }

    this.inputRefs.forEach((ref, index) =>
      ref.target.addEventListener('change', this.changeEventHandler.bind(this, index)),
    );

    this.subs.push(
      this.props.selectedIndex.watch((val) => {
        this.inputRefs.forEach((ref, index) => {
          if (index === val) {
            ref.target.checked = true;
          } else {
            ref.target.checked = false;
          }
        });
      }, true),
    );

    this.subs.push(
      this.props.valuesDisabled.watch((val) => {
        this.inputRefs.forEach((ref, index) => {
          if (val[index]) {
            ref.target.setAttribute('disabled', 'disabled');
          } else {
            ref.target.removeAttribute('disabled');
          }
        });
      }, true),
    );

    this.subs.push(
      this.props.color.watch((v) => {
        this.labelRefs.forEach((ref) => {
          ref.target.classList.toggle('yellow', v === RadioButtonColor.Yellow);
          ref.target.classList.toggle('green', v === RadioButtonColor.Green);
          ref.target.classList.toggle('amber', v === RadioButtonColor.Amber);
          ref.target.classList.toggle('cyan', v === RadioButtonColor.Cyan);
          ref.target.classList.toggle('white', v === RadioButtonColor.White);
        });
      }, true),
    );
  }

  public destroy(): void {
    // Destroy all subscriptions to remove all references to this instance.
    this.subs.forEach((x) => x.destroy());

    this.inputRefs.forEach((ref, index) =>
      ref.target.removeEventListener('change', this.changeEventHandler.bind(this, index)),
    );

    super.destroy();
  }

  render(): ViewNode {
    return (
      <form>
        {this.props.values.map((el, idx) => (
          <label
            ref={this.labelRefs[idx]}
            class="mfd-radio-button"
            htmlFor={`${this.props.idPrefix}_${idx}`}
            style={this.props.additionalVerticalSpacing ? `margin-top: ${this.props.additionalVerticalSpacing}px;` : ''}
            id={`${this.props.idPrefix}_label_${idx}`}
          >
            <input
              ref={this.inputRefs[idx]}
              type="radio"
              name={`${this.props.idPrefix}`}
              id={`${this.props.idPrefix}_${idx}`}
            />
            <span>{el}</span>
          </label>
        ))}
      </form>
    );
  }
}

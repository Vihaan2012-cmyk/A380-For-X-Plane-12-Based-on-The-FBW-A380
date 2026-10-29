//  Copyright (c) 2024-2026 FlyByWire Simulations
//  SPDX-License-Identifier: GPL-3.0

import {
  SignalList,
  ViewProps,
  Stream,
  View,
  UI,
  Derived,
  Writable,
  Signal,
  Readable,
  ReadableList,
  Readables,
  Bond,
  ViewNode,
} from 'xplane-js/ui';
import { InputField, InteractionMode } from './InputField';
import { DropdownFieldFormat } from '../../MFD/pages/common/DataEntryFormats';
import { EfisSide } from '@flybywiresim/fbw-sdk';

interface DropdownMenuProps extends ViewProps {
  values: ReadableList<string>;
  selectedIndex: Readable<number | null> | Writable<number | null>;
  freeTextAllowed?: boolean;
  idPrefix: string;
  /** If defined, this component does not update the selectedIndex prop by itself, but rather calls this method. */
  onModified?: (newSelectedIndex: number | null, freeTextEntry: string) => void;
  inactive?: Readable<boolean>;
  disabled?: Readable<boolean>;
  containerStyle?: string;
  alignLabels?: 'flex-start' | 'center' | 'flex-end' | Readable<'flex-start' | 'center' | 'flex-end'>;
  /** Defined by the width of the component */
  numberOfDigitsForInputField?: number;
  tmpyActive?: Readable<boolean>;
  /** Only handles KCCU input for respective side, receives key name only */
  hEventConsumer: Stream<[EfisSide, string]>;
  /** Kccu uses the HW keys, and doesn't focus input fields */
  interactionMode: Readable<InteractionMode>;

  /**
   * Indicates whether the user is allowed to type a dropdown value rather than selecting from the list.
   */
  keyboardEntryAllowed?: boolean;

  /** Callback invoked when a text entry is not in the list of values. */
  errorOnNotInList?: () => void;
}

/*
 * Dropdown menu with optional free text entry (with black background, and cyan font color)
 */
export class DropdownMenu extends View<DropdownMenuProps> {
  // Make sure to collect all subscriptions here, otherwise page navigation doesn't work.
  private readonly subs = [] as Bond[];

  private readonly topRef = UI.ref<HTMLDivElement>();

  private readonly dropdownSelectorRef = UI.ref<HTMLDivElement>();

  private readonly dropdownMenuRef = UI.ref<HTMLDivElement>();

  private readonly dropdownIsOpened = Signal.of(false);

  private readonly dropdownMenuDisplay = this.dropdownIsOpened.derive((opened) => (opened ? 'block' : 'none'));

  private readonly inputFieldRef = UI.ref<InputField<string>>();

  private readonly inputFieldValue = Signal.of<string | null>('');

  private readonly inactive = Readables.toReadable(this.props.inactive ?? Signal.of(false), true);

  private readonly disabled = Readables.toReadable(this.props.disabled ?? Signal.of(false), true);

  private readonly tmpyActive = Readables.toReadable(this.props.tmpyActive ?? Signal.of(false), true);

  private readonly disabledWhenActive = Derived.from(
    ([inactive, disabled]) => !inactive && disabled,
    this.inactive,
    this.disabled,
  );

  private readonly dropdownArrowFill = this.disabled.derive((isDisabled) => (isDisabled ? 'gray' : 'white'));

  private freeTextEntered = false;

  private readonly renderedDropdownOptions = SignalList.of<string>();

  private renderedDropdownOptionsIndices: number[] = [];

  private onDropdownOpenedCallback: (() => void | undefined) | undefined = undefined;

  private readonly alignTextSub: Readable<'flex-start' | 'center' | 'flex-end'> = Readables.toReadable(
    this.props.alignLabels ?? 'center',
    true,
  );

  private onClick(i: number) {
    if (!this.inactive.get() && !this.disabled.get()) {
      this.freeTextEntered = false;
      if (this.props.onModified) {
        this.props.onModified(this.renderedDropdownOptionsIndices[i], '');
      } else if (Readables.isWritable(this.props.selectedIndex)) {
        this.props.selectedIndex.set(this.renderedDropdownOptionsIndices[i]);
      }
      this.dropdownIsOpened.set(false);
      this.filterList('');
    }
  }

  private onFieldSubmit(text: string) {
    if (!this.inactive.get() && !this.disabled.get()) {
      let error = false;

      if (this.props.keyboardEntryAllowed || this.props.freeTextAllowed) {
        if (
          text.length > 0 &&
          this.props.errorOnNotInList !== undefined &&
          !this.props.values.snapshot().includes(text)
        ) {
          this.props.errorOnNotInList();
          error = true;
        }
      }
      if (!error) {
        if (this.props.onModified !== undefined) {
          this.props.onModified(-1, text); // selected index of -1 marks free text entry
        }
      }

      this.freeTextEntered = true;
      this.inputFieldValue.set(error ? this.props.values.get(this.props.selectedIndex.get() ?? 0) : text);
      this.dropdownIsOpened.set(false);
      this.freeTextEntered = false;
      this.filterList('');
    }
  }

  private filterList(text: string) {
    const arr = this.props.values.snapshot();
    this.renderedDropdownOptionsIndices = arr.map((_, idx) => idx).filter((_, idx) => arr[idx].startsWith(text));
    this.renderedDropdownOptions.set(arr.filter((val) => val.startsWith(text)));
  }

  private onFieldChanged(text: string) {
    this.freeTextEntered = true;

    // Filter dropdown options based on input
    this.filterList(text);
  }

  public forceLabel(label: string) {
    this.inputFieldValue.set(label);
  }

  afterRender(node: ViewNode): void {
    super.afterRender(node);

    this.subs.push(
      this.renderedDropdownOptions.watch((_, __, ___, array) => {
        // Remove click handlers
        array.forEach((_, i) => {
          if (document.getElementById(`${this.props.idPrefix}_${i}`)) {
            document
              .getElementById(`${this.props.idPrefix}_${i}`)
              ?.removeEventListener('click', this.onClick.bind(this, i));
          }
        });

        // Re-draw all options
        while (this.dropdownMenuRef.target.firstChild) {
          this.dropdownMenuRef.target.removeChild(this.dropdownMenuRef.target.firstChild);
        }
        array.forEach((el, idx) => {
          const n: ViewNode = (
            <span
              id={`${this.props.idPrefix}_${idx}`}
              class="mfd-dropdown-menu-element"
              style={this.alignTextSub.derive((it) => `text-align: ${it};`)}
            >
              {el}
            </span>
          );
          UI.mount(n, this.dropdownMenuRef.target);
        }, this);

        // Add click handlers
        array.forEach((_, i) => {
          document.getElementById(`${this.props.idPrefix}_${i}`)?.addEventListener('click', this.onClick.bind(this, i));
        });
      }),
    );

    this.subs.push(
      this.props.values.watch((_, __, ___, array) => {
        const selIdx = this.props.selectedIndex.get();
        if (selIdx !== undefined && selIdx !== null) {
          this.inputFieldValue.set(array[selIdx]);
        } else {
          this.inputFieldValue.set('');
        }
        this.renderedDropdownOptionsIndices = array.map((_, idx) => idx);
        this.renderedDropdownOptions.set(array);
      }, true),
    );

    this.subs.push(
      this.props.selectedIndex.watch((value) => {
        this.inputFieldValue.set(value !== null && this.props.values.get(value) ? this.props.values.get(value) : null);
      }),
    );

    this.dropdownSelectorRef.target.addEventListener('click', this.onOpenCloseClickHandler);

    // Close dropdown menu if clicked outside
    document.getElementById('MFD_CONTENT')?.addEventListener('click', this.onClickedOutsideHandler);

    this.subs.push(
      this.dropdownIsOpened.watch((opened) => {
        this.onDropdownOpenedCallback?.();
        this.onDropdownOpenedCallback = undefined;

        if (!this.freeTextEntered) {
          if (opened) {
            this.inputFieldRef.target.textInputRef.target.focus();
            this.inputFieldRef.target.onFocus();
          } else {
            this.inputFieldRef.target.textInputRef.target.blur();
            this.inputFieldRef.target.onBlur(false);
          }
        }
      }),
    );

    this.subs.push(this.dropdownArrowFill, this.disabledWhenActive);
    // TODO add KCCU events
  }

  private onOpenCloseClick() {
    if (!this.inactive.get() && !this.disabled.get()) {
      this.dropdownIsOpened.set(!this.dropdownIsOpened.get());
    }
  }

  private onOpenCloseClickHandler = this.onOpenCloseClick.bind(this);

  private onClickedOutside(e: MouseEvent) {
    if (!this.topRef.peek()?.contains(e.target as Node) && this.dropdownIsOpened.get()) {
      if (this.inputFieldValue.get() === '') {
        this.inputFieldValue.set(null);
      }
      this.inputFieldRef.target.onBlur();
      this.dropdownIsOpened.set(false);
    }
  }

  private onClickedOutsideHandler = this.onClickedOutside.bind(this);

  /**
   * Scrolls the dropdown list to the given index
   *
   * @param index the index of the value to scroll to
   */
  public scrollToValue(index: number): void {
    this.onDropdownOpenedCallback = () => {
      const element = Array.from(this.dropdownMenuRef.target.children).find(
        (it) => it.id === `${this.props.idPrefix}_${index}`,
      );

      this.dropdownMenuRef.target.scrollTop = (element as unknown as { offsetTop: number }).offsetTop;
    };
  }

  public destroy(): void {
    // Destroy all subscriptions to remove all references to this instance.
    this.subs.forEach((x) => x.destroy());

    this.dropdownSelectorRef.peek()?.removeEventListener('click', this.onOpenCloseClickHandler);
    document.getElementById('MFD_CONTENT')?.removeEventListener('click', this.onClickedOutsideHandler);

    this.renderedDropdownOptions.clear();

    super.destroy();
  }

  render(): ViewNode {
    return (
      <div class="mfd-dropdown-container" ref={this.topRef} style={this.props.containerStyle}>
        <div
          ref={this.dropdownSelectorRef}
          class={{
            'mfd-dropdown-outer': true,
            inactive: this.inactive,
            disabled: this.disabledWhenActive,
          }}
        >
          <div
            class={{
              'mfd-dropdown-inner': true,
              inactive: this.inactive,
              disabled: this.disabledWhenActive,
            }}
            style={`justify-content: ${this.props.alignLabels};`}
          >
            <InputField<string>
              ref={this.inputFieldRef}
              dataEntryFormat={new DropdownFieldFormat(this.props.numberOfDigitsForInputField ?? 6)}
              value={this.inputFieldValue}
              containerStyle="border: 2px inset transparent"
              alignText={this.props.alignLabels}
              freeText={this.props.freeTextAllowed}
              onModified={(text) => this.onFieldSubmit(text ?? '')}
              onInput={(text) => this.onFieldChanged(text)}
              inactive={this.inactive}
              disabled={this.disabled}
              handleFocusBlurExternally
              tmpyActive={this.tmpyActive}
              hEventConsumer={this.props.hEventConsumer}
              interactionMode={this.props.interactionMode}
              errorHandler={() => {}}
              fixedValuesDropDown={!this.props.keyboardEntryAllowed && !this.props.freeTextAllowed}
            />
          </div>
          <div
            class={{
              'mfd-dropdown-arrow': true,
              inactive: this.inactive,
              disabled: this.disabledWhenActive,
            }}
          >
            <svg height="15" width="15">
              <polygon points="0,0 15,0 7.5,15" style={{ fill: this.dropdownArrowFill }} />
            </svg>
          </div>
        </div>
        <div ref={this.dropdownMenuRef} class="mfd-dropdown-menu" style={{ display: this.dropdownMenuDisplay }} />
      </div>
    );
  }
}

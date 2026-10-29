//  Copyright (c) 2024-2026 FlyByWire Simulations
//  SPDX-License-Identifier: GPL-3.0

import {
  ViewProps,
  View,
  UI,
  Signal,
  Readable,
  Logic,
  Readables,
  Bond,
  ViewNode,
} from 'xplane-js/ui';
import { TriangleDown, TriangleUp } from './shapes';

export type ButtonMenuItem = {
  label: string | Readable<string>;
  disabled?: boolean | Readable<boolean>;
  action(): void;
};

// FIXME these should all be immutable
export interface ButtonProps extends ViewProps {
  readonly label: string | ViewNode | Readable<string>;
  // FIXME should be SubscribableArray<ButtonMenuItem>... ButtonMenu should really be a separate component...
  readonly menuItems?: Readable<ButtonMenuItem[]>; // When defining menu items, idPrefix has to be set FIXME express that in the types
  /** Whether to show an arrow after the label for buttons with menu items. Defaults to true. */
  readonly showArrow?: boolean;
  idPrefix?: string;
  disabled?: boolean | Readable<boolean>;
  visible?: boolean | Readable<boolean>;
  selected?: Readable<boolean>; // Renders with lighter grey if selected (e.g. for segmented controls)
  highlighted?: Readable<boolean>;
  buttonStyle?: string | Readable<string>;
  containerStyle?: string;
  onClick: () => void;
  scrollToMenuItem?: Readable<number>;
  dropdownMenuRightAligned?: boolean;
}

/*
 * Button for MFD pages. If menuItems is set, a dropdown menu will be displayed when button is clicked
 */
export class Button extends View<ButtonProps> {
  // Make sure to collect all subscriptions here, otherwise page navigation doesn't work.
  private readonly subs = [] as Bond[];

  private readonly topRef = UI.ref<HTMLDivElement>();

  private readonly buttonRef = UI.ref<HTMLSpanElement>();

  private readonly dropdownMenuRef = UI.ref<HTMLDivElement>();

  private readonly dropdownIsOpened = Signal.of(false);

  private readonly menuOpensUpwards = Signal.of(false);

  private renderedMenuItems: ButtonMenuItem[] = [];

  private readonly disabled: Readable<boolean> = Readables.toReadable(
    this.props.disabled ?? false,
    true,
  );

  private readonly arrowColour = this.disabled.derive((v) => (v ? 'grey' : 'white'));

  private readonly visible = Readables.toReadable(this.props.visible ?? Signal.of(true), true);

  private onClick() {
    if (!this.disabled.get()) {
      const menuItems = this.props.menuItems?.get();
      if (menuItems === undefined || menuItems.length === 0) {
        this.props.onClick();
      }
    }
  }

  private onClickHandler = this.onClick.bind(this);

  onDropdownMenuElementClick(val: ButtonMenuItem) {
    if (
      val.disabled === undefined ||
      (typeof val.disabled === 'boolean' && val.disabled === false) ||
      (Readables.isReadable(val.disabled) && val.disabled.get() === false)
    ) {
      val.action();
      this.dropdownIsOpened.set(false);
    }
  }

  private onDropdownMenuElementClickHandler = this.onDropdownMenuElementClick.bind(this);

  private scrollMenuTo(elementIndex: number) {
    // Assume 36px height for each menu item div
    this.dropdownMenuRef.target.scrollTo({ behavior: 'instant', left: 0, top: elementIndex * 36 });
  }

  public afterRender(node: ViewNode): void {
    super.afterRender(node);

    if (this.props.disabled === undefined) {
      this.props.disabled = Signal.of(false);
    }
    if (this.props.selected === undefined) {
      this.props.selected = Signal.of(false);
    }
    if (this.props.highlighted === undefined) {
      this.props.highlighted = Signal.of(false);
    }
    if (this.props.menuItems && !this.props.idPrefix) {
      console.error('Button: menuItems set without idPrefix.');
    }
    if (this.props.idPrefix === undefined) {
      this.props.idPrefix = '';
    }
    this.buttonRef.target.addEventListener('click', this.onClickHandler);

    this.subs.push(
      this.disabled.watch((val) => {
        this.buttonRef.peek()?.classList.toggle('disabled', val);
      }, true),
    );

    this.subs.push(
      this.visible.watch((val) => {
        this.topRef.target.style.visibility = val ? 'inherit' : 'hidden';
      }, true),
    );

    this.subs.push(
      this.props.selected.watch((val) => {
        this.buttonRef.peek()?.classList.toggle('selected', val);
      }, true),
    );

    this.subs.push(
      this.props.highlighted.watch((val) => {
        this.buttonRef.peek()?.classList.toggle('highlighted', val);
      }, true),
    );

    // Menu handling
    if (this.props.menuItems !== undefined) {
      this.subs.push(
        this.props.menuItems.watch((items) => {
          // Delete click handler, delete dropdownMenuRef children, render dropdownMenuRef children,
          this.renderedMenuItems?.forEach((val, i) => {
            document
              .getElementById(`${this.props.idPrefix}_${i}`)
              ?.removeEventListener('click', this.onDropdownMenuElementClickHandler.bind(this, val));
          });

          // Delete dropdownMenuRef's children
          while (this.dropdownMenuRef.target.firstChild) {
            this.dropdownMenuRef.target.removeChild(this.dropdownMenuRef.target.firstChild);
          }

          this.renderedMenuItems = items;

          // Render dropdownMenuRef's children
          const itemNodes: ViewNode = (
            <div>
              {items?.map<ViewNode>(
                (el, idx) => (
                  <span
                    id={`${this.props.idPrefix}_${idx}`}
                    class={{
                      'mfd-dropdown-menu-element': true,
                      disabled: el.disabled ?? false,
                    }}
                  >
                    {el.label}
                  </span>
                ),
                this,
              )}
            </div>
          );
          UI.mount(itemNodes, this.dropdownMenuRef.target);

          // Add click event listener
          for (const val of items) {
            if (typeof val.disabled === 'boolean' && val.disabled === true) {
              continue;
            }

            const i = items.indexOf(val);
            document
              .getElementById(`${this.props.idPrefix}_${i}`)
              ?.addEventListener('click', this.onDropdownMenuElementClickHandler.bind(this, val));
          }

          // Check if menu would overflow vertically (i.e. leave screen at the bottom). If so, open menu upwards
          // Open menu for a split second to measure size
          this.dropdownMenuRef.target.style.display = 'block';
          this.buttonRef.target.classList.add('opened');

          // Check if menu leaves screen at the bottom, reposition if needed
          const boundingRect = this.dropdownMenuRef.target.getBoundingClientRect();
          const overflowsVertically = boundingRect.top + boundingRect.height > 1024;
          this.menuOpensUpwards.set(overflowsVertically);

          if (overflowsVertically) {
            this.dropdownMenuRef.target.style.top = `${Math.round(-boundingRect.height)}px`;
          }

          // Close again
          this.dropdownMenuRef.target.style.display = 'none';
          this.buttonRef.target.classList.remove('opened');
        }, true),
      );
    }

    // Close dropdown menu if clicked outside
    document.getElementById('MFD_CONTENT')?.addEventListener('click', this.onCloseDropdownHandler);

    this.buttonRef.target.addEventListener('click', this.onButtonClickHandler);

    this.subs.push(
      this.dropdownIsOpened.watch((val) => {
        this.dropdownMenuRef.target.style.display = val ? 'block' : 'none';

        if (val) {
          this.buttonRef.target.classList.add('opened');
        } else {
          this.buttonRef.target.classList.remove('opened');
        }

        if (this.props.scrollToMenuItem !== undefined) {
          this.scrollMenuTo(this.props.scrollToMenuItem.get());
        }

        if (this.props.dropdownMenuRightAligned) this.dropdownMenuRef.target.style.right = '0px';
      }),
    );

    if (this.props.scrollToMenuItem !== undefined) {
      this.subs.push(
        this.props.scrollToMenuItem.watch((val) => {
          if (this.dropdownIsOpened.get()) {
            this.scrollMenuTo(val);
          }
        }),
      );
    }
  }

  private onCloseDropdown(e: MouseEvent) {
    if (!this.topRef.peek()?.contains(e.target as Node) && this.dropdownIsOpened.get()) {
      this.dropdownIsOpened.set(false);
    }
  }

  private onCloseDropdownHandler = this.onCloseDropdown.bind(this);

  private onButtonClick() {
    if (this.props.menuItems && this.props.menuItems.get().length > 0 && !this.disabled.get()) {
      this.dropdownIsOpened.set(!this.dropdownIsOpened.get());
    }
  }

  private onButtonClickHandler = this.onButtonClick.bind(this);

  public destroy(): void {
    // Destroy all subscriptions to remove all references to this instance.
    this.subs.forEach((x) => x.destroy());

    this.buttonRef.target.removeEventListener('click', this.onClickHandler);
    document.getElementById('MFD_CONTENT')?.removeEventListener('click', this.onCloseDropdownHandler);
    this.buttonRef.target.removeEventListener('click', this.onButtonClickHandler);

    this.renderedMenuItems?.forEach((val, i) => {
      document
        .getElementById(`${this.props.idPrefix}_${i}`)
        ?.removeEventListener('click', this.onDropdownMenuElementClickHandler.bind(this, val));
    });

    super.destroy();
  }

  public render(): ViewNode {
    return (
      <div
        class="mfd-dropdown-container"
        ref={this.topRef}
        style={this.props.containerStyle ? this.props.containerStyle : ''}
      >
        <span
          ref={this.buttonRef}
          class="mfd-button"
          style={`${this.props.buttonStyle} ${this.props.menuItems && this.props.menuItems.get().length > 0 ? 'padding-right: 5px;' : ''}`}
        >
          {this.props.menuItems !== undefined && this.props.showArrow !== false ? (
            <div class="mfd-fms-fpln-button-dropdown">
              <span class="mfd-fms-fpln-button-dropdown-label">{this.props.label}</span>
              <span class="mfd-fms-fpln-button-dropdown-arrow">
                <TriangleUp
                  class={{ hidden: this.menuOpensUpwards.derive(Logic.not()) }}
                  color={this.arrowColour}
                />
                <TriangleDown class={{ hidden: this.menuOpensUpwards }} color={this.arrowColour} />
              </span>
            </div>
          ) : (
            <span>{this.props.label}</span>
          )}
        </span>
        <div
          ref={this.dropdownMenuRef}
          class="mfd-dropdown-menu"
          style={`display: ${this.dropdownIsOpened.get() ? 'block' : 'none'}`}
        />
      </div>
    );
  }
}

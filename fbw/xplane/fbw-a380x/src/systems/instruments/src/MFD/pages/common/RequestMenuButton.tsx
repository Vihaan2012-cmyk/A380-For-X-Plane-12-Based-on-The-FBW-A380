import {
  ViewProps,
  View,
  UI,
  Signal,
  Readable,
  Bond,
  ViewNode,
} from 'xplane-js/ui';
import './style.scss';
import { TriangleLeft } from '../../../MsfsAvionicsCommon/UiWidgets/shapes';

export type ButtonMenuItem = {
  label: string;
  action(): void;
};

export interface RequestMenuButtonProps extends ViewProps {
  label: string | Readable<ViewNode>;
  menuItems?: Readable<ButtonMenuItem[]>; // When defining menu items, idPrefix has to be set
  showArrow?: boolean;
  idPrefix?: string;
  disabled?: Readable<boolean>;
  selected?: Readable<boolean>; // Renders with lighter grey if selected (e.g. for segmented controls)
  buttonStyle?: string;
  onClick: () => void;
  scrollToMenuItem?: Readable<number>;
}

/*
 * Button for MFD pages. If menuItems is set, a dropdown menu will be displayed when button is clicked
 */
export class RequestMenuButton extends View<RequestMenuButtonProps> {
  // Make sure to collect all subscriptions here, otherwise page navigation doesn't work.
  private subs = [] as Bond[];

  private topRef = UI.ref<HTMLDivElement>();

  private buttonRef = UI.ref<HTMLSpanElement>();

  private dropdownMenuRef = UI.ref<HTMLDivElement>();

  private dropdownIsOpened = Signal.of(false);

  private menuOpensUpwards = Signal.of(false);

  private renderedMenuItems: ButtonMenuItem[] = [];

  private onClick() {
    if (!this.props.disabled?.get()) {
      this.props.onClick();
    }
  }

  private onClickHandler = this.onClick.bind(this);

  onDropdownMenuElementClick(val: ButtonMenuItem) {
    val.action();
    this.dropdownIsOpened.set(false);
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
    if (typeof this.props.label === 'string') {
      this.props.label = Signal.of(<span>{this.props.label}</span>);
    }
    if (this.props.menuItems && !this.props.idPrefix) {
      console.error('Button: menuItems set without idPrefix.');
    }
    if (this.props.idPrefix === undefined) {
      this.props.idPrefix = '';
    }
    if (this.props.showArrow === undefined) {
      this.props.showArrow = true;
    }
    this.buttonRef.target.addEventListener('click', this.onClickHandler);

    this.subs.push(
      this.props.disabled.watch((val) => {
        if (val) {
          this.buttonRef.peek()?.classList.add('disabled');
        } else {
          this.buttonRef.peek()?.classList.remove('disabled');
        }
      }, true),
    );

    this.subs.push(
      this.props.selected.watch((val) => {
        if (val) {
          this.buttonRef.peek()?.classList.add('selected');
        } else {
          this.buttonRef.peek()?.classList.remove('selected');
        }
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
                  <span id={`${this.props.idPrefix}_${idx}`} class="mfd-dropdown-menu-element">
                    {el.label}
                  </span>
                ),
                this,
              )}
            </div>
          );
          UI.mount(itemNodes, this.dropdownMenuRef.target);

          // Add click event listener
          items?.forEach((val, i) => {
            document
              .getElementById(`${this.props.idPrefix}_${i}`)
              ?.addEventListener('click', this.onDropdownMenuElementClickHandler.bind(this, val));
          });

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
          } else {
            this.dropdownMenuRef.target.style.top = '0';
          }

          // Offset dropdown menu to the left
          const buttonBoundingRect = this.buttonRef.target.getBoundingClientRect();
          this.dropdownMenuRef.target.style.right = `${Math.round(buttonBoundingRect.width)}px`;

          // Close again
          this.dropdownMenuRef.target.style.display = 'none';
          this.buttonRef.target.classList.remove('opened');
        }, true),
      );
    }

    this.subs.push(
      this.props.label?.watch((val) => {
        while (this.buttonRef.target.firstChild) {
          this.buttonRef.target.removeChild(this.buttonRef.target.firstChild);
        }

        // If menuItems is defined, render as button with arrow on the right side
        if (this.props.menuItems !== undefined && this.props.showArrow) {
          const n: ViewNode = (
            <div class="mfd-fms-fpln-button-dropdown">
              <span class="mfd-fms-fpln-button-dropdown-arrow">
                <TriangleLeft color={this.props.disabled?.get() ? 'grey' : 'white'} />
              </span>
              <span class="mfd-atccom-request-button-dropdown-label">{val}</span>
            </div>
          );
          UI.mount(n, this.buttonRef.target);
        } else {
          UI.mount(val, this.buttonRef.target);
        }
      }, true),
    );

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
    if (this.props.menuItems && this.props.menuItems.get().length > 0 && !this.props.disabled?.get()) {
      this.dropdownIsOpened.set(!this.dropdownIsOpened.get());
    }
  }

  private onButtonClickHandler = this.onButtonClick.bind(this);

  public destroy(): void {
    // Destroy all subscriptions to remove all references to this instance.
    this.subs.forEach((x) => x.destroy());

    this.buttonRef.peek()?.removeEventListener('click', this.onClickHandler);
    document.getElementById('MFD_CONTENT')?.removeEventListener('click', this.onCloseDropdownHandler);
    this.buttonRef.peek()?.removeEventListener('click', this.onButtonClickHandler);

    super.destroy();
  }

  public render(): ViewNode {
    return (
      <div class="mfd-dropdown-container" ref={this.topRef}>
        <span
          ref={this.buttonRef}
          class="mfd-button"
          style={`${this.props.buttonStyle} ${this.props.menuItems && this.props.menuItems.get().length > 0 ? 'padding-right: 5px;' : ''}`}
        />
        <div
          ref={this.dropdownMenuRef}
          class="mfd-atccom-request-button-dropdown-menu"
          style={`display: ${this.dropdownIsOpened.get() ? 'block' : 'none'}`}
        />
      </div>
    );
  }
}

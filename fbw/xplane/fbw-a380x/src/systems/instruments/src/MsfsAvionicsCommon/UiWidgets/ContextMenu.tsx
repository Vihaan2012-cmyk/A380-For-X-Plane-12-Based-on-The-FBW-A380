//  Copyright (c) 2024-2025 FlyByWire Simulations
//  SPDX-License-Identifier: GPL-3.0

import {
  ViewProps,
  View,
  UI,
  Signal,
  Readable,
  Bond,
  ViewNode,
} from 'xplane-js/ui';

export interface ContextMenuElement {
  name: string;
  disabled: boolean | Readable<boolean>;
  onPressed: () => void;
}

interface ContextMenuProps extends ViewProps {
  values: Readable<ContextMenuElement[]>;
  idPrefix: string;
  opened: Signal<boolean>;
}

/*
 * Context menu, which can be opened e.g. by clicking on F-PLN waypoints
 */
export class ContextMenu extends View<ContextMenuProps> {
  // Make sure to collect all subscriptions here, otherwise page navigation doesn't work.
  private readonly subs = [] as Bond[];

  private readonly contextMenuRef = UI.ref<HTMLDivElement>();

  private renderedMenuItems: ContextMenuElement[] = [];

  private openedAt: number = 0;

  public display(x: number, y: number) {
    this.props.opened.set(true);
    this.openedAt = Date.now();
    this.contextMenuRef.target.style.display = 'block';
    this.contextMenuRef.target.style.left = `${x}px`;
    this.contextMenuRef.target.style.top = `${y}px`;
  }

  public hideMenu() {
    this.props.opened.set(false);
    this.contextMenuRef.target.style.display = 'none';
  }

  afterRender(node: ViewNode): void {
    super.afterRender(node);

    if (!this.props.idPrefix) {
      console.error('ContextMenu: No idPrefix set.');
    }

    this.contextMenuRef.target.style.display = 'none';

    this.subs.push(
      this.props.values.watch((items) => {
        // Delete click handler, delete contextMenuRef children, render contextMenuRef children,
        this.renderedMenuItems?.forEach((val, i) => {
          document
            .getElementById(`${this.props.idPrefix}_${i}`)
            ?.removeEventListener('click', this.onItemClick.bind(this, val));
        });

        // Delete contextMenuRef's children
        while (this.contextMenuRef.target.firstChild) {
          this.contextMenuRef.target.removeChild(this.contextMenuRef.target.firstChild);
        }

        this.renderedMenuItems = items;

        // Render contextMenuRef's children
        const itemNodes: ViewNode = (
          <div>
            {items?.map<ViewNode>(
              (el, idx) => (
                <span
                  class={`mfd-context-menu-element${el.disabled === true ? ' disabled' : ''}`}
                  id={`${this.props.idPrefix}_${idx}`}
                >
                  {el.name}
                </span>
              ),
              this,
            )}
          </div>
        );
        UI.mount(itemNodes, this.contextMenuRef.target);

        // Add click event listener
        items?.forEach((val, i) => {
          document
            .getElementById(`${this.props.idPrefix}_${i}`)
            ?.addEventListener('click', this.onItemClick.bind(this, val));
        });
      }, true),
    );

    // Close dropdown menu if clicked outside
    document.getElementById('MFD_CONTENT')?.addEventListener('click', this.onCloseHandler);
  }

  private onItemClick(val: ContextMenuElement) {
    if (!val.disabled) {
      this.hideMenu();
      val.onPressed();
    }
  }

  private onClose() {
    if (Date.now() - this.openedAt > 100 && this.props.opened.get() === true) {
      this.hideMenu();
    }
  }

  private onCloseHandler = this.onClose.bind(this);

  render(): ViewNode {
    return <div ref={this.contextMenuRef} class="mfd-context-menu" />;
  }

  destroy(): void {
    for (const s of this.subs) {
      s.destroy();
    }

    this.renderedMenuItems?.forEach((val, i) => {
      document
        .getElementById(`${this.props.idPrefix}_${i}`)
        ?.removeEventListener('click', this.onItemClick.bind(this, val));
    });

    super.destroy();
  }
}

// Copyright (c) 2024-2026 FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0
import {
  ViewProps,
  View,
  UI,
  Writable,
  Readable,
  Readables,
  Bond,
  ViewNode,
} from 'xplane-js/ui';

interface TopTabElementProps extends ViewProps {
  title: string;
  isSelected: boolean;
  isLast: boolean;
  selectedTextColor: string;
  isHighlighted: boolean;
  height: number; // height of tab bar element
  slantedEdgeAngle: number; // in degrees
  smallFont: boolean;
  onClick: () => void;
}

class TopTabElement extends View<TopTabElementProps> {
  private readonly triangleWidth = this.props.height * Math.tan((this.props.slantedEdgeAngle * Math.PI) / 180);

  private readonly divRef = UI.ref<HTMLDivElement>();

  private readonly textRef = UI.ref<HTMLSpanElement>();

  afterRender(node: ViewNode): void {
    super.afterRender(node);

    this.divRef.target.addEventListener('click', this.props.onClick);

    if (this.props.isHighlighted) {
      this.textRef.target.style.color = '#00ff00';
    } else if (this.props.isSelected) {
      this.textRef.target.style.color = this.props.selectedTextColor;
    } else {
      this.textRef.target.style.color = 'white;';
    }
  }

  destroy(): void {
    this.divRef.target.removeEventListener('click', this.props.onClick);

    super.destroy();
  }

  render(): ViewNode {
    return (
      <div
        ref={this.divRef}
        class={`mfd-top-tab-navigator-bar-element-outer${this.props.isSelected ? ' active' : ''}`}
        style={`margin-right: ${this.props.isLast ? 0 : -15}px`}
      >
        <svg height={this.props.height} width={this.triangleWidth}>
          <polygon
            points={`0,${this.props.height} ${this.triangleWidth},0 ${this.triangleWidth},${this.props.height}`}
            style={`fill:${this.props.isSelected ? '#000000' : '#272525'};`}
          />
          <line
            x1="0"
            y1={this.props.height}
            x2={this.triangleWidth}
            y2="0"
            style="stroke: lightgrey; stroke-width:2"
          />
          {!this.props.isSelected && (
            <line
              x1="0"
              y1={this.props.height - 1}
              x2={this.triangleWidth}
              y2={this.props.height - 1}
              style="stroke: lightgrey; stroke-width:2"
            />
          )}
        </svg>
        <span
          ref={this.textRef}
          class={`mfd-top-tab-navigator-bar-element-label${this.props.isSelected ? ' active' : ''}`}
          // eslint-disable-next-line max-len
          style={`font-size: ${this.props.smallFont ? Math.round(this.props.height * 0.58) : Math.round(this.props.height * 0.66)}px;`}
        >
          {this.props.title}
        </span>
        <svg height={this.props.height} width={this.triangleWidth}>
          <polygon
            points={`0,0 ${this.triangleWidth},${this.props.height} 0,${this.props.height}`}
            style={`fill:${this.props.isSelected ? '#000000' : '#272525'};`}
          />
          <line
            x1="0"
            y1="0"
            x2={this.triangleWidth}
            y2={this.props.height}
            style="stroke: lightgrey; stroke-width:2"
          />
          {!this.props.isSelected && (
            <line
              x1="0"
              y1={this.props.height - 1}
              x2={this.triangleWidth}
              y2={this.props.height - 1}
              style="stroke: lightgrey; stroke-width:2"
            />
          )}
        </svg>
      </div>
    );
  }
}

interface TopTabNavigatorPageProps {
  isVisible?: Readable<boolean>;
  containerStyle?: string;
}

export class TopTabNavigatorPage extends View<TopTabNavigatorPageProps> {
  private readonly topDivRef = UI.ref<HTMLDivElement>();

  private isVisible = false;

  public getVisibility(): boolean {
    return this.isVisible;
  }

  public setVisibility(value: boolean) {
    this.isVisible = value;

    if (this.topDivRef.target !== undefined) {
      this.topDivRef.target.style.display = value ? 'flex' : 'none';
    }
  }

  render(): ViewNode {
    return (
      <div ref={this.topDivRef} class="mfd-top-tab-navigator-tab-content" style={this.props.containerStyle}>
        {this.props.children}
      </div>
    );
  }
}

interface TopTabNavigatorProps {
  pageTitles: string[] | Readable<string[]>;
  selectedPageIndex: Writable<number>;
  selectedTabTextColor?: string;
  /** in pixels */
  tabBarHeight?: number;
  /** in degrees, vertical line equals 0° */
  tabBarSlantedEdgeAngle?: number;
  /** in pixels */
  additionalRightSpace?: number;
  /** Index of tab to be highlighted with green text color instead of white (e.g. used for PERF page) */
  highlightedTab?: Readable<number>;
  pageChangeCallback?: (index: number) => void;
}

/*
 * Container for multiple tabs, with tab navigation at the top. Pages are instantiated as children of the component, as TopTabElement
 */
export class TopTabNavigator extends View<TopTabNavigatorProps> {
  private readonly subs = [] as Bond[];

  private readonly navigatorBarRef = UI.ref<HTMLDivElement>();

  private readonly pageTitles = Readables.toReadable(this.props.pageTitles, true);

  constructor(props: TopTabNavigatorProps) {
    super(props);

    if (this.pageTitles.get().length !== this.props.children?.length) {
      console.error('Number of TopTabNavigator children is not equal to number of elements in pageTitles array.');
    }

    // Set visibility for pages and check whether TopTabNavigator only has TopTabNavigatorPage elements as children
    const nodes = UI.childNodes(<></>, this.props.children ?? []);
    nodes?.forEach((page, index) => {
      if (page.target instanceof TopTabNavigatorPage) {
        page.target.setVisibility(index === this.props.selectedPageIndex.get());
      } else {
        console.error('Discovered child of TopTabNavigator which is not an instance of TopTabNavigatorPage.');
      }
    });
  }

  onPageChange(newIndex: number): void {
    const pageTitle = this.pageTitles.get()[newIndex];
    // Skip empty page titles;
    if (!pageTitle || pageTitle.length === 0) {
      return;
    }

    if (this.props.pageChangeCallback) {
      this.props.pageChangeCallback(newIndex);
    } else {
      this.props.selectedPageIndex.set(newIndex);
    }
  }

  populateElements(node: ViewNode, selectedTab: number): void {
    // Clear children nodes of top navigation bar
    while (this.navigatorBarRef.target.firstChild) {
      this.navigatorBarRef.target.removeChild(this.navigatorBarRef.target.firstChild);
    }

    // Re-set visibility for pages
    const nodes = UI.childNodes(node, this.props.children ?? []);
    nodes?.forEach((page, index) => {
      if (page.target instanceof TopTabNavigatorPage) {
        page.target.setVisibility(index === selectedTab);
      }
    });

    // Re-populate top navigation bar
    this.pageTitles.get().forEach((pageTitle, index) => {
      if (pageTitle.length > 0) {
        // Skip empty titles. Used to hide pages
        UI.mount(
          <TopTabElement
            title={pageTitle}
            isSelected={selectedTab === index}
            isLast={index === this.pageTitles.get().length - 1}
            height={this.props.tabBarHeight || 36}
            slantedEdgeAngle={this.props.tabBarSlantedEdgeAngle || 30}
            selectedTextColor={this.props.selectedTabTextColor || 'white'}
            onClick={() => this.onPageChange(index)}
            isHighlighted={this.props.highlightedTab ? this.props.highlightedTab?.get() === index : false}
            smallFont={this.pageTitles.get().filter((title) => title.length > 0).length > 2}
          />,
          this.navigatorBarRef.target,
        );
      }
    });

    // Add space at end, if any
    if (this.props.additionalRightSpace && this.props.additionalRightSpace > 0) {
      const div = document.createElement('div');
      div.style.width = `${this.props.additionalRightSpace}px`;
      div.style.borderBottom = '2px solid lightgray';
      this.navigatorBarRef.target.appendChild(div);
    }
  }

  afterRender(node: ViewNode): void {
    super.afterRender(node);

    this.subs.push(
      this.props.selectedPageIndex.watch((value) => {
        this.populateElements(node, value);
      }, true),

      this.pageTitles.watch(() => {
        this.populateElements(node, this.props.selectedPageIndex.get());
      }),
    );

    if (this.props.highlightedTab) {
      this.subs.push(
        this.props.highlightedTab.watch(() => this.populateElements(node, this.props.selectedPageIndex.get())),
      );
    }
  }

  render(): ViewNode {
    return (
      <div class="mfd-top-tab-navigator-container">
        <div
          class="mfd-top-tab-navigator-bar"
          ref={this.navigatorBarRef}
          style={`height: ${this.props.tabBarHeight || 36}px`}
        >
          {this.pageTitles
            .get()
            .map(
              (pageTitle, index) =>
                pageTitle.length > 0 && (
                  <TopTabElement
                    title={pageTitle}
                    isSelected={this.props.selectedPageIndex.get() === index}
                    isLast={index === this.pageTitles.get().length - 1}
                    height={this.props.tabBarHeight || 36}
                    slantedEdgeAngle={this.props.tabBarSlantedEdgeAngle || 30}
                    selectedTextColor={this.props.selectedTabTextColor || 'white'}
                    onClick={() => this.onPageChange(index)}
                    isHighlighted={false}
                    smallFont={this.pageTitles.get().filter((title) => title.length > 0).length > 2}
                  />
                ),
            )}
          <div
            style={`width: ${this.props.additionalRightSpace ? this.props.additionalRightSpace : '0'}px; border-bottom: 2px solid lightgray`}
          />
        </div>
        {this.props.children}
      </div>
    );
  }

  destroy(): void {
    for (const s of this.subs) {
      s.destroy();
    }
  }
}

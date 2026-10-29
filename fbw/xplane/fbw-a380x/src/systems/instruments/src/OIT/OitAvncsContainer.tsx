//  Copyright (c) 2025 FlyByWire Simulations
//  SPDX-License-Identifier: GPL-3.0

import {
  BusSignal,
  View,
  Bus,
  UI,
  Derived,
  Ref,
  Signal,
  Readable,
  Bond,
  ViewNode,
} from 'xplane-js/ui';
import { OisDomain } from './OIT';
import { OitUiService, OitUriInformation } from './OitUiService';
import { OitDisplayUnit } from './OitDisplayUnit';
import { avncsPageForUrl } from './OitPageDirectory';
import { OitNotFound } from './Pages/OitNotFound';
import { InternalKbdKeyEvent, OitSimvars } from './OitSimvarPublisher';
import { InteractionMode } from '../MsfsAvionicsCommon/UiWidgets/InputField';
import { OitAvncsHeader } from './Pages/NssAvncs/OitAvncsHeader';
import { OitAvncsFooter } from './Pages/NssAvncs/OitAvncsFooter';
import { OitAvncsLoadingScreen } from './Pages/NssAvncs/OitAvncsLoadingScreen';
import { OitAvncsLogin } from './Pages/NssAvncs/OitAvncsLogin';
import { AnsuOps } from './System/AnsuOps';

interface OitAvncsContainerProps {
  readonly bus: Bus;
  readonly displayUnitRef: Ref<OitDisplayUnit>;
  readonly captOrFo: 'CAPT' | 'FO';
  readonly avncsOrFltOps: Readable<OisDomain>;
  readonly ansu: AnsuOps;
}

export abstract class OitAvncsContainer extends View<OitAvncsContainerProps> {
  // Make sure to collect all subscriptions here, otherwise page navigation doesn't work.
  protected readonly subscriptions = [] as Bond[];

  private readonly sub = this.props.bus.inbox<OitSimvars>();

  #uiService = new OitUiService(this.props.captOrFo);

  get uiService() {
    return this.#uiService;
  }

  get ansu() {
    return this.props.ansu;
  }

  public readonly hEventConsumer = this.props.bus.inbox<InternalKbdKeyEvent>().topic('kbdKeyEvent');
  public readonly interactionMode = Signal.of<InteractionMode>(InteractionMode.Touchscreen);

  private readonly topRef = UI.ref<HTMLDivElement>();

  private readonly ansuPowered = BusSignal.from(this.sub.topic('nssAnsu1Healthy'), false);

  private readonly hideContent = Derived.from(
    ([mode, powered]) => mode === 'flt-ops' || !powered,
    this.props.avncsOrFltOps,
    this.ansuPowered,
  );

  private readonly hideContainer = this.props.avncsOrFltOps.derive((mode) => mode === 'flt-ops');

  private readonly renderedUri = Signal.of<OitUriInformation | null>(null);

  private readonly activePageRef = UI.ref<HTMLDivElement>();

  private activePage: ViewNode = (<OitNotFound uiService={this.uiService} />);

  public afterRender(node: ViewNode): void {
    super.afterRender(node);

    this.subscriptions.push(
      this.uiService.activeUri.watch((uri) => {
        this.activeUriChanged(uri);
      }),
      this.ansuPowered.watch((powered) => {
        if (!powered) {
          this.uiService.navigateTo('nss-avncs');
        }
      }),
      this.ansuPowered,
      this.hideContent,
      this.hideContainer,
    );

    this.uiService.navigateTo('nss-avncs');
  }

  private activeUriChanged(uri: OitUriInformation) {
    if (
      (uri.uri.match(/nss-avncs\/company-com\/\S*/gm) &&
        this.renderedUri.get()?.uri.match(/nss-avncs\/company-com/gm)) ||
      (uri.uri.match(/nss-avncs\/a380x-systems\/\S*/gm) &&
        this.renderedUri.get()?.uri.match(/nss-avncs\/a380x-systems/gm)) ||
      (uri.uri.match(/nss-avncs\/maintenance\/\S*/gm) &&
        this.renderedUri.get()?.uri.match(/nss-avncs\/maintenance/gm))
    ) {
      // Handle special case for company-com, where the sub-navigation is handled by the component with their own navigator and we're already on the right parent page.
      return;
    }

    // Remove and destroy old OIT page
    if (this.activePageRef.peek()) {
      while (this.activePageRef.target.firstChild) {
        this.activePageRef.target.removeChild(this.activePageRef.target.firstChild);
      }
    }
    if (this.activePage && this.activePage.target instanceof View) {
      this.activePage.target.destroy();
    }

    // Mapping from URL to page component
    if (uri.page) {
      this.activePage = avncsPageForUrl(`${uri.sys}/${uri.page}`, this.props.bus, this.uiService, this);
    } else {
      this.activePage = avncsPageForUrl(`${uri.sys}`, this.props.bus, this.uiService, this);
    }

    UI.mount(this.activePage, this.activePageRef?.peek());
    this.renderedUri.set(uri);
  }

  public destroy(): void {
    // Destroy all subscriptions to remove all references to this instance.
    for (const s of this.subscriptions) {
      s.destroy();
    }

    super.destroy();
  }

  render(): ViewNode {
    return (
      <div ref={this.topRef} class={{ 'oit-main': true, hidden: this.hideContainer }}>
        <div class={{ 'oit-page-container': true, hidden: this.hideContent }}>
          <OitAvncsHeader bus={this.props.bus} uiService={this.uiService} avncsOrFltOps={this.props.avncsOrFltOps} />
          <div ref={this.activePageRef} class="mfd-navigator-container" />
          <OitAvncsFooter uiService={this.uiService} avncsOrFltOps={this.props.avncsOrFltOps} />
        </div>
        <OitAvncsLogin bus={this.props.bus} uiService={this.uiService} captOrFo={this.props.captOrFo} />
        <OitAvncsLoadingScreen bus={this.props.bus} captOrFo={this.props.captOrFo} />
      </div>
    );
  }
}

//  Copyright (c) 2025 FlyByWire Simulations
//  SPDX-License-Identifier: GPL-3.0

import {
  BusSignal,
  View,
  Bus,
  UI,
  Derived,
  DerivedReadable,
  Ref,
  VarUnit,
  Signal,
  Readable,
  Bond,
  ViewNode,
} from 'xplane-js/ui';
import { OisDomain } from './OIT';
import { OitUiService, OitUriInformation } from './OitUiService';
import { getDisplayIndex, OitDisplayUnit } from './OitDisplayUnit';
import { fltOpsPageForUrl } from './OitPageDirectory';
import { OitFltOpsHeader } from './Pages/FltOps/OitFltOpsHeader';
import { OitNotFound } from './Pages/OitNotFound';
import { OitFltOpsFooter } from './Pages/FltOps/OitFltOpsFooter';
import { OisLaptop } from './OisLaptop';
import { InternalKbdKeyEvent, OitSimvars } from './OitSimvarPublisher';
import { InteractionMode } from '../MsfsAvionicsCommon/UiWidgets/InputField';
import { OitFltOpsLoadingScreen } from './Pages/FltOps/OitFltOpsLoadingScreen';
import { OitFltOpsLogin } from './Pages/FltOps/OitFltOpsLogin';
import { Vars } from 'xplane-js/data';

interface OitFltOpsContainerProps {
  readonly bus: Bus;
  readonly laptop: OisLaptop;
  readonly displayUnitRef: Ref<OitDisplayUnit>;
  readonly captOrFo: 'CAPT' | 'FO';
  readonly avncsOrFltOps: Readable<OisDomain>;
}

export abstract class OitFltOpsContainer extends View<OitFltOpsContainerProps> {
  // Make sure to collect all subscriptions here, otherwise page navigation doesn't work.
  protected readonly subscriptions = [] as Bond[];

  private readonly sub = this.props.bus.inbox<OitSimvars>();

  #uiService = new OitUiService(this.props.captOrFo);

  get uiService() {
    return this.#uiService;
  }

  get laptop() {
    return this.props.laptop;
  }

  get laptopData() {
    return this.props.laptop.data;
  }

  public readonly hEventConsumer = this.props.bus.inbox<InternalKbdKeyEvent>().topic('kbdKeyEvent');
  public readonly interactionMode = Signal.of<InteractionMode>(InteractionMode.Touchscreen);

  private readonly topRef = UI.ref<HTMLDivElement>();

  private readonly laptopHealthy = BusSignal.from(
    this.sub.topic(this.props.captOrFo === 'CAPT' ? 'laptopCaptHealthy' : 'laptopFoHealthy'),
    false,
  );

  private readonly hideContent = Derived.from(
    ([mode, healthy]) => mode === 'nss-avncs' || !healthy,
    this.props.avncsOrFltOps,
    this.laptopHealthy,
  );

  private readonly hideContainer = this.props.avncsOrFltOps.derive((mode) => mode === 'nss-avncs');

  private readonly activePageRef = UI.ref<HTMLDivElement>();

  private activePage: ViewNode = (<OitNotFound uiService={this.uiService} />);

  private readonly showChartsSimvar = `L:A32NX_OIS_${getDisplayIndex()}_SHOW_CHARTS`;
  private readonly showOfpSimvar = `L:A32NX_OIS_${getDisplayIndex()}_SHOW_OFP`;

  private overlaySub: DerivedReadable<void> | undefined;

  public afterRender(node: ViewNode): void {
    super.afterRender(node);

    this.overlaySub = Derived.from(
      ([uri, displayFailed, displayPowered, laptopHealthy, operationMode]) => {
        // Activate EFB overlay if on charts or flt-folder page
        Vars.write(
          this.showChartsSimvar,
          VarUnit.Bool,
          uri.uri === 'flt-ops/charts' &&
            operationMode === 'flt-ops' &&
            !displayFailed &&
            displayPowered &&
            laptopHealthy,
        );
        Vars.write(
          this.showOfpSimvar,
          VarUnit.Bool,
          uri.uri === 'flt-ops/flt-folder' &&
            operationMode === 'flt-ops' &&
            !displayFailed &&
            displayPowered &&
            laptopHealthy,
        );
      },
      this.uiService.activeUri,
      this.props.displayUnitRef.target.failed,
      this.props.displayUnitRef.target.powered,
      this.laptopHealthy,
      this.props.avncsOrFltOps,
    );

    this.subscriptions.push(
      this.uiService.activeUri.watch((uri) => {
        this.activeUriChanged(uri);
      }),
      this.laptopHealthy.watch((healthy) => {
        if (!healthy) {
          this.uiService.resetFltOpsSession();
        }
      }, true),
      this.overlaySub,
      this.laptopHealthy,
      this.hideContent,
      this.hideContainer,
    );

    this.uiService.navigateTo('flt-ops');
  }

  private activeUriChanged(uri: OitUriInformation) {
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
      this.activePage = fltOpsPageForUrl(`${uri.sys}/${uri.page}`, this.props.bus, this.uiService, this);
    } else {
      this.activePage = fltOpsPageForUrl(`${uri.sys}`, this.props.bus, this.uiService, this);
    }

    UI.mount(this.activePage, this.activePageRef?.peek());
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
      <>
        <div ref={this.topRef} class={{ 'oit-main': true, hidden: this.hideContainer }}>
          <div class={{ 'oit-page-container': true, hidden: this.hideContent }}>
            <OitFltOpsHeader uiService={this.uiService} avncsOrFltOps={this.props.avncsOrFltOps} />
            <div ref={this.activePageRef} class="mfd-navigator-container" />
            <OitFltOpsFooter uiService={this.uiService} avncsOrFltOps={this.props.avncsOrFltOps} />
          </div>
          <OitFltOpsLogin bus={this.props.bus} uiService={this.uiService} captOrFo={this.props.captOrFo} />
          <OitFltOpsLoadingScreen bus={this.props.bus} captOrFo={this.props.captOrFo} />
        </div>
      </>
    );
  }
}

//  Copyright (c) 2024-2025 FlyByWire Simulations
//  SPDX-License-Identifier: GPL-3.0

import './pages/common/style.scss';

import {
  ClockTopics,
  ViewProps,
  Stream,
  BusSignal,
  View,
  Bus,
  UI,
  PanelEventTopics,
  Derived,
  Signal,
  Readable,
  Bond,
  ViewNode,
} from 'xplane-js/ui';
import { DatabaseItem, EfisSide, logTroubleshootingError, Waypoint } from '@flybywiresim/fbw-sdk';

import { MouseCursor } from '../MsfsAvionicsCommon/UiWidgets/MouseCursor';

import { MfdMsgList } from './pages/FMS/MfdMsgList';
import { ActiveUriInformation, MfdUiService } from './pages/common/MfdUiService';
import { MfdFmsFplnDuplicateNames } from './pages/FMS/F-PLN/MfdFmsFplnDuplicateNames';
import { headerForSystem, pageForUrl } from './MfdPageDirectory';
import { FmsDisplayInterface } from '@fmgc/flightplanning/interface/FmsDisplayInterface';
import { FmsErrorType } from '@fmgc/FmsError';
import { FmcServiceInterface } from './FMC/FmcServiceInterface';
import { CdsDisplayUnit, DisplayUnitID } from '../MsfsAvionicsCommon/CdsDisplayUnit';
import { InternalKccuKeyEvent, MfdSimvars } from './shared/MFDSimvarPublisher';
import { MfdFmsPageNotAvail } from './pages/FMS/MfdFmsPageNotAvail';

import { InteractionMode } from '../MsfsAvionicsCommon/UiWidgets/InputField';
import { NXSystemMessages } from './shared/NXSystemMessages';
import { AtcDatalinkSystem } from './ATCCOM/AtcDatalinkSystem';
import { FlightPlanInterface } from '@fmgc/flightplanning/FlightPlanInterface';
import { FlightPlanLegFlags } from '@fmgc/flightplanning/legs/FlightPlanLeg';
import { Vars } from 'xplane-js/data';

export const getDisplayIndex = () => {
  const url = document.getElementsByTagName('a380x-mfd')[0].getAttribute('url');
  return url ? parseInt(url.substring(url.length - 1), 10) : 0;
};

export interface AbstractMfdPageProps extends ViewProps {
  pageTitle?: string;
  bus: Bus;
  mfd: FmsDisplayInterface & MfdDisplayInterface;
  fmcService: FmcServiceInterface;
  flightPlanInterface: FlightPlanInterface;
}

export interface AtccomMfdPageProps extends ViewProps {
  pageTitle?: string;
  bus: Bus;
  mfd: MfdDisplayInterface;
  atcService: AtcDatalinkSystem;
}

interface MfdComponentProps extends ViewProps {
  bus: Bus;
  fmcService: FmcServiceInterface;
  atcService: AtcDatalinkSystem;
  captOrFo: 'CAPT' | 'FO';
}

// TODO integrate in fmgc's DisplayInterface
export interface MfdDisplayInterface {
  side: EfisSide;

  get uiService(): MfdUiService;

  hEventConsumer: Stream<[EfisSide, string]>;

  interactionMode: Readable<InteractionMode>;

  openMessageList(): void;

  navigateTo(uri: string): void;
}

export class MfdComponent
  extends View<MfdComponentProps>
  implements FmsDisplayInterface, MfdDisplayInterface
{
  private readonly subs: Bond[] = [];

  private readonly sub = this.props.bus.inbox<ClockTopics & MfdSimvars>();

  side: EfisSide = this.props.captOrFo === 'CAPT' ? 'L' : 'R';

  #uiService = new MfdUiService(this.props.captOrFo, this.props.bus);

  get uiService() {
    return this.#uiService;
  }

  public readonly hEventConsumer = this.props.bus.inbox<InternalKccuKeyEvent>().topic('kccuKeyEvent');

  public readonly interactionMode = Signal.of<InteractionMode>(InteractionMode.Touchscreen);

  private readonly fmsDataKnob = BusSignal.from(this.sub.topic('fmsDataKnob').distinct(), 0);

  private readonly fmcAIsHealthy = BusSignal.from(this.sub.topic('fmcAIsHealthy').distinct(), true);

  private readonly fmcBIsHealthy = BusSignal.from(this.sub.topic('fmcBIsHealthy').distinct(), true);

  private readonly activeFmsSource = Derived.from(
    ([knob, a, b]) => {
      const capt = a ? 'FMS 1' : 'FMS 1-C';
      const fo = b ? 'FMS 2' : 'FMS 2-C';

      switch (knob) {
        case 0:
          return fo;
        case 1:
          return this.props.captOrFo === 'CAPT' ? capt : fo;
        case 2:
          return capt;
        default:
          return 'FMS 1';
      }
    },
    this.fmsDataKnob,
    this.fmcAIsHealthy,
    this.fmcBIsHealthy,
  );

  private readonly mouseCursorRef = UI.ref<MouseCursor>();

  private readonly topRef = UI.ref<HTMLDivElement>();

  private readonly activePageRef = UI.ref<HTMLDivElement>();

  private activePage: ViewNode | null = null;

  private readonly activeHeaderRef = UI.ref<HTMLDivElement>();

  private activeHeader: ViewNode | null = null;

  private readonly messageListOpened = Signal.of<boolean>(false);

  private readonly duplicateNamesOpened = Signal.of<boolean>(false);

  private readonly duplicateNamesRef = UI.ref<MfdFmsFplnDuplicateNames>();

  // Necessary to enable mouse interaction
  get isInteractive(): boolean {
    return true;
  }

  public openMessageList() {
    this.messageListOpened.set(true);
  }

  /**
   * Called when a flight plan uplink is in progress
   */
  onUplinkInProgress() {
    this.props.fmcService.master.onUplinkInProgress();
  }

  /**
   * Called when a flight plan uplink is done
   */
  onUplinkDone(fltPlnReceived: boolean) {
    this.props.fmcService.master.onUplinkDone(fltPlnReceived);
  }

  /**
   * Calling this function with a message should display the message in the FMS' message area,
   * such as the scratchpad or a dedicated error line. The FMS error type given should be translated
   * into the appropriate message for the UI
   *
   * @param errorType the message to show
   */
  showFmsErrorMessage(errorType: FmsErrorType) {
    this.props.fmcService.master.showFmsErrorMessage(errorType);
  }

  /**
   * Calling this function with an array of items should display a UI allowing the user to
   * select the right item from a list of duplicates, and return the one chosen by the user or
   * `undefined` if the operation is cancelled.
   *
   * @param items the items to de-duplicate
   *
   * @returns the chosen item
   * @throws If another request came in over top of this one.
   */
  async deduplicateFacilities<T extends DatabaseItem<any>>(items: T[]): Promise<T | undefined> {
    if (items.length > 1) {
      return this.duplicateNamesRef.target.deduplicateFacilities(items);
    }
    return items[0];
  }

  /**
   * Calling this function should show a UI allowing the pilot to create a new waypoint with the ident
   * provided
   *
   * @param ident the identifier the waypoint should have
   *
   * @returns the created waypoint, or `undefined` if the operation is cancelled
   */
  // eslint-disable-next-line @typescript-eslint/no-unused-vars
  async createNewWaypoint(ident: string): Promise<Waypoint | undefined> {
    // TODO navigate to DATA/NAVAID --> PILOT STORED NAVAIDS --> NEW NAVAID
    return undefined;
  }

  /**
   * Checks whether a waypoint is currently in use
   * @param waypoint the waypoint to look for
   */
  async isWaypointInUse(waypoint: Waypoint): Promise<boolean> {
    return this.props.fmcService.master.isWaypointInUse(waypoint) ?? false;
  }

  public async afterRender(node: ViewNode): Promise<void> {
    super.afterRender(node);

    const isCaptainSide = this.props.captOrFo === 'CAPT';

    this.subs.push(
      this.props.bus
        .inbox<MfdSimvars>()
        .topic(isCaptainSide ? 'kccuOnL' : 'kccuOnR')
        .distinct()
        .listen((it) => this.interactionMode.set(it ? InteractionMode.Kccu : InteractionMode.Touchscreen)),
    );

    this.subs.push(
      this.props.bus
        .inbox<PanelEventTopics>()
        .topic('hEvent')
        .listen((eventName) => {
          if (eventName.startsWith(this.props.captOrFo === 'CAPT' ? 'A32NX_KCCU_L' : 'A32NX_KCCU_R')) {
            const key = eventName.substring(13);

            this.props.bus
              .outbox<InternalKccuKeyEvent>()
              .post('kccuKeyEvent', [this.props.captOrFo === 'CAPT' ? 'L' : 'R', key]);

            switch (key) {
              case 'DIR':
                this.navigateTo('fms/active/f-pln-direct-to');
                break;
              case 'PERF':
                this.navigateTo('fms/active/perf');
                break;
              case 'INIT':
                this.navigateTo('fms/active/init');
                break;
              case 'NAVAID':
                this.navigateTo('fms/position/navaids');
                break;
              case 'MAILBOX': // Move cursor to SD mail box
                break;
              case 'FPLN':
                this.navigateTo('fms/active/f-pln/top');
                break;
              case 'DEST':
                this.navigateTo('fms/active/f-pln/dest');
                break;
              case 'SECINDEX':
                this.navigateTo('fms/sec/index');
                break;
              case 'SURV':
                this.navigateTo('surv/controls');
                break;
              case 'ATCCOM':
                this.navigateTo('atccom/connect');
                break;
              case 'ND': // Move cursor to ND
                break;
              case 'CLRINFO':
                this.props.fmcService.master.clearLatestFmsErrorMessage();
                break;
              default:
                break;
            }
          }
        }),
    );

    this.subs.push(
      this.uiService.activeUri.watch((uri) => {
        this.activeUriChanged(uri);
      }),
    );

    this.topRef.target.addEventListener('mousemove', this.onMouseMoveHandler);

    this.subs.push(this.fmsDataKnob, this.fmcAIsHealthy, this.fmcBIsHealthy, this.activeFmsSource);
  }

  public navigateTo(uri: string) {
    const parsedUri = this.uiService.parseUri(uri);
    const flightPlans = this.props.fmcService.master?.flightPlanInterface;
    const tmpy = flightPlans?.hasTemporary ? flightPlans.temporary : null;
    const tmpyFromLeg = tmpy?.maybeElementAt(tmpy.activeLegIndex - 1);
    const directToInTmpy =
      tmpyFromLeg?.isDiscontinuity === false && (tmpyFromLeg.flags & FlightPlanLegFlags.DirectToTurningPoint) !== 0;
    if (parsedUri?.page == 'f-pln-direct-to' && tmpy && !directToInTmpy) {
      this.props.fmcService.master?.addMessageToQueue(NXSystemMessages.insertOrEraseTmpPlan, undefined, undefined);
      return;
    }

    this.uiService.navigateTo(uri);
  }

  private onMouseMove(ev: MouseEvent) {
    this.mouseCursorRef.peek()?.updatePosition(ev.clientX, ev.clientY);
  }

  private onMouseMoveHandler = this.onMouseMove.bind(this);

  private activeUriChanged(uri: ActiveUriInformation) {
    if (!this.props.fmcService.master) {
      return;
    }

    // Remove and destroy old header
    if (this.activeHeaderRef.peek()) {
      while (this.activeHeaderRef.target.firstChild) {
        this.activeHeaderRef.target.removeChild(this.activeHeaderRef.target.firstChild);
      }
    }

    if (this.activeHeader && this.activeHeader.target instanceof View) {
      this.activeHeader.target.destroy();
    }

    // Remove and destroy old MFD page
    if (this.activePageRef.peek()) {
      while (this.activePageRef.target.firstChild) {
        this.activePageRef.target.removeChild(this.activePageRef.target.firstChild);
      }
    }
    if (this.activePage && this.activePage.target instanceof View) {
      this.activePage.target.destroy();
    }

    // Different systems use different navigation bars
    this.activeHeader = headerForSystem(
      uri.sys,
      this,
      this.props.fmcService.master.fmgc.data.atcCallsign,
      this.activeFmsSource,
      this.uiService,
    );

    // Mapping from URL to page component
    if (uri.page) {
      this.activePage = pageForUrl(
        `${uri.sys}/${uri.category}/${uri.page}`,
        this.props.bus,
        this,
        this.props.fmcService,
        this.props.atcService,
      );
    } else {
      this.activePage = pageForUrl(
        `${uri.sys}/${uri.category}`,
        this.props.bus,
        this,
        this.props.fmcService,
        this.props.atcService,
      );
    }

    UI.mount(this.activeHeader, this.activeHeaderRef.peek());
    UI.mount(this.activePage, this.activePageRef?.peek());

    Vars.write(`L:A380X_MFD_${this.props.captOrFo === 'CAPT' ? 'L' : 'R'}_ACTIVE_PAGE`, 'string', uri.uri);
  }

  fmcChanged() {
    // Will be called if the FMC providing all the data has changed.
  }

  public logTroubleshootingError(msg: any) {
    logTroubleshootingError(this.props.bus, msg);
  }

  destroy(): void {
    this.topRef.peek()?.removeEventListener('mousemove', this.onMouseMoveHandler);
    this.mouseCursorRef.peek()?.destroy();
    this.duplicateNamesRef.peek()?.destroy();

    for (const s of this.subs) {
      s.destroy();
    }

    this.mouseCursorRef.target.destroy();
    this.duplicateNamesRef.target.destroy();

    if (this.activeHeader && this.activeHeader.target instanceof View) {
      this.activeHeader.target.destroy();
    }
    if (this.activePage && this.activePage.target instanceof View) {
      this.activePage.target.destroy();
    }

    super.destroy();
  }

  render(): ViewNode {
    return (
      <CdsDisplayUnit
        bus={this.props.bus}
        displayUnitId={this.props.captOrFo === 'CAPT' ? DisplayUnitID.CaptMfd : DisplayUnitID.FoMfd}
      >
        <div class="mfd-main" ref={this.topRef}>
          <div ref={this.activeHeaderRef} />
          <MfdFmsPageNotAvail
            bus={this.props.bus}
            fmcService={this.props.fmcService}
            captOrFo={this.props.captOrFo}
            requestedSystem={this.uiService.activeUri.derive((uri) => uri.sys)}
          />
          <MfdMsgList visible={this.messageListOpened} bus={this.props.bus} fmcService={this.props.fmcService} />
          <MfdFmsFplnDuplicateNames
            ref={this.duplicateNamesRef}
            visible={this.duplicateNamesOpened}
            fmcService={this.props.fmcService}
          />
          <div ref={this.activePageRef} class="mfd-navigator-container" />
          <MouseCursor isDoubleScreenMfd side={Signal.of(this.props.captOrFo)} ref={this.mouseCursorRef} />
        </div>
      </CdsDisplayUnit>
    );
  }
}

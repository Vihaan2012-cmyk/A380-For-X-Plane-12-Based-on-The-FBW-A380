// Copyright (c) 2021-2024 FlyByWire Simulations
//
// SPDX-License-Identifier: GPL-3.0

import {
  SimClock,
  BusSignal,
  HostedInstrument,
  UI,
  HostedDevice,
  PanelEventTopics,
  PanelEventPublisher,
  Board,
  Derived,
  Signal,
} from 'xplane-js/ui';
import {
  A380EfisNdRangeValue,
  a380EfisRangeSettings,
  a380NdModeChange,
  a380NdRangeChange,
  a380TerrainThresholdPadValue,
  ArincEventBus,
  BtvSimvarPublisher,
  EfisNdMode,
  EfisSide,
  FcuBusPublisher,
  FcuSimVars,
  FmsOansSimvarPublisher,
  LgciuBusPublisher,
  OansControlEvents,
  RaBusPublisher,
} from '@flybywiresim/fbw-sdk';
import { NDComponent } from '@flybywiresim/navigation-display';
import { a380EfisZoomRangeSettings, A380EfisZoomRangeValue, Oanc } from '@flybywiresim/oanc';

import { ContextMenu, ContextMenuElement } from '../MsfsAvionicsCommon/UiWidgets/ContextMenu';
import { MouseCursor, MouseCursorColor } from '../MsfsAvionicsCommon/UiWidgets/MouseCursor';
import { EraseSymbolsDialog, OansControlPanel } from './OansControlPanel';
import { FmsSymbolsPublisher } from './FmsSymbolsPublisher';
import { NDSimvarPublisher, NDSimvars } from './NDSimvarPublisher';
import { AdirsValueProvider } from '../MsfsAvionicsCommon/AdirsValueProvider';
import { FmsDataPublisher } from '../MsfsAvionicsCommon/providers/FmsDataPublisher';
import { VorBusPublisher } from '../MsfsAvionicsCommon/providers/VorBusPublisher';
import { TcasBusPublisher } from '../MsfsAvionicsCommon/providers/TcasBusPublisher';
import { FGDataPublisher } from '../MsfsAvionicsCommon/providers/FGDataPublisher';
import { NDControlEvents } from './NDControlEvents';
import { CdsDisplayUnit, DisplayUnitID, getDisplayIndex } from '../MsfsAvionicsCommon/CdsDisplayUnit';
import { EgpwcBusPublisher } from '../MsfsAvionicsCommon/providers/EgpwcBusPublisher';
import { DmcPublisher } from '../MsfsAvionicsCommon/providers/DmcPublisher';
import { FMBusPublisher } from '../MsfsAvionicsCommon/providers/FMBusPublisher';
import { ResetPanelSimvarPublisher, ResetPanelSimvars } from '../MsfsAvionicsCommon/providers/ResetPanelPublisher';
import { RopRowOansPublisher } from '@flybywiresim/msfs-avionics-common';
import { SimplaneValueProvider } from '../MsfsAvionicsCommon/providers/SimplaneValueProvider';
import { AesuBusPublisher } from '../MsfsAvionicsCommon/providers/AesuBusPublisher';
import { FcuEfisCpBusPublisher } from '@shared/publishers/EfisCpBusPublisher';
import { NDFMMessageTypes } from '@shared/FmMessages';

import './style.scss';
import './oans-style.scss';
import { VerticalDisplay } from './VerticalDisplay/VerticalDisplay';
import { InternalKccuKeyEvent } from '../MFD/shared/MFDSimvarPublisher';
import { WxrNdMessagePublisher } from './WxrNdMessagePublisher';
import { PanelInstrument, SessionPhase, SoundId, defineInstrument } from 'xplane-js/data';

declare type MousePosition = {
  x: number;
  y: number;
};

class NDInstrument implements HostedDevice {
  public readonly instrument!: PanelInstrument;

  private readonly efisSide: EfisSide;

  private readonly bus: ArincEventBus;

  private readonly backplane = new Board();

  private readonly simVarPublisher: NDSimvarPublisher;

  private readonly fcuBusPublisher: FcuBusPublisher;

  private readonly fmsDataPublisher: FmsDataPublisher;

  private readonly fmsOansSimvarPublisher: FmsOansSimvarPublisher;

  private readonly ropRowOansPublisher: RopRowOansPublisher;

  private readonly btvSimvarPublisher: BtvSimvarPublisher;

  private readonly fgDataPublisher: FGDataPublisher;

  private readonly fmBusPublisher: FMBusPublisher;

  private readonly fmsSymbolsPublisher: FmsSymbolsPublisher;

  private readonly vorBusPublisher: VorBusPublisher;

  private readonly tcasBusPublisher: TcasBusPublisher;

  private readonly dmcPublisher: DmcPublisher;

  private readonly egpwcBusPublisher: EgpwcBusPublisher;

  private readonly raBusPublisher: RaBusPublisher;

  private readonly lgciuBusPublisher: LgciuBusPublisher;

  private readonly hEventPublisher: PanelEventPublisher;

  private readonly resetPanelPublisher: ResetPanelSimvarPublisher;

  private readonly adirsValueProvider: AdirsValueProvider<NDSimvars>;

  private readonly simplaneValueProvider: SimplaneValueProvider;

  private readonly aesuPublisher: AesuBusPublisher;

  private readonly fcuEfisCpBusPublisher: FcuEfisCpBusPublisher;

  private readonly wxrNdMessagePublisher: WxrNdMessagePublisher;

  private readonly clock: SimClock;

  public readonly controlPanelRef = UI.ref<OansControlPanel>();

  private readonly contextMenuVisible = Signal.of(false);

  private readonly contextMenuX = Signal.of(0);

  private readonly contextMenuY = Signal.of(0);

  private readonly controlPanelVisible = Signal.of(false);

  private readonly eraseAllCrossesDialogVisible = Signal.of(false);

  private readonly eraseAllFlagsDialogVisible = Signal.of(false);

  private readonly eraseCrossIndex = Signal.of<number | null>(null);

  private readonly eraseFlagIndex = Signal.of<number | null>(null);

  private oansContextMenuItems = Signal.of(this.getOansContextMenu(false, false));

  private contextMenuRef = UI.ref<ContextMenu>();

  private contextMenuOpened = Signal.of<boolean>(false);

  private contextMenuPositionTriggered = Signal.of<MousePosition>({ x: 0, y: 0 });

  private mouseCursorRef = UI.ref<MouseCursor>();

  private topRef = UI.ref<HTMLDivElement>();

  private efisNdMode = EfisNdMode.ARC;

  private efisCpRange: A380EfisNdRangeValue = 10;

  private oansRef = UI.ref<Oanc<A380EfisZoomRangeValue>>();

  private cursorVisible = Signal.of<boolean>(true);

  private readonly oansNotAvailable = BusSignal.from(null, true);

  private readonly oansShown = Signal.of(false);

  constructor() {
    const side: EfisSide = getDisplayIndex() === 1 ? 'L' : 'R';
    const stateSubject = Signal.of<'L' | 'R'>(side);
    this.efisSide = side;

    this.bus = new ArincEventBus();

    this.simVarPublisher = new NDSimvarPublisher(this.bus);
    this.fcuBusPublisher = new FcuBusPublisher(this.bus, side);
    this.fmsDataPublisher = new FmsDataPublisher(this.bus, stateSubject);
    this.fmsOansSimvarPublisher = new FmsOansSimvarPublisher(this.bus);
    this.ropRowOansPublisher = new RopRowOansPublisher(this.bus);
    this.btvSimvarPublisher = new BtvSimvarPublisher(this.bus);
    this.fgDataPublisher = new FGDataPublisher(this.bus);
    this.fmBusPublisher = new FMBusPublisher(this.bus);
    this.fmsSymbolsPublisher = new FmsSymbolsPublisher(this.bus, side);
    this.vorBusPublisher = new VorBusPublisher(this.bus);
    this.tcasBusPublisher = new TcasBusPublisher(this.bus);
    this.dmcPublisher = new DmcPublisher(this.bus);
    this.egpwcBusPublisher = new EgpwcBusPublisher(this.bus, side);
    this.raBusPublisher = new RaBusPublisher(this.bus);
    this.lgciuBusPublisher = new LgciuBusPublisher(this.bus);
    this.hEventPublisher = new PanelEventPublisher(this.bus);
    this.resetPanelPublisher = new ResetPanelSimvarPublisher(this.bus);
    this.aesuPublisher = new AesuBusPublisher(this.bus);
    this.fcuEfisCpBusPublisher = new FcuEfisCpBusPublisher(this.bus);
    this.wxrNdMessagePublisher = new WxrNdMessagePublisher(this.bus, side);

    this.adirsValueProvider = new AdirsValueProvider(this.bus, this.simVarPublisher, side);
    this.simplaneValueProvider = new SimplaneValueProvider(this.bus);

    this.clock = new SimClock(this.bus);

    this.backplane.attachFeeder('ndSimVars', this.simVarPublisher);
    this.backplane.attachFeeder('fcu', this.fcuBusPublisher);
    this.backplane.attachFeeder('fms', this.fmsDataPublisher);
    this.backplane.attachFeeder('fms-oans', this.fmsOansSimvarPublisher);
    this.backplane.attachFeeder('rop-row-oans', this.ropRowOansPublisher);
    this.backplane.attachFeeder('btv', this.btvSimvarPublisher);
    this.backplane.attachFeeder('fg', this.fgDataPublisher);
    this.backplane.attachFeeder('fms-arinc', this.fmBusPublisher);
    this.backplane.attachFeeder('fms-symbols', this.fmsSymbolsPublisher);
    this.backplane.attachFeeder('vor', this.vorBusPublisher);
    this.backplane.attachFeeder('tcas', this.tcasBusPublisher);
    this.backplane.attachFeeder('dmc', this.dmcPublisher);
    this.backplane.attachFeeder('egpwc', this.egpwcBusPublisher);
    this.backplane.attachFeeder('ra', this.raBusPublisher);
    this.backplane.attachFeeder('lgciu', this.lgciuBusPublisher);
    this.backplane.attachFeeder('hEvent', this.hEventPublisher);
    this.backplane.attachFeeder('resetPanel', this.resetPanelPublisher);
    this.backplane.attachFeeder('aesu', this.aesuPublisher);
    this.backplane.attachFeeder('fcuEfisCp', this.fcuEfisCpBusPublisher);

    this.backplane.attachDevice('Simplane', this.simplaneValueProvider);
    this.backplane.attachDevice('clock', this.clock);

    this.doInit();
  }

  private doInit(): void {
    this.backplane.setup();

    this.adirsValueProvider.start();

    UI.mount(
      <div ref={this.topRef}>
        <CdsDisplayUnit
          bus={this.bus}
          displayUnitId={getDisplayIndex() === 1 ? DisplayUnitID.CaptNd : DisplayUnitID.FoNd}
          test={Signal.of(-1)}
          failed={Signal.of(false)}
        >
          <div
            class="oanc-container"
            style={{
              display: this.oansShown.derive((v) => (v ? 'block' : 'none')),
            }}
          >
            {/* This flag is rendered by the CDS so it lives here instead of in the OANC */}
            <div
              class="oanc-flag-container amber FontLarge"
              style={{
                visibility: this.oansNotAvailable.derive((v) => (v ? 'inherit' : 'hidden')),
              }}
            >
              NOT AVAIL
            </div>

            <div
              style={{
                display: this.oansNotAvailable.derive((v) => (v ? 'none' : 'block')),
              }}
            >
              <Oanc
                bus={this.bus}
                side={this.efisSide}
                ref={this.oansRef}
                contextMenuVisible={this.contextMenuVisible}
                contextMenuX={this.contextMenuX}
                contextMenuY={this.contextMenuY}
                zoomValues={a380EfisZoomRangeSettings}
              />
            </div>
          </div>
          <NDComponent
            bus={this.bus}
            side={this.efisSide}
            rangeValues={a380EfisRangeSettings}
            terrainThresholdPaddingText={a380TerrainThresholdPadValue}
            rangeChangeMessage={a380NdRangeChange}
            modeChangeMessage={a380NdModeChange}
            mapOptions={{ waypointBoxing: true, secondaryFlightPlanWaypointsInWhite: true }}
            fmMessages={Object.values(NDFMMessageTypes)}
          />
          <ContextMenu
            ref={this.contextMenuRef}
            opened={this.contextMenuOpened}
            idPrefix="contextMenu"
            values={this.oansContextMenuItems}
          />
          <EraseSymbolsDialog
            visible={Derived.from(
              ([crosses, flags, oans]) => crosses && !flags && oans,
              this.eraseAllCrossesDialogVisible,
              this.eraseAllFlagsDialogVisible,
              this.oansShown,
            )}
            confirmAction={() =>
              this.bus.outbox<OansControlEvents>().post('oans_erase_all_crosses', true, true, false)
            }
            hideDialog={() => this.eraseAllCrossesDialogVisible.set(false)}
            isCross={true}
          />
          <EraseSymbolsDialog
            visible={Derived.from(
              ([crosses, flags, oans]) => !crosses && flags && oans,
              this.eraseAllCrossesDialogVisible,
              this.eraseAllFlagsDialogVisible,
              this.oansShown,
            )}
            confirmAction={() =>
              this.bus.outbox<OansControlEvents>().post('oans_erase_all_flags', true, true, false)
            }
            hideDialog={() => this.eraseAllFlagsDialogVisible.set(false)}
            isCross={false}
          />
          <div
            style={{
              display: this.oansShown.derive((v) => (v ? 'block' : 'none')),
            }}
          >
            <OansControlPanel
              ref={this.controlPanelRef}
              bus={this.bus}
              side={this.efisSide}
              isVisible={this.controlPanelVisible}
              togglePanel={() => this.controlPanelVisible.set(!this.controlPanelVisible.get())}
            />
          </div>
          <MouseCursor
            ref={this.mouseCursorRef}
            side={Signal.of(this.efisSide === 'L' ? 'CAPT' : 'FO')}
            visible={this.cursorVisible}
            color={this.oansShown.derive((it) => (it ? MouseCursorColor.Magenta : MouseCursorColor.Yellow))}
          />
          <VerticalDisplay bus={this.bus} side={this.efisSide} />
        </CdsDisplayUnit>
      </div>,
      document.getElementById('ND_CONTENT'),
    );

    // Remove "instrument didn't load" text
    document.getElementById('ND_CONTENT')?.querySelector(':scope > h1')?.remove();

    this.topRef.target.addEventListener('mousemove', (ev) => {
      if (this.oansRef.peek() && this.oansRef.target.isPanning) {
        this.cursorVisible.set(false);
      } else {
        this.mouseCursorRef.target.updatePosition(ev.clientX, ev.clientY);
        this.cursorVisible.set(true);
      }
    });

    if (this.oansRef?.target?.labelContainerRef?.target) {
      this.oansRef.target.labelContainerRef.target.addEventListener('dblclick', (e) => {
        this.bus
          .outbox<OansControlEvents>()
          .post('oans_query_symbols_at_cursor', { side: this.efisSide, cursorPosition: [e.clientX, e.clientY] });
        this.contextMenuPositionTriggered.set({ x: e.clientX, y: e.clientY });
        this.contextMenuRef.target.display(e.clientX, e.clientY);
      });

      this.oansRef.target.labelContainerRef.target.addEventListener('click', () => {
        this.contextMenuRef.target.hideMenu();
      });
    }

    const sub = this.bus.inbox<FcuSimVars & OansControlEvents & ResetPanelSimvars & PanelEventTopics>();

    this.oansNotAvailable.rebind(sub.topic('oans_not_avail'));

    sub
      .topic('ndMode')
      .distinct()
      .listen((mode) => {
        this.efisNdMode = mode;
        this.updateNdOansVisibility();
      });

    sub
      .topic('ndRangeSetting')
      .distinct()
      .listen((range) => {
        this.efisCpRange = a380EfisRangeSettings[range];
        this.updateNdOansVisibility();
      });

    sub.topic('oans_answer_symbols_at_cursor').listen((symbols) => {
      if (symbols.side === this.efisSide) {
        this.eraseCrossIndex.set(symbols.cross);
        this.eraseFlagIndex.set(symbols.flag);
        this.oansContextMenuItems.set(this.getOansContextMenu(symbols.cross !== null, symbols.flag !== null));
      }
    });

    sub.topic('hEvent').listen((eventName) => {
      if (eventName.startsWith(this.efisSide === 'L' ? 'A32NX_KCCU_L' : 'A32NX_KCCU_R')) {
        const key = eventName.substring(13);

        this.bus.outbox<InternalKccuKeyEvent>().post('kccuKeyEvent', [this.efisSide, key]);
      }
    });
  }

  private updateNdOansVisibility() {
    if (this.efisCpRange === -1 && [EfisNdMode.PLAN, EfisNdMode.ARC, EfisNdMode.ROSE_NAV].includes(this.efisNdMode)) {
      this.bus.outbox<OansControlEvents>().post('nd_show_oans', { side: this.efisSide, show: true }, true, false);
      this.oansShown.set(true);
    } else {
      this.bus.outbox<OansControlEvents>().post('nd_show_oans', { side: this.efisSide, show: false }, true, false);
      this.oansShown.set(false);
    }
  }

  getOansContextMenu(hasCross: boolean, hasFlag: boolean): ContextMenuElement[] {
    const crossIndex = this.eraseCrossIndex.get();
    const flagIndex = this.eraseFlagIndex.get();
    return [
      {
        name: hasCross ? 'DELETE CROSS' : 'ADD CROSS',
        disabled: false,
        onPressed: () =>
          hasCross && crossIndex !== null
            ? this.bus.outbox<OansControlEvents>().post('oans_erase_cross_id', crossIndex)
            : this.bus
                .outbox<OansControlEvents>()
                .post('oans_add_cross_at_cursor', [
                  this.contextMenuPositionTriggered.get().x,
                  this.contextMenuPositionTriggered.get().y,
                ]),
      },
      {
        name: hasFlag ? 'DELETE FLAG' : 'ADD FLAG',
        disabled: false,
        onPressed: () =>
          hasFlag && flagIndex !== null
            ? this.bus.outbox<OansControlEvents>().post('oans_erase_flag_id', flagIndex)
            : this.bus
                .outbox<OansControlEvents>()
                .post('oans_add_flag_at_cursor', [
                  this.contextMenuPositionTriggered.get().x,
                  this.contextMenuPositionTriggered.get().y,
                ]),
      },
      {
        name: 'MAP DATA',
        disabled: false,
        onPressed: () => {
          if (this.controlPanelRef.peek()) {
            this.controlPanelVisible.set(!this.controlPanelVisible.get());
          }
        },
      },
      {
        name: 'ERASE ALL CROSSES',
        disabled: false,
        onPressed: () => this.eraseAllCrossesDialogVisible.set(true),
      },
      {
        name: 'ERASE ALL FLAGS',
        disabled: false,
        onPressed: () => this.eraseAllFlagsDialogVisible.set(true),
      },
      {
        name: 'CENTER ON ACFT',
        disabled: false,
        onPressed: async () => {
          this.bus.outbox<OansControlEvents>().post('oans_center_on_acft', true, true, false);
        },
      },
    ];
  }

  /**
   * A callback called when the instrument gets a frame update.
   */
  public onFrame(): void {
    this.backplane.tick();

    if (this.oansRef.peek()) {
      this.oansRef.target.Update();
    }
  }

  public onPanelEvent(args: string[]): void {
    if (args[0].endsWith(`A32NX_EFIS_${this.efisSide}_CHRONO_PUSHED`)) {
      this.bus.outbox<NDControlEvents>().post('chrono_pushed', undefined);
    }

    this.hEventPublisher.dispatchPanelEvent(args[0]);
  }

  onSessionChange(_oldState: SessionPhase, _newState: SessionPhase) {
    // noop
  }

  onFlightBegin() {
    // noop
  }

  onSoundDone(_soundEventId: SoundId) {
    // noop
  }
}

class A380X_ND extends HostedInstrument<NDInstrument> {
  createDevice(): NDInstrument {
    return new NDInstrument();
  }

  get interactive(): boolean {
    return true;
  }

  get templateId(): string {
    return 'A380X_ND';
  }
}

defineInstrument('a380x-nd', A380X_ND);

// Copyright (c) 2024 FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0

import './oans-style.scss';
import './OansControlPanel.scss';

import { Feature, Geometry, LineString, Point, Position } from 'geojson';
import { LengthFormat } from '../MFD/pages/common/DataEntryFormats';
import { InternalKccuKeyEvent } from '../MFD/shared/MFDSimvarPublisher';
import { ResetPanelSimvars } from '../MsfsAvionicsCommon/providers/ResetPanelPublisher';
import { AdirsSimVars } from '../MsfsAvionicsCommon/SimVarTypes';
import { Button } from '../MsfsAvionicsCommon/UiWidgets/Button';
import { DropdownMenu } from '../MsfsAvionicsCommon/UiWidgets/DropdownMenu';
import { IconButton } from '../MsfsAvionicsCommon/UiWidgets/IconButton';
import { InputField, InteractionMode } from '../MsfsAvionicsCommon/UiWidgets/InputField';
import { RadioButtonGroup } from '../MsfsAvionicsCommon/UiWidgets/RadioButtonGroup';
import { TopTabNavigator, TopTabNavigatorPage } from '../MsfsAvionicsCommon/UiWidgets/TopTabNavigator';
import { NDSimvars } from './NDSimvarPublisher';
import { Coordinates, distanceTo } from 'msfs-geo';

import {
  AmdbAirportSearchResult,
  AmdbProperties,
  Arinc429LocalVarConsumerSubject,
  BTV_MIN_TOUCHDOWN_ZONE_DISTANCE,
  BtvData,
  EfisSide,
  FeatureType,
  FeatureTypeString,
  FmsOansData,
  LgciuBusEvents,
  NXDataStore,
  NXLogicConfirmNode,
  OansControlEvents,
  OansFmsDataStore,
  RegisteredSimVar,
} from '@flybywiresim/fbw-sdk';
import {
  ControlPanelAirportSearchMode,
  ControlPanelMapDataSearchMode,
  ControlPanelStore,
  ControlPanelUtils,
  NavigraphAmdbClient,
} from '@flybywiresim/oanc';
import { NavigationDatabaseService } from '@fmgc/flightplanning/NavigationDatabaseService';
import { NavigationDatabase, NavigationDatabaseBackend } from '@fmgc/NavigationDatabase';
import {
  SignalList,
  ClockTopics,
  ViewProps,
  BusSignal,
  View,
  Bus,
  UI,
  Derived,
  SignalMap,
  NumFormat,
  QuantitySignal,
  VarUnit,
  Signal,
  Readable,
  Bond,
  UnitList,
  ViewNode,
} from 'xplane-js/ui';

import { OansRunwayInfoBox } from './OANSRunwayInfoBox';
import { Bridge, Vars } from 'xplane-js/data';

export interface OansProps extends ViewProps {
  bus: Bus;
  side: EfisSide;
  isVisible: Readable<boolean>;
  togglePanel: () => void;
}

const months = ['JAN', 'FEB', 'MAR', 'APR', 'MAY', 'JUN', 'JUL', 'AUG', 'SEP', 'OCT', 'NOV', 'DEC'];

export class OansControlPanel extends View<OansProps> {
  private readonly subs: Bond[] = [];

  private readonly sub = this.props.bus.inbox<
    AdirsSimVars &
      BtvData &
      ClockTopics &
      FmsOansData &
      LgciuBusEvents &
      NDSimvars &
      OansControlEvents &
      ResetPanelSimvars
  >();

  /** If navigraph not available, this class will compute BTV features */
  private readonly navigraphAvailable = Signal.of(false);
  private readonly oansResetPulled = BusSignal.from(this.sub.topic('a380x_reset_panel_arpt_nav'), false);

  private oansPerformanceModeSettingSub = () => {};
  private readonly oansPerformanceMode = Signal.of(false);
  private showOans = false;
  private lastUpdateTime: number | null = null;
  private readonly oansPerformanceModeAndMovedOutOfZoomRange = new NXLogicConfirmNode(60, true);

  private readonly oansAvailable = Derived.from(
    ([ng, reset]) => ng && !reset,
    this.navigraphAvailable,
    this.oansResetPulled,
  );

  private readonly oansFailed = Derived.from(([reset]) => reset, this.oansResetPulled);

  private amdbClient = new NavigraphAmdbClient();

  private readonly lengthUnit = NXDataStore.getSetting('CONFIG_USING_METRIC_UNIT').derive((v) =>
    v ? UnitList.METER : UnitList.FOOT,
  );

  private readonly lengthUnitText = this.lengthUnit.derive((v) => (v === UnitList.FOOT ? 'FT' : 'M'));

  private readonly lengthFormatter = NumFormat.make({
    nanString: '',
    precision: 1,
  });

  private readonly oansMenuRef = UI.ref<HTMLDivElement>();

  private readonly airportSearchAirportDropdownRef = UI.ref<DropdownMenu>();

  private readonly displayAirportButtonRef = UI.ref<Button>();

  private readonly closePanelButtonRef = UI.ref<HTMLButtonElement>();

  private readonly mapDataLdgShiftPanelRef = UI.ref<HTMLDivElement>();

  private readonly mapDataMainRef = UI.ref<HTMLDivElement>();

  private readonly mapDataBtvFallback = UI.ref<HTMLDivElement>();

  private readonly store = new ControlPanelStore();

  private readonly style = SignalMap.of<string, string>();

  private readonly activeTabIndex = Signal.of<number>(2);

  private readonly availableEntityTypes = ['RWY', 'TWY', 'STAND', 'OTHER'];

  private mapDataFeatures: Feature<Geometry, AmdbProperties>[] | undefined = undefined;

  private readonly thresholdShift = Signal.of<number | null>(null);

  private readonly endShift = Signal.of<number | null>(null);

  private readonly selectedEntityType = Signal.of<ControlPanelMapDataSearchMode | null>(
    ControlPanelMapDataSearchMode.Runway,
  );

  private readonly availableEntityList = SignalList.of(['']);

  private readonly selectedEntityIndex = Signal.of<number | null>(0);

  private readonly selectedEntityString = Signal.of<string | null>(null);

  private readonly entityIsNotSelected = this.selectedEntityIndex.derive((i) => i === null);

  private selectedEntityPosition: Position = [];

  private readonly selectedFeatureId = Signal.of<number | null>(null);
  private readonly selectedFeatureType = Signal.of<FeatureType | null>(null);

  private readonly symbolsForFeatureIds = BusSignal.from(this.sub.topic('oans_symbols_for_feature_ids'), {
    featureIdsWithCrosses: [],
    featureIdsWithFlags: [],
  });

  private readonly flagExistsForEntity = Derived.from(
    ([symbols, id]) => symbols.featureIdsWithFlags.some((f) => f === id),
    this.symbolsForFeatureIds,
    this.selectedFeatureId,
  );

  private readonly crossExistsForEntity = Derived.from(
    ([symbols, id]) => symbols.featureIdsWithCrosses.some((f) => f === id),
    this.symbolsForFeatureIds,
    this.selectedFeatureId,
  );

  private manualAirportSelection = false;
  private manualAirportSelectionTime: number = 0;

  // TODO: Should be using GPS position interpolated with IRS velocity data
  private readonly pposLatWord = Arinc429LocalVarConsumerSubject.create(this.sub.topic('latitude'));

  private readonly pposLongWord = Arinc429LocalVarConsumerSubject.create(this.sub.topic('longitude'));

  private readonly presentPos = Derived.from(
    ([lat, lon]) => {
      return { lat: lat.value, long: lon.value } as Coordinates;
    },
    this.pposLatWord,
    this.pposLongWord,
  );

  private readonly presentPosNotAvailable = Derived.from(
    ([lat, long]) => !lat.isNormalOperation() || !long.isNormalOperation(),
    this.pposLatWord,
    this.pposLongWord,
  );

  private readonly setPlanModeConsumer = BusSignal.from(this.sub.topic('oans_show_set_plan_mode'), false);
  private readonly setPlanModeDisplay = this.setPlanModeConsumer.derive((it) => (it ? 'inherit' : 'none'));

  private readonly fmsDataStore = new OansFmsDataStore(this.props.bus);

  private readonly runwayTora = QuantitySignal.of(UnitList.METER.quantity(NaN));

  private readonly runwayLda = QuantitySignal.of(UnitList.METER.quantity(NaN));

  private readonly runwayLdaText = Derived.from(
    ([lda, unit]) => this.lengthFormatter(lda.inUnit(unit)),
    this.runwayLda,
    this.lengthUnit,
  );

  private readonly runwayToraText = Derived.from(
    ([tora, unit]) => this.lengthFormatter(tora.inUnit(unit)),
    this.runwayTora,
    this.lengthUnit,
  );

  private readonly standCoordinateString = Signal.of<string>('');

  private readonly oansRequestedStoppingDistance = Arinc429LocalVarConsumerSubject.create(
    this.sub.topic('oansRequestedStoppingDistance'),
  );

  private readonly simTimeVar = RegisteredSimVar.create<number>('E:fbw/SIMULATION_TIME', VarUnit.Seconds);

  // Need to add touchdown zone distance to displayed value. BTV computes from TDZ internally,
  // but users enter LDA from threshold
  private readonly reqStoppingDistance = this.oansRequestedStoppingDistance.derive((it) =>
    it.isNormalOperation() ? Math.round(it.value + BTV_MIN_TOUCHDOWN_ZONE_DISTANCE) : null,
  );

  private readonly fmsLandingRunwayNotSelectedInFallback = Derived.from(
    ([ldgRwy, avail]) => !avail && ldgRwy === null,
    this.fmsDataStore.landingRunway,
    this.oansAvailable,
  );
  private readonly fmsLandingRunwayVisibility = this.fmsLandingRunwayNotSelectedInFallback.derive((notSelected) =>
    !notSelected ? 'inherit' : 'hidden',
  );

  private readonly ropsDetectedAirport = BusSignal.from(this.sub.topic('ropsDetectedAirport'), null);
  private readonly ropsDetectedRunway = BusSignal.from(this.sub.topic('ropsDetectedRunway'), null);
  private readonly ropsDetectedRunwayLda = BusSignal.from(this.sub.topic('ropsDetectedRunwayLda'), null);
  private readonly oansSelectedLandingRunway = BusSignal.from(this.sub.topic('oansSelectedLandingRunway'), null);

  private readonly airportDatabase = this.navigraphAvailable.derive((a) => (a ? 'FBW9027250BB04' : 'N/A'));

  private readonly activeDatabase = Signal.of('30DEC-27JAN');

  public hEventConsumer = this.props.bus.inbox<InternalKccuKeyEvent>().topic('kccuKeyEvent');

  public interactionMode = Signal.of<InteractionMode>(InteractionMode.Touchscreen);

  private showLdgShiftPanel() {
    if (this.mapDataLdgShiftPanelRef.peek() && this.mapDataMainRef.peek()) {
      this.mapDataLdgShiftPanelRef.target.style.display = 'flex';
      this.mapDataMainRef.target.style.display = 'none';
    }
  }

  private hideLdgShiftPanel() {
    if (this.mapDataLdgShiftPanelRef.peek() && this.mapDataMainRef.peek()) {
      this.mapDataLdgShiftPanelRef.target.style.display = 'none';
      this.mapDataMainRef.target.style.display = 'flex';
    }
  }

  private loadOansDb() {
    this.amdbClient
      .searchForAirports('')
      .then((airports) => {
        this.store.airports.set(airports);
        this.navigraphAvailable.set(true);
      })
      .catch(() => {
        this.store.airports.set([]);
        this.navigraphAvailable.set(false);
      });
  }

  public afterRender(node: ViewNode): void {
    super.afterRender(node);

    new Promise((resolve) => setTimeout(resolve, 5000)).then(() => {});

    // FIXME this should only ever be used within the FMGC
    NavigationDatabaseService.activeDatabase = new NavigationDatabase(this.props.bus, NavigationDatabaseBackend.Msfs);

    NavigationDatabaseService.activeDatabase.getDatabaseIdent().then((db) => {
      const from = new Date(db.effectiveFrom);
      const to = new Date(db.effectiveTo);
      this.activeDatabase.set(`${from.getDate()}${months[from.getMonth()]}-${to.getDate()}${months[to.getMonth()]}`);
    });

    NXDataStore.getAndSubscribeLegacy('NAVIGRAPH_ACCESS_TOKEN', () => this.loadOansDb());

    this.subs.push(
      this.props.isVisible.watch((it) => this.style.assign('visibility', it ? 'inherit' : 'hidden'), true),
    );

    this.subs.push(
      this.store.airports.watch(() =>
        this.sortAirports(this.store.airportSearchMode.get() ?? ControlPanelAirportSearchMode.Icao),
      ),
    );

    this.subs.push(
      this.store.airportSearchMode.watch((mode) => this.sortAirports(mode ?? ControlPanelAirportSearchMode.Icao)),
    );

    this.subs.push(
      this.store.airportSearchMode.watch(() => this.updateAirportSearchData(), true),
      this.store.sortedAirports.watch(() => this.updateAirportSearchData(), true),
    );

    // unfocus input fields on tab change
    this.subs.push(this.activeTabIndex.watch((_index) => Bridge.signal('input.blur')));

    this.subs.push(
      this.oansAvailable.watch((v) => {
        if (this.mapDataMainRef.peek() && this.mapDataBtvFallback.peek()) {
          this.mapDataMainRef.target.style.display = v ? 'block' : 'none';
          this.mapDataBtvFallback.target.style.display = v ? 'none' : 'block';
        }
        Vars.write('L:A32NX_OANS_AVAILABLE', VarUnit.Bool, v);
        this.props.bus.outbox<OansControlEvents>().post('oans_not_avail', !v, true, false);
      }, true),
      this.oansFailed.watch((v) => {
        Vars.write('L:A32NX_OANS_FAILED', VarUnit.Bool, v);
      }, true),
    );

    this.subs.push(
      this.oansResetPulled.watch((v) => {
        if (v) {
          this.unloadCurrentAirport();
        }
      }, true),
    );

    this.oansPerformanceModeSettingSub = NXDataStore.getAndSubscribeLegacy(
      'CONFIG_A380X_OANS_PERFORMANCE_MODE',
      (_, v) => this.oansPerformanceMode.set(v === '1'),
      '0',
    );

    this.subs.push(
      Derived.from(
        async ([arpt, ropsRwy, lda, oansRwy]) => {
          if (oansRwy || (arpt && ropsRwy)) {
            this.runwayLda.set(lda ?? NaN);
            this.runwayTora.set(lda ?? NaN);
          } else {
            this.clearRunwayInfo();
          }
        },
        this.ropsDetectedAirport,
        this.ropsDetectedRunway,
        this.ropsDetectedRunwayLda,
        this.oansSelectedLandingRunway,
      ),
    );

    this.subs.push(
      this.sub
        .topic('nd_show_oans')
        .distinct()
        .listen((showOans) => {
          if (this.props.side === showOans.side) {
            this.showOans = showOans.show;
          }
        }),
    );

    this.subs.push(
      this.sub
        .topic('realTime')
        .limit(0.5)
        .listen((time) => {
          this.oansPerformanceModeAndMovedOutOfZoomRange.write(
            this.oansPerformanceMode.get() && !this.showOans,
            this.lastUpdateTime === null ? 0 : time - this.lastUpdateTime,
          );
          this.props.bus.outbox<OansControlEvents>().post(
            'oans_performance_mode_hide',
            {
              side: this.props.side,
              hide: this.oansPerformanceModeAndMovedOutOfZoomRange.read(),
            },
            true,
          );
          this.autoLoadAirport();

          this.lastUpdateTime = time;
        }),
    );

    this.subs.push(this.sub.topic('oans_display_airport').listen((arpt) => this.handleSelectAirport(arpt)));

    this.subs.push(
      this.selectedEntityIndex.watch((val) => {
        const searchMode = this.selectedEntityType.get();
        if (searchMode !== null && this.mapDataFeatures && val !== null) {
          const prop = ControlPanelUtils.getMapDataSearchModeProp(searchMode);
          const idx = this.mapDataFeatures.findIndex((f) => f.properties[prop] === this.availableEntityList.get(val));
          this.selectedEntityString.set(
            idx !== -1 ? this.mapDataFeatures[idx]?.properties[prop]?.toString() ?? '' : '',
          );

          if (
            (idx !== -1 && searchMode === ControlPanelMapDataSearchMode.Runway) ||
            searchMode === ControlPanelMapDataSearchMode.Stand
          ) {
            const feature = this.mapDataFeatures[idx] as Feature<Point>;
            this.selectedEntityPosition = feature.geometry.coordinates;
            this.selectedFeatureId.set(feature.properties?.id);
            this.selectedFeatureType.set(feature.properties?.feattype);
          } else if (
            idx !== -1 &&
            (searchMode === ControlPanelMapDataSearchMode.Taxiway || searchMode === ControlPanelMapDataSearchMode.Other)
          ) {
            const taxiway = this.mapDataFeatures[idx] as Feature<LineString, AmdbProperties>;
            this.selectedEntityPosition = taxiway.properties.midpoint?.coordinates ?? [0, 0];
            this.selectedFeatureId.set(taxiway.properties?.id);
            this.selectedFeatureType.set(taxiway.properties?.feattype);
          }

          if (idx !== -1 && this.selectedEntityType.get() === ControlPanelMapDataSearchMode.Runway) {
            this.runwayLda.set(this.mapDataFeatures[idx].properties.lda ?? NaN);
            this.runwayTora.set(this.mapDataFeatures[idx].properties.tora ?? NaN);
          }
        } else if (this.oansAvailable.get()) {
          this.selectedEntityString.set('');
          this.runwayLda.set(NaN);
          this.runwayTora.set(NaN);
        }
      }, true),
    );
    this.subs.push(
      this.selectedEntityType.watch((v) => this.handleSelectMapDataSearchMode(v ?? ControlPanelMapDataSearchMode.Runway)),
    );

    this.subs.push(
      this.sub
        .topic(this.props.side === 'L' ? 'kccuOnL' : 'kccuOnR')
        .distinct()
        .listen((it) => this.interactionMode.set(it ? InteractionMode.Kccu : InteractionMode.Touchscreen)),
    );

    this.subs.push(
      this.lengthUnit,
      this.lengthUnitText,
      this.runwayLdaText,
      this.runwayToraText,
      this.setPlanModeConsumer,
      this.setPlanModeDisplay,
      this.oansResetPulled,
      this.oansAvailable,
      this.entityIsNotSelected,
      this.symbolsForFeatureIds,
      this.flagExistsForEntity,
      this.crossExistsForEntity,
      this.pposLatWord,
      this.pposLongWord,
      this.presentPos,
      this.presentPosNotAvailable,
      this.oansRequestedStoppingDistance,
      this.reqStoppingDistance,
      this.fmsLandingRunwayVisibility,
      this.airportDatabase,
    );
  }

  public updateAirportSearchData() {
    const searchMode = this.store.airportSearchMode.get();
    const sortedAirports = this.store.sortedAirports.snapshot();

    const prop = ControlPanelUtils.getSearchModeProp(searchMode ?? ControlPanelAirportSearchMode.Icao);

    this.store.airportSearchData.set(sortedAirports.map((it) => (it[prop] as string).toUpperCase()));
  }

  public setSelectedAirport(airport: AmdbAirportSearchResult) {
    this.store.selectedAirport.set(airport);
    const foundIndex = this.store.sortedAirports.snapshot().findIndex((it) => it.idarpt === airport.idarpt);
    this.store.airportSearchSelectedAirportIndex.set(foundIndex === -1 ? null : foundIndex);
  }

  private sortAirports(mode: ControlPanelAirportSearchMode) {
    const array = this.store.airports.snapshot().slice();

    const prop = ControlPanelUtils.getSearchModeProp(mode);

    array.sort((a, b) => {
      if (a[prop] == null || b[prop] == null) {
        return 0;
      }

      if (a[prop] < b[prop]) {
        return -1;
      }
      if (a[prop] > b[prop]) {
        return 1;
      }
      return 0;
    });

    this.store.sortedAirports.set(array.filter((it) => it[prop] !== null));
  }

  private handleSelectAirport = async (icao: string, indexInSearchData?: number) => {
    const airport = this.store.airports.snapshot().find((it) => it.idarpt === icao);
    const prop = ControlPanelUtils.getSearchModeProp(
      this.store.airportSearchMode.get() ?? ControlPanelAirportSearchMode.Icao,
    );

    if (!airport || typeof airport[prop] !== 'string') {
      throw new Error('');
    }

    const firstLetter = airport[prop][0];
    this.store.airportSearchSelectedSearchLetterIndex.set(
      ControlPanelUtils.LETTERS.findIndex((it) => it === firstLetter),
    );

    const airportIndexInSearchData =
      indexInSearchData ?? this.store.sortedAirports.snapshot().findIndex((it) => it.idarpt === icao);

    this.store.airportSearchSelectedAirportIndex.set(airportIndexInSearchData);
    this.store.selectedAirport.set(airport);

    this.handleSelectMapDataSearchMode(ControlPanelMapDataSearchMode.Runway);

    this.store.isAirportSelectionPending.set(true);
  };

  private handleSelectMapDataSearchMode = async (newSearchMode: ControlPanelMapDataSearchMode) => {
    const selectedAirport = this.store.selectedAirport.get();
    this.selectedEntityIndex.set(null);

    if (selectedAirport !== null) {
      let featureType: FeatureTypeString = FeatureTypeString.RunwayThreshold;
      switch (newSearchMode) {
        case ControlPanelMapDataSearchMode.Runway:
          featureType = FeatureTypeString.RunwayThreshold;
          break;
        case ControlPanelMapDataSearchMode.Taxiway:
          featureType = FeatureTypeString.TaxiwayGuidanceLine;
          break;
        case ControlPanelMapDataSearchMode.Stand:
          featureType = FeatureTypeString.ParkingStandLocation;
          break;
        case ControlPanelMapDataSearchMode.Other:
          featureType = FeatureTypeString.DeicingArea;
          break;
        default:
          break;
      }
      // Populate MAP DATA
      const data = await this.amdbClient.getAirportData(selectedAirport.idarpt, [featureType]);
      this.mapDataFeatures = data[featureType]?.features;
      if (this.mapDataFeatures) {
        const prop = ControlPanelUtils.getMapDataSearchModeProp(newSearchMode);
        const entityData = this.mapDataFeatures
          .map((f) => f.properties[prop]?.toString().trim().substring(0, 6) ?? '')
          .filter((it) => it);
        this.availableEntityList.set([...new Set(entityData)].sort());
      }
    }
  };

  private handleSelectAirportSearchMode = (newSearchMode: ControlPanelAirportSearchMode) => {
    const selectedAirport = this.store.selectedAirport.get();

    this.store.airportSearchMode.set(newSearchMode);
    const prop = ControlPanelUtils.getSearchModeProp(newSearchMode);

    if (selectedAirport !== null && typeof selectedAirport[prop] === 'string') {
      const firstLetter = selectedAirport[prop][0];
      const airportIndexInSearchData = this.store.sortedAirports
        .snapshot()
        .findIndex((it) => it.idarpt === selectedAirport.idarpt);

      this.store.airportSearchSelectedSearchLetterIndex.set(
        ControlPanelUtils.LETTERS.findIndex((it) => it === firstLetter),
      );
      this.store.airportSearchSelectedAirportIndex.set(airportIndexInSearchData);
    }
  };

  private handleDisplayAirport = () => {
    const selectedArpt = this.store.selectedAirport.get();
    if (!selectedArpt || !selectedArpt.idarpt) {
      throw new Error('[OANS] Empty airport selected for display.');
    }

    this.manualAirportSelection = true;
    this.manualAirportSelectionTime = this.simTimeVar.get();
    this.props.bus.outbox<OansControlEvents>().post('oans_display_airport', selectedArpt.idarpt, true);
    this.store.loadedAirport.set(selectedArpt);
    this.store.isAirportSelectionPending.set(false); // TODO should be done when airport is fully loaded
  };

  private handleCrossButton() {
    {
      const selId = this.selectedFeatureId.get();
      const selFeatType = this.selectedFeatureType.get();
      if (selId !== null && selFeatType !== null) {
        this.props.bus
          .outbox<OansControlEvents>()
          .post(
            this.crossExistsForEntity.get() ? 'oans_remove_cross_at_feature' : 'oans_add_cross_at_feature',
            { id: selId, feattype: selFeatType },
            true,
          );
      }
    }
  }

  private handleFlagButton() {
    {
      const selId = this.selectedFeatureId.get();
      const selFeatType = this.selectedFeatureType.get();
      if (selId !== null && selFeatType !== null) {
        this.props.bus
          .outbox<OansControlEvents>()
          .post(
            this.flagExistsForEntity.get() ? 'oans_remove_flag_at_feature' : 'oans_add_flag_at_feature',
            { id: selId, feattype: selFeatType },
            true,
          );
      }
    }
  }

  private unloadCurrentAirport() {
    if (this.store.loadedAirport.get()) {
      this.props.bus.outbox<OansControlEvents>().post('oans_display_airport', '', true);
      this.store.loadedAirport.set(null);
      this.store.isAirportSelectionPending.set(false);
    }
  }

  private autoLoadAirport() {
    // If we don't have ppos or airport unloaded due to performance reasons, do not try to auto load
    // If airport has been manually selected within the last 10 minutes, do not auto load.
    // FIXME reset manualAirportSelection after a while, to enable auto-load for destination even if departure was selected manually
    if (
      this.presentPosNotAvailable.get() ||
      this.oansPerformanceModeAndMovedOutOfZoomRange.read() ||
      (this.manualAirportSelection === true && this.simTimeVar.get() - this.manualAirportSelectionTime < 600) ||
      this.store.loadedAirport.get() !== this.store.selectedAirport.get() ||
      this.store.airports.length === 0 ||
      this.oansResetPulled.get()
    ) {
      return;
    }
    // If on ground, and no airport is loaded, find current airport.
    if (![6, 7, 8, 9].includes(Vars.read('L:A32NX_FWC_FLIGHT_PHASE', VarUnit.Number))) {
      // Go through all airports, load if distance <20NM
      const nearestAirports = this.store.airports
        .snapshot()
        .filter((ap) => distanceTo(this.presentPos.get(), { lat: ap.coordinates.lat, long: ap.coordinates.lon }) < 20);
      const sortedAirports = nearestAirports.sort(
        (a, b) =>
          distanceTo(this.presentPos.get(), { lat: a.coordinates.lat, long: a.coordinates.lon }) -
          distanceTo(this.presentPos.get(), { lat: b.coordinates.lat, long: b.coordinates.lon }),
      );
      if (sortedAirports.length > 0) {
        const ap = sortedAirports[0];
        if (ap.idarpt !== this.store.loadedAirport.get()?.idarpt) {
          this.props.bus.outbox<OansControlEvents>().post('oans_display_airport', ap.idarpt, true);
          this.store.loadedAirport.set(ap);
          this.store.isAirportSelectionPending.set(false); // TODO should be done when airport is fully loaded
        }
        return;
      }
    }
    // If in flight, load destination airport if distance is <50NM. This could cause stutters, consider deactivating.
    else {
      const destArpt = this.store.airports.snapshot().find((it) => it.idarpt === this.fmsDataStore.destination.get());
      if (destArpt && destArpt.idarpt !== this.store.loadedAirport.get()?.idarpt) {
        if (distanceTo(this.presentPos.get(), { lat: destArpt.coordinates.lat, long: destArpt.coordinates.lon }) < 50) {
          this.props.bus.outbox<OansControlEvents>().post('oans_display_airport', destArpt.idarpt, true);
          this.store.loadedAirport.set(destArpt);
          this.store.isAirportSelectionPending.set(false); // TODO should be done when airport is fully loaded
          return;
        }
      }
    }
  }

  private clearRunwayInfo() {
    this.selectedEntityIndex.set(null);
    this.selectedEntityString.set(null);
    this.runwayLda.set(NaN);
    this.runwayTora.set(NaN);
  }

  destroy(): void {
    for (const s of this.subs) {
      s.destroy();
    }
    this.oansPerformanceModeSettingSub();
    super.destroy();
  }

  render(): ViewNode {
    return (
      <>
        <div style={{ display: this.props.isVisible.derive((v) => (v ? 'inherit' : 'none')) }}>
          <IconButton
            ref={this.closePanelButtonRef}
            onClick={() => this.props.togglePanel()}
            icon="double-up"
            containerStyle="z-index: 10; width: 49px; height: 45px; position: absolute; right: 2px; top: 768px;"
          />
        </div>
        <div style={{ display: this.props.isVisible.derive((v) => (v ? 'none' : 'inherit')) }}>
          <IconButton
            ref={this.closePanelButtonRef}
            onClick={() => this.props.togglePanel()}
            icon="double-down"
            containerStyle="z-index: 10; width: 49px; height: 45px; position: absolute; right: 2px; top: 768px;"
          />
        </div>
        <div class="oans-control-panel-background">
          <div ref={this.oansMenuRef} class="oans-control-panel" style={this.style}>
            <TopTabNavigator
              pageTitles={Signal.of(['MAP DATA', 'ARPT SEL', 'STATUS'])}
              selectedPageIndex={this.activeTabIndex}
              tabBarHeight={45}
              tabBarSlantedEdgeAngle={30}
              selectedTabTextColor="cyan"
              additionalRightSpace={50}
            >
              <TopTabNavigatorPage>
                <div class="oans-cp-map-data-tab">
                  <div class="oans-cp-map-data-left">
                    <DropdownMenu
                      values={this.availableEntityList}
                      selectedIndex={this.selectedEntityIndex}
                      idPrefix="oanc-search-letter"
                      freeTextAllowed={false}
                      onModified={(i) => this.selectedEntityIndex.set(i)}
                      inactive={this.fmsLandingRunwayNotSelectedInFallback}
                      hEventConsumer={this.hEventConsumer}
                      interactionMode={this.interactionMode}
                    />
                    <div class="oans-cp-map-data-entitytype">
                      <RadioButtonGroup
                        values={this.availableEntityTypes}
                        valuesDisabled={Signal.of(Array(4).fill(false))}
                        selectedIndex={this.selectedEntityType}
                        idPrefix="entityTypesRadio"
                      />
                    </div>
                  </div>
                  <div ref={this.mapDataLdgShiftPanelRef} class="oans-cp-map-data-ldg-shift">
                    <div class="oans-cp-map-data-ldg-shift-rwy">
                      <div class="mfd-label-value-container" style="padding: 15px;">
                        <span class="mfd-label mfd-spacing-right">RWY</span>
                        <span class="mfd-value">{this.selectedEntityString}</span>
                      </div>
                    </div>
                    <div class="oans-cp-map-data-ldg-shift-1">
                      <div class="oans-cp-map-data-ldg-shift-2">
                        <div class="oans-cp-map-data-ldg-shift-3">
                          <span class="mfd-label mfd-spacing-right bigger" style="justify-self: flex-end">
                            THRESHOLD SHIFT
                          </span>
                          <InputField<number>
                            dataEntryFormat={new LengthFormat(Signal.of(0), Signal.of(4000))}
                            value={this.thresholdShift}
                            mandatory={Signal.of(false)}
                            hEventConsumer={this.hEventConsumer}
                            interactionMode={this.interactionMode}
                            errorHandler={() => {}}
                          />
                          <span class="mfd-label mfd-spacing-right bigger" style="justify-self: flex-end">
                            END SHIFT
                          </span>
                          <InputField<number>
                            dataEntryFormat={new LengthFormat(Signal.of(0), Signal.of(4000))}
                            value={this.endShift}
                            mandatory={Signal.of(false)}
                            hEventConsumer={this.hEventConsumer}
                            interactionMode={this.interactionMode}
                            errorHandler={() => {}}
                          />
                        </div>
                        <div class="oans-cp-map-data-ldg-shift-return-button">
                          <Button
                            label="RETURN"
                            buttonStyle="padding: 7px 15px 5px 15px;"
                            onClick={() => this.hideLdgShiftPanel()}
                          />
                        </div>
                      </div>
                    </div>
                  </div>
                  <div ref={this.mapDataMainRef} class="oans-cp-map-data-main">
                    <div class="oans-cp-map-data-main-2">
                      <Button
                        label={this.crossExistsForEntity.derive((e) => (e ? 'DEL CROSS' : 'ADD CROSS'))}
                        onClick={() => this.handleCrossButton()}
                        buttonStyle="flex: 1"
                        disabled={this.entityIsNotSelected}
                      />
                      <Button
                        label={this.flagExistsForEntity.derive((e) => (e ? 'DEL FLAG' : 'ADD FLAG'))}
                        onClick={() => this.handleFlagButton()}
                        buttonStyle="flex: 1; margin-left: 10px; margin-right: 10px"
                        disabled={this.entityIsNotSelected}
                      />
                      <Button
                        label="LDG SHIFT"
                        onClick={() => this.showLdgShiftPanel()}
                        buttonStyle="flex: 1"
                        disabled={Signal.of(true)}
                      />
                    </div>
                    <div class="oans-cp-map-data-main-center">
                      <Button
                        label={this.selectedEntityString.derive((s) => `CENTER MAP ON ${s}`)}
                        onClick={() => {
                          if (this.selectedEntityPosition) {
                            this.props.bus
                              .outbox<OansControlEvents>()
                              .post('oans_center_map_on', this.selectedEntityPosition, true);
                          }
                        }}
                        disabled={this.entityIsNotSelected}
                      />
                    </div>
                    <OansRunwayInfoBox
                      rwyOrStand={this.selectedEntityType}
                      selectedEntity={this.selectedEntityString}
                      tora={this.runwayToraText}
                      lda={this.runwayLdaText}
                      ldaIsReduced={Signal.of(false)}
                      coordinate={Signal.of('----')}
                      lengthUnit={this.lengthUnitText}
                    />
                  </div>
                  <div ref={this.mapDataBtvFallback} class="oans-cp-map-data-btv-fallback">
                    <div class="oans-cp-map-data-btv-button">
                      <div class="mfd-label" style="margin-right: 10px;">
                        BTV MANUAL CONTROL / FALLBACK
                      </div>
                    </div>
                    <div
                      class="oans-cp-map-data-btv-rwy-length"
                      style={{ visibility: this.fmsLandingRunwayVisibility }}
                    >
                      <div class="mfd-label" style="margin-right: 10px;">
                        RUNWAY LENGTH
                      </div>
                      <span class="mfd-value smaller">
                        {this.runwayLdaText}
                        <span style="color: rgb(33, 33, 255)">{this.lengthUnitText}</span>
                      </span>
                    </div>
                    <div
                      class="oans-cp-map-data-btv-rwy-length"
                      style={{ visibility: this.fmsLandingRunwayVisibility }}
                    >
                      <div class="mfd-label" style="margin-right: 10px;">
                        BTV STOP DISTANCE
                      </div>
                      <div>
                        <InputField<number, number, false>
                          dataEntryFormat={
                            new LengthFormat(Signal.of(0), Signal.of(4000), this.lengthUnit, 5)
                          }
                          dataHandlerDuringValidation={async (val) =>
                            this.props.bus.outbox<FmsOansData>().post('oansManualStoppingDistance', val, true)
                          }
                          readonlyValue={this.reqStoppingDistance}
                          mandatory={Signal.of(false)}
                          inactive={this.fmsLandingRunwayNotSelectedInFallback}
                          hEventConsumer={this.hEventConsumer}
                          interactionMode={this.interactionMode}
                          errorHandler={() => {}}
                        />
                      </div>
                    </div>
                    <div
                      class="oans-cp-map-data-btv-rwy-length"
                      style={{
                        visibility: this.fmsLandingRunwayVisibility.derive((it) =>
                          it === 'hidden' ? 'inherit' : 'hidden',
                        ),
                      }}
                    >
                      <div class="mfd-label amber" style="margin-right: 10px;">
                        SELECT LANDING RUNWAY IN FMS
                      </div>
                    </div>
                  </div>
                </div>
              </TopTabNavigatorPage>
              <TopTabNavigatorPage>
                <div class="oans-cp-arpt-sel">
                  <div class="oans-cp-arpt-sel-2">
                    <div style="display: flex;">
                      <DropdownMenu
                        ref={this.airportSearchAirportDropdownRef}
                        values={this.store.airportSearchData}
                        selectedIndex={this.store.airportSearchSelectedAirportIndex}
                        onModified={(newSelectedIndex) => {
                          this.handleSelectAirport(
                            this.store.sortedAirports.get(newSelectedIndex ?? 0).idarpt,
                            newSelectedIndex ?? undefined,
                          );
                        }}
                        freeTextAllowed={false}
                        numberOfDigitsForInputField={7}
                        alignLabels={this.store.airportSearchMode.derive((it) =>
                          it === ControlPanelAirportSearchMode.City ? 'flex-start' : 'center',
                        )}
                        idPrefix="oanc-search-airport"
                        hEventConsumer={this.hEventConsumer}
                        interactionMode={this.interactionMode}
                      />
                    </div>
                    <div class="oans-cp-arpt-sel-radio">
                      <RadioButtonGroup
                        values={['ICAO', 'IATA', 'CITY NAME']}
                        selectedIndex={this.store.airportSearchMode}
                        onModified={(newSelectedIndex) => {
                          switch (newSelectedIndex) {
                            case 0:
                              this.handleSelectAirportSearchMode(ControlPanelAirportSearchMode.Icao);
                              break;
                            case 1:
                              this.handleSelectAirportSearchMode(ControlPanelAirportSearchMode.Iata);
                              break;
                            default:
                              this.handleSelectAirportSearchMode(ControlPanelAirportSearchMode.City);
                              break;
                          }
                        }}
                        idPrefix="oanc-search"
                      />
                    </div>
                  </div>
                  <div id="ArptSelMiddle" class="oans-cp-arpt-sel-middle">
                    <div class="oans-cp-arpt-sel-middle-1">
                      <span class="mfd-value">
                        {this.store.selectedAirport.derive((it) => it?.name?.substring(0, 18).toUpperCase() ?? '')}
                      </span>
                      <span class="mfd-value">
                        {this.store.selectedAirport.derive((it) => {
                          if (!it) {
                            return '';
                          }

                          return `${it.idarpt}       ${it.iata}`;
                        })}
                      </span>
                      <span class="mfd-value">
                        {this.store.selectedAirport.derive((it) => {
                          if (!it) {
                            return '';
                          }

                          return `${ControlPanelUtils.LAT_FORMATTER(it.coordinates.lat)}/${ControlPanelUtils.LONG_FORMATTER(it.coordinates.lon)}`;
                        })}
                      </span>
                      <span class="mfd-label bigger" style={{ display: this.setPlanModeDisplay }}>
                        SET PLAN MODE
                      </span>
                    </div>
                    <div style="flex-grow: 1;" />
                    <div class="oans-cp-arpt-sel-display-arpt">
                      <Button
                        ref={this.displayAirportButtonRef}
                        label="DISPLAY AIRPORT"
                        onClick={() => this.handleDisplayAirport()}
                        buttonStyle="width: 100%"
                      />
                    </div>
                  </div>
                  <div class="oans-cp-arpt-sel-fms">
                    <Button
                      label={this.fmsDataStore.origin.derive((it) => it ?? 'ORIGIN')}
                      onClick={() => {
                        const airport = this.fmsDataStore.origin.get();
                        if (airport) {
                          this.handleSelectAirport(airport);
                        }
                      }}
                      disabled={this.fmsDataStore.origin.derive((it) => !it)}
                      buttonStyle="width: 100px;"
                    />
                    <Button
                      label={this.fmsDataStore.destination.derive((it) => it ?? 'DEST')}
                      onClick={() => {
                        const airport = this.fmsDataStore.destination.get();
                        if (airport) {
                          this.handleSelectAirport(airport);
                        }
                      }}
                      disabled={this.fmsDataStore.destination.derive((it) => !it)}
                      buttonStyle="width: 100px;"
                    />
                    <Button
                      label={this.fmsDataStore.alternate.derive((it) => it ?? 'ALTN')}
                      onClick={() => {
                        const airport = this.fmsDataStore.alternate.get();
                        if (airport) {
                          this.handleSelectAirport(airport);
                        }
                      }}
                      disabled={this.fmsDataStore.alternate.derive((it) => !it)}
                      buttonStyle="width: 100px;"
                    />
                  </div>
                </div>
              </TopTabNavigatorPage>
              <TopTabNavigatorPage containerStyle="justify-content: space-between; align-content: space-between; justify-items: space-between;">
                <div class="oans-cp-status">
                  <div class="oans-cp-status-active">
                    <span class="mfd-label" style="margin-bottom: 10px;">
                      ACTIVE
                    </span>
                    <span class="mfd-value bigger">{this.activeDatabase}</span>
                  </div>
                  <div class="oans-cp-status-2">
                    <Button
                      label="SWAP"
                      disabled={this.oansAvailable}
                      onClick={() => this.loadOansDb()}
                      buttonStyle="padding: 20px 30px 20px 30px;"
                    />
                  </div>
                  <div class="oans-cp-status-second">
                    <span class="mfd-label" style="margin-bottom: 10px;">
                      SECOND
                    </span>
                    <span class="mfd-value smaller">{this.activeDatabase}</span>
                  </div>
                </div>
                <div class="oans-cp-status-db">
                  <span class="mfd-label">AIRPORT DATABASE</span>
                  <span class="mfd-value">{this.airportDatabase}</span>
                </div>
                <div class="oans-cp-status-1">
                  <span class="mfd-label bigger" />
                </div>
              </TopTabNavigatorPage>
            </TopTabNavigator>
          </div>
        </div>
      </>
    );
  }
}

interface EraseSymbolsDialogProps extends ViewProps {
  visible: Readable<boolean>;
  confirmAction: () => void;
  hideDialog: () => void;
  /** True: Cross, false: flag */
  isCross: boolean;
}

/*
 * ERASE ALL xxx dialog
 */
export class EraseSymbolsDialog extends View<EraseSymbolsDialogProps> {
  // Make sure to collect all subscriptions here, otherwise page navigation doesn't work.
  private subs = [] as Bond[];

  private topRef = UI.ref<HTMLDivElement>();

  afterRender(node: ViewNode): void {
    super.afterRender(node);

    this.subs.push(
      this.props.visible.watch((val) => {
        if (this.topRef.peek()) {
          this.topRef.target.style.display = val ? 'block' : 'none';
        }
      }, true),
    );
  }

  public destroy(): void {
    // Destroy all subscriptions to remove all references to this instance.
    for (const x of this.subs) {
      x.destroy();
    }
    super.destroy();
  }

  render(): ViewNode {
    return (
      <div ref={this.topRef}>
        <div class="mfd-dialog" style="left: 209px; top: 350px; width: 350px;">
          <div class="mfd-dialog-title" style="margin-bottom: 20px;">
            <span class="mfd-label">{`ERASE ALL ${this.props.isCross ? 'CROSSES' : 'FLAGS'}`}</span>
            <span>
              <img
                style="position: relative; top: -5px; left: 10px"
                width="25px"
                src={`/Images/fbw-a380x/oans/oans-${this.props.isCross ? 'cross' : 'flag'}.svg`}
              />
            </span>
          </div>
          <div class="mfd-dialog-buttons">
            <Button label="CANCEL" onClick={() => this.props.hideDialog()} />
            <Button
              label="CONFIRM"
              onClick={() => {
                this.props.confirmAction();
                this.props.hideDialog();
              }}
              buttonStyle="padding-right: 6px;"
            />
          </div>
        </div>
      </div>
    );
  }
}

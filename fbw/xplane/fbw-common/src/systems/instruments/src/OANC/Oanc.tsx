// Copyright (c) 2023-2024 FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0

import {
  Arrays,
  ViewProps,
  BusSignal,
  Debouncer,
  View,
  Bus,
  UI,
  Derived,
  VarUnit,
  Signal,
  Readable,
  ListChange,
  UnitList,
  ViewNode,
  Waits,
} from 'xplane-js/ui';

import {
  AmdbFeature,
  AmdbFeatureCollection,
  AmdbFeatureTypeStrings,
  AmdbProjection,
  AmdbProperties,
  Arinc429LocalVarConsumerSubject,
  EfisNdMode,
  EfisSide,
  FcuSimVars,
  FeatureType,
  FeatureTypeString,
  FmsOansData,
  GenericAdirsEvents,
  MapParameters,
  MathUtils,
  OansControlEvents,
  OansFmsDataStore,
  OansMapProjection,
  PolygonalStructureType,
} from '@flybywiresim/fbw-sdk';

import {
  bbox,
  bboxPolygon,
  booleanPointInPolygon,
  centroid,
  featureCollection,
  polygon,
  simplify,
  union,
} from '@turf/turf';
import { Feature, FeatureCollection, Geometry, LineString, Point, Polygon, MultiPolygon, Position } from 'geojson';
import { bearingTo, clampAngle, Coordinates, distanceTo, placeBearingDistance } from 'msfs-geo';

import { reciprocal } from '@fmgc/guidance/lnav/CommonGeometry';
import { OansBrakeToVacateSelection } from './OansBrakeToVacateSelection';
import { LAYER_SPECIFICATIONS } from './style-data';
import { OancMovingModeOverlay, OancStaticModeOverlay } from './OancMovingModeOverlay';
import { OancAircraftIcon } from './OancAircraftIcon';
import { OancLabelManager } from './OancLabelManager';
import { OancPositionComputer } from './OancPositionComputer';
import { OancMarkerManager } from './OancMarkerManager';
import { ResetPanelSimvars } from './ResetPanelPublisher';
import { NavigraphAmdbClient } from './api/NavigraphAmdbClient';
import { pointAngle } from './OancMapUtils';
import { LubberLine } from '../ND/pages/arc/LubberLine';
import { Vars } from 'xplane-js/data';

export const OANC_RENDER_WIDTH = 768;
export const OANC_RENDER_HEIGHT = 768;

const FEATURE_DRAW_PER_FRAME = 50;

export const ZOOM_TRANSITION_TIME_MS = 300;

const PAN_MIN_MOVEMENT = 10;

const LABEL_FEATURE_TYPES = [
  FeatureType.TaxiwayElement,
  FeatureType.VerticalPolygonalStructure,
  FeatureType.PaintedCenterline,
  FeatureType.ParkingStandLocation,
  FeatureType.RunwayExitLine,
];

const LABEL_POLYGON_STRUCTURE_TYPES = [PolygonalStructureType.TerminalBuilding];

export type A320EfisZoomRangeValue = 0.2 | 0.5 | 1 | 2.5;

export type A380EfisZoomRangeValue = 0.2 | 0.5 | 1 | 2 | 5;

export const a320EfisZoomRangeSettings: A320EfisZoomRangeValue[] = [0.2, 0.5, 1, 2.5];

export const a380EfisZoomRangeSettings: A380EfisZoomRangeValue[] = [0.2, 0.5, 1, 2, 5];

const DEFAULT_SCALE_NM = 0.539957;

export const LABEL_VISIBILITY_RULES = [true, true, true, true, true];

export enum LabelStyle {
  Taxiway = 'taxiway',
  ExitLine = 'exit-line',
  TerminalBuilding = 'terminal-building',
  RunwayAxis = 'runway-axis',
  RunwayEnd = 'runway-end',
  FmsSelectedRunwayEnd = 'runway-end-fms-selected',
  FmsSelectedRunwayAxis = 'runway-axis-fms-selected',
  BtvSelectedRunwayEnd = 'runway-end-btv-selected',
  BtvSelectedRunwayArrow = 'runway-arrow-btv-selected',
  BtvSelectedExit = 'exit-line-btv-selected',
  BtvStopLineMagenta = 'btv-stop-line-magenta',
  BtvStopLineAmber = 'btv-stop-line-amber',
  BtvStopLineRed = 'btv-stop-line-red',
  BtvStopLineGreen = 'btv-stop-line-green',
  CrossSymbol = 'cross-symbol',
  FlagSymbol = 'flag-symbol',
}

export interface Label {
  text: string;
  style: LabelStyle;
  position: Position;
  rotation: number | undefined;
  associatedFeature?: AmdbFeature;
}

export interface ContextMenuItemData {
  name: string;

  disabled?: boolean | Readable<boolean>;

  onPressed?: () => void;
}

export interface OancProps<T extends number> extends ViewProps {
  bus: Bus;
  side: EfisSide;
  contextMenuVisible?: Signal<boolean>;
  contextMenuX?: Signal<number>;
  contextMenuY?: Signal<number>;
  contextMenuItems?: ContextMenuItemData[];
  zoomValues: T[];
}

export class Oanc<T extends number> extends View<OancProps<T>> {
  private readonly sub = this.props.bus.inbox<
    FcuSimVars & OansControlEvents & FmsOansData & GenericAdirsEvents
  >();

  private readonly animationContainerRef = [
    UI.ref<HTMLDivElement>(),
    UI.ref<HTMLDivElement>(),
  ];

  private readonly panContainerRef = [UI.ref<HTMLDivElement>(), UI.ref<HTMLDivElement>()];

  private readonly layerCanvasRefs = [
    UI.ref<HTMLCanvasElement>(),
    UI.ref<HTMLCanvasElement>(),
    UI.ref<HTMLCanvasElement>(),
    UI.ref<HTMLCanvasElement>(),
    UI.ref<HTMLCanvasElement>(),
  ];

  private readonly layerCanvasScaleContainerRefs = [
    UI.ref<HTMLCanvasElement>(),
    UI.ref<HTMLCanvasElement>(),
    UI.ref<HTMLCanvasElement>(),
    UI.ref<HTMLCanvasElement>(),
    UI.ref<HTMLCanvasElement>(),
  ];

  public labelContainerRef = UI.ref<HTMLDivElement>();

  public data: AmdbFeatureCollection | undefined;

  private arpCoordinates = Signal.of<Coordinates | null>(null);

  private canvasCenterCoordinates: Coordinates | null = null;

  private readonly dataAirportName = Signal.of('');

  private readonly dataAirportIcao = Signal.of('');

  private readonly dataAirportIata = Signal.of('');

  private readonly positionString = Signal.of('');

  private readonly positionVisible = Signal.of(false);

  private readonly airportInfoLine1 = this.dataAirportName.derive((it) => it.toUpperCase());

  private readonly airportInfoLine2 = Derived.from(
    ([icao, iata]) => `${icao}  ${iata}`,
    this.dataAirportIcao,
    this.dataAirportIata,
  );

  private readonly resetPulled = BusSignal.from(
    this.props.bus.inbox<ResetPanelSimvars>().topic('a380x_reset_panel_arpt_nav'),
    false,
  );

  private layerFeatures: FeatureCollection<Geometry, AmdbProperties>[] = [
    featureCollection([]), // Layer 0: TAXIWAY BG + TAXIWAY SHOULDER
    featureCollection([]), // Layer 1: APRON + STAND BG + BUILDINGS (terminal only)
    featureCollection([]), // Layer 2: RUNWAY (with markings)
    featureCollection([]), // Layer 3: TAXIWAY GUIDANCE LINES (unscaled width)
    featureCollection([]), // Layer 4: TAXIWAY GUIDANCE LINES (scaled width), HOLD SHORT LINES
  ];

  public readonly amdbClient = new NavigraphAmdbClient();

  private readonly labelManager = new OancLabelManager<T>(this);

  private readonly positionComputer = new OancPositionComputer<T>(this);

  public dataLoading = false;

  public doneDrawing = false;

  private isPanningArmed = false;

  public isPanning = false;

  private lastPanX = 0;

  private lastPanY = 0;

  public panArmedX = Signal.of(0);

  public panArmedY = Signal.of(0);

  public panOffsetX = Signal.of(0);

  public panOffsetY = Signal.of(0);

  public panBeingAnimated = Signal.of(false);

  // eslint-disable-next-line arrow-body-style
  private readonly isMapPanned = Derived.from(
    ([panX, panY, panBeingAnimated]) => {
      return panX !== 0 || panY !== 0 || panBeingAnimated;
    },
    this.panOffsetX,
    this.panOffsetY,
    this.panBeingAnimated,
  );

  public modeAnimationOffsetX = Signal.of(0);

  public modeAnimationOffsetY = Signal.of(0);

  private modeAnimationMapNorthUp = Signal.of(false);

  private canvasWidth = Signal.of(0);

  private canvasHeight = Signal.of(0);

  private canvasCentreX = Signal.of(0);

  private canvasCentreY = Signal.of(0);

  // TODO: Should be using GPS position interpolated with IRS velocity data
  private readonly pposLatWord = Arinc429LocalVarConsumerSubject.create(this.sub.topic('latitude'));
  private readonly pposLongWord = Arinc429LocalVarConsumerSubject.create(this.sub.topic('longitude'));
  private readonly trueHeadingWord = Arinc429LocalVarConsumerSubject.create(this.sub.topic('trueHeadingRaw'));

  public readonly ppos = Derived.from(
    ([latWord, longWord]) => ({ lat: latWord.value, long: longWord.value }) as Coordinates,
    this.pposLatWord,
    this.pposLongWord,
  );

  public referencePos: Coordinates = { lat: 0, long: 0 };

  public readonly aircraftWithinAirport = Signal.of(false);

  private readonly airportWithinRange = Signal.of(false);

  private readonly airportTooFarAwayAndInArcNavMode = Signal.of(false);

  private readonly airportBearing = Signal.of(0);

  public readonly projectedPpos = Derived.from<[Coordinates, Coordinates | null], Position>(
    ([ppos, arpCoordinates], previous?: Position | undefined) => {
      if (arpCoordinates) {
        return OansMapProjection.globalToAirportCoordinates(arpCoordinates, ppos, [0, 0]);
      }

      return previous ?? [0, 0];
    },
    this.ppos,
    this.arpCoordinates,
  );

  private readonly aircraftOnGround = Signal.of(true);

  private readonly mapHeading = Signal.of(0);

  public readonly interpolatedMapHeading = Signal.of(0);

  public readonly previousZoomLevelIndex: Signal<number> = Signal.of(this.props.zoomValues.length - 1);

  public readonly zoomLevelIndex: Signal<number> = Signal.of(this.props.zoomValues.length - 1);

  public readonly canvasCentreReferencedMapParams = new MapParameters();

  public readonly arpReferencedMapParams = new MapParameters();

  private readonly oansVisible = BusSignal.from<{ side: EfisSide; show: boolean }>(null, {
    side: this.props.side,
    show: false,
  });

  private readonly efisNDModeSub = BusSignal.from<EfisNdMode>(null, EfisNdMode.PLAN);

  private readonly efisOansRangeSub = BusSignal.from<number>(null, 4);

  private readonly overlayNDModeSub = Signal.of(EfisNdMode.PLAN);

  private readonly ndModeSwitchDelayDebouncer = new Debouncer();

  private readonly fmsDataStore = new OansFmsDataStore(this.props.bus);

  private readonly btvUtils = new OansBrakeToVacateSelection<T>(
    this.props.bus,
    this.labelManager,
    this.aircraftOnGround,
    this.projectedPpos,
    this.arpCoordinates,
    this.layerCanvasRefs[this.layerCanvasRefs.length - 1],
    this.canvasCentreX,
    this.canvasCentreY,
    this.zoomLevelIndex,
    this.getZoomLevelInverseScale.bind(this),
  );

  private readonly markerManager = new OancMarkerManager<T>(this, this.labelManager, this.props.bus);

  private readonly airportNotInActiveFpln = Derived.from(
    ([ndMode, arpt, origin, dest, altn]) => ndMode !== EfisNdMode.ARC && ![origin, dest, altn].includes(arpt),
    this.overlayNDModeSub,
    this.dataAirportIcao,
    this.fmsDataStore.origin,
    this.fmsDataStore.destination,
    this.fmsDataStore.alternate,
  );

  private readonly pposNotAvailable = Derived.from(
    ([lat, long, trueHeading]) =>
      !lat.isNormalOperation() || !long.isNormalOperation() || !trueHeading.isNormalOperation(),
    this.pposLatWord,
    this.pposLongWord,
    this.trueHeadingWord,
  );

  // eslint-disable-next-line arrow-body-style
  public usingPposAsReference = Derived.from(
    ([overlayNDMode, aircraftOnGround, aircraftWithinAirport]) => {
      return (aircraftOnGround && aircraftWithinAirport) || overlayNDMode === EfisNdMode.ARC;
    },
    this.overlayNDModeSub,
    this.aircraftOnGround,
    this.aircraftWithinAirport,
  );

  // eslint-disable-next-line arrow-body-style
  private readonly showAircraft = Derived.from(
    ([icao, pposRef]) => icao !== '' && pposRef,
    this.dataAirportIcao,
    this.usingPposAsReference,
  );

  private readonly aircraftX = Signal.of(0);

  private readonly aircraftY = Signal.of(0);

  private readonly aircraftRotation = Signal.of(0);

  private readonly zoomLevelScales: number[] = this.props.zoomValues.map((it) => 1 / ((it * 2) / DEFAULT_SCALE_NM));

  private readonly airportLoading = Signal.of(false);

  private readonly arptNavPosLostFlagVisible = Derived.from(
    ([pposNotAvailable, efisNDModeSub]) => pposNotAvailable && efisNDModeSub !== EfisNdMode.PLAN,
    this.pposNotAvailable,
    this.overlayNDModeSub,
  );

  private readonly pleaseWaitFlagVisible = Derived.from(
    ([arptNavPosLostFlagVisible, airportLoading]) => !arptNavPosLostFlagVisible && airportLoading,
    this.arptNavPosLostFlagVisible,
    this.airportLoading,
  );

  private readonly oansNotAvailable = BusSignal.from(null, false);

  private readonly anyFlagVisible = Derived.from(
    ([arptNavPosLostFlagVisible, pleaseWaitFlagVisible]) => arptNavPosLostFlagVisible || pleaseWaitFlagVisible,
    this.arptNavPosLostFlagVisible,
    this.pleaseWaitFlagVisible,
  );

  private readonly oansPerformanceModeHide = Signal.of(false);

  public getZoomLevelInverseScale() {
    const multiplier = this.overlayNDModeSub.get() === EfisNdMode.ROSE_NAV ? 0.5 : 1;

    return this.zoomLevelScales[this.zoomLevelIndex.get()] * multiplier;
  }

  afterRender(node: ViewNode) {
    super.afterRender(node);

    this.labelContainerRef.target.addEventListener('mousedown', this.handleCursorPanStart.bind(this));
    this.labelContainerRef.target.addEventListener('mousemove', this.handleCursorPanMove.bind(this));
    this.labelContainerRef.target.addEventListener('mouseup', this.handleCursorPanStop.bind(this));

    this.oansVisible.rebind(this.sub.topic('nd_show_oans'));
    this.oansNotAvailable.rebind(this.sub.topic('oans_not_avail'));
    this.efisNDModeSub.rebind(this.sub.topic('ndMode'));

    this.efisNDModeSub.watch((mode) => {
      this.handleNDModeChange(mode);
      this.handleLabelFilter();
    }, true);

    this.efisOansRangeSub.rebind(this.sub.topic('oansRange'));

    this.efisOansRangeSub.watch((range) => this.zoomLevelIndex.set(range), true);

    this.airportTooFarAwayAndInArcNavMode.watch((v) =>
      this.props.bus.outbox<OansControlEvents>().post('oans_show_set_plan_mode', v, true),
    );

    this.sub
      .topic('oans_display_airport')
      .distinct()
      .listen((airport) => {
        if (this.oansPerformanceModeHide.get()) {
          this.dataAirportIcao.set(airport);
        } else {
          this.loadAirportMap(airport);
        }
      });

    this.sub
      .topic('oans_performance_mode_hide')
      .distinct()
      .listen((perfHide) => {
        if (this.props.side === perfHide.side) {
          this.oansPerformanceModeHide.set(perfHide.hide);
        }
      });

    this.oansPerformanceModeHide.watch((hide) => {
      if (hide) {
        this.unloadAirportMap(true);
      } else if (this.dataAirportIcao.get()) {
        this.loadAirportMap(this.dataAirportIcao.get(), true);
      }
    });

    this.sub.topic('oans_center_on_acft').listen(() => this.centerOnAcft());
    this.sub.topic('oans_center_map_on').listen((coords) => this.centerMapOn(coords));
    this.sub.topic('oans_add_cross_at_feature').listen((f) => this.markerManager.addCrossAtFeature(f.id, f.feattype));
    this.sub.topic('oans_add_flag_at_feature').listen((f) => this.markerManager.addFlagAtFeature(f.id, f.feattype));
    this.sub
      .topic('oans_remove_cross_at_feature')
      .listen((f) => this.markerManager.removeCrossAtFeature(f.id, f.feattype));
    this.sub.topic('oans_remove_flag_at_feature').listen((f) => this.markerManager.removeFlagAtFeature(f.id, f.feattype));
    this.sub
      .topic('oans_add_cross_at_cursor')
      .listen(([x, y]) => this.markerManager.addCross(this.unprojectPoint([x, y])));
    this.sub.topic('oans_add_flag_at_cursor').listen(([x, y]) => this.markerManager.addFlag(this.unprojectPoint([x, y])));
    this.sub.topic('oans_erase_all_crosses').listen(() => this.markerManager.eraseAllCrosses());
    this.sub.topic('oans_erase_all_flags').listen(() => this.markerManager.eraseAllFlags());
    this.sub.topic('oans_erase_cross_id').listen((id) => this.markerManager.removeCross(id));
    this.sub.topic('oans_erase_flag_id').listen((id) => this.markerManager.removeFlag(id));
    this.sub.topic('oans_query_symbols_at_cursor').listen((data) => {
      const foundSymbols = this.markerManager.findSymbolAtCursor(this.unprojectPoint(data.cursorPosition));
      this.props.bus.outbox<OansControlEvents>().post('oans_answer_symbols_at_cursor', {
        side: data.side,
        cross: foundSymbols.cross,
        flag: foundSymbols.flag,
      });
    });
    this.sub
      .topic('oans_remove_btv_data')
      .distinct()
      .listen((removeBtvData) => {
        if (removeBtvData) {
          this.btvUtils.clearSelection();
        }
      });

    this.fmsDataStore.origin.watch(() => this.updateLabelClasses());
    this.fmsDataStore.departureRunway.watch(() => this.updateLabelClasses());
    this.fmsDataStore.destination.watch(() => this.updateLabelClasses());
    this.fmsDataStore.landingRunway.watch(() => this.updateLabelClasses());
    this.btvUtils.btvRunway.watch(() => this.updateLabelClasses());
    this.btvUtils.btvExit.watch(() => {
      this.updateLabelClasses();
    });

    this.labelManager.visibleLabels.watch((_index, type, item) => {
      switch (type) {
        case ListChange.Added: {
          if (Array.isArray(item)) {
            for (const label of item as Label[]) {
              const element = this.createLabelElement(label);

              this.labelContainerRef.target.appendChild(element);
              this.labelManager.visibleLabelElements.set(label, element);
            }
          } else {
            const element = this.createLabelElement(item as Label);

            this.labelContainerRef.target.appendChild(element);
            this.labelManager.visibleLabelElements.set(item as Label, element);
          }
          break;
        }
        case ListChange.Removed: {
          if (Array.isArray(item)) {
            for (const label of item as Label[]) {
              const element = this.labelManager.visibleLabelElements.get(label);
              if (element) {
                this.labelContainerRef.target.removeChild(element);
                this.labelManager.visibleLabelElements.delete(label);
              }
            }
          } else {
            const element = this.labelManager.visibleLabelElements.get(item as Label);
            if (element) {
              this.labelContainerRef.target.removeChild(element);
              this.labelManager.visibleLabelElements.delete(item as Label);
            }
          }
          break;
        }
        default:
          break;
      }
    });

    this.zoomLevelIndex.watch(() => this.handleLabelFilter(), true);

    Derived.from(this.panOffsetX, this.panOffsetY).watch(([x, y]) => {
      this.panContainerRef[0].target.style.transform = `translate(${x}px, ${y}px)`;
      this.panContainerRef[1].target.style.transform = `translate(${x}px, ${y}px)`;

      const depRwy = this.fmsDataStore.departureRunway.get();
      const ldgRwy = this.fmsDataStore.landingRunway.get();
      const btvRwy = this.btvUtils.btvRunway.get();
      const btvExit = this.btvUtils.btvExit.get();

      this.labelManager.reflowLabels(
        depRwy !== null ? depRwy : undefined,
        ldgRwy !== null ? ldgRwy : undefined,
        btvRwy !== null ? btvRwy : undefined,
        btvExit !== null ? btvExit : undefined,
      );
    });

    Derived.from(
      ([x, y]) => {
        this.animationContainerRef[0].target.style.transform = `translate(${x}px, ${y}px)`;
        this.animationContainerRef[1].target.style.transform = `translate(${x}px, ${y}px)`;
      },
      this.modeAnimationOffsetX,
      this.modeAnimationOffsetY,
    );

    this.pposNotAvailable.watch((notAvailable) => {
      Vars.write('L:A32NX_ARPT_NAV_POS_LOST', VarUnit.Bool, notAvailable);
    }, true);
  }

  private handleLabelFilter() {
    this.labelManager.showLabels = false;

    if (this.efisNDModeSub.get() === EfisNdMode.ARC) {
      switch (this.zoomLevelIndex.get()) {
        case 4:
        case 3:
          this.labelManager.currentFilter = { type: 'none' };
          break;
        case 2:
          this.labelManager.currentFilter = { type: 'major' };
          break;
        default:
          this.labelManager.currentFilter = { type: 'null' };
          break;
      }
    } else {
      switch (this.zoomLevelIndex.get()) {
        case 0:
          this.labelManager.currentFilter = { type: 'runwayBtvSelection', runwayIdent: null, showAdjacent: true };
          break;
        default:
          this.labelManager.currentFilter = { type: 'runwayBtvSelection', runwayIdent: null, showAdjacent: false };
          break;
      }
    }

    this.handleLayerVisibilities();

    setTimeout(() => (this.labelManager.showLabels = true), ZOOM_TRANSITION_TIME_MS + 200);
  }

  public unloadAirportMap(performanceModeUnload: boolean = false) {
    if (!performanceModeUnload) {
      this.btvUtils.clearSelection();
    }
    this.markerManager.eraseAllCrosses();
    this.markerManager.eraseAllFlags();
    this.clearMap();
    this.clearData();

    this.arpCoordinates.set(null);
    this.data = undefined;
    this.aircraftWithinAirport.set(false);

    this.btvUtils.transmitRwyAheadAdvisory(false, '', true);
  }

  /**
   *
   * @param icao four letter ICAO code of airport to load
   * @returns
   */
  public async loadAirportMap(icao: string, performanceModeUnload: boolean = false) {
    this.dataLoading = true;
    this.airportLoading.set(true);

    this.unloadAirportMap(performanceModeUnload);

    if (!icao) {
      this.dataLoading = false;
      this.airportLoading.set(false);

      this.dataAirportName.set('');
      this.dataAirportIcao.set('');
      this.dataAirportIata.set('');

      return;
    }

    const includeFeatureTypes: FeatureType[] = Object.values(LAYER_SPECIFICATIONS).reduce(
      (acc, it) => [
        ...acc,
        ...it.styleRules.reduce(
          (acc, it) => [
            ...acc,
            ...(it?.dontFetchFromAmdb || it.forFeatureTypes === undefined ? [] : it.forFeatureTypes),
          ],
          [] as FeatureType[],
        ),
      ],
      [] as FeatureType[],
    );
    const includeLayers = includeFeatureTypes.map((it) => AmdbFeatureTypeStrings[it]);

    // Additional stuff we need that isn't handled by the canvas renderer
    includeLayers.push(FeatureTypeString.AerodromeReferencePoint);
    includeLayers.push(FeatureTypeString.ParkingStandLocation);
    includeLayers.push(FeatureTypeString.PaintedCenterline);
    includeLayers.push(FeatureTypeString.RunwayThreshold);

    const data = await this.amdbClient.getAirportData(icao, includeLayers, undefined);
    const wgs84ArpDat = await this.amdbClient.getAirportData(
      icao,
      [FeatureTypeString.AerodromeReferencePoint],
      undefined,
      AmdbProjection.Epsg4326,
    );

    const features = Object.values(data).reduce((acc, it) => {
      const features = it.features.map((f) => {
        // FeatureCollection
        if (f.properties.idthr || f.properties.idrwy) {
          const nf = Object.assign({}, f);
          if (nf.properties.idthr && nf.properties.idthr.replace(/[^0-9]/g, '').length < 2) {
            nf.properties.idthr = `0${nf.properties.idthr}`;
          }

          if (nf.properties.idrwy) {
            nf.properties.idrwy = nf.properties.idrwy
              .split('.')
              .map((qfu) => (qfu.replace(/[^0-9]/g, '').length < 2 ? `0${qfu}` : qfu))
              .join('.');
          }

          return nf;
        }
        return f;
      });

      return [...acc, ...features];
    }, [] as AmdbFeature[]);
    const airportMap: AmdbFeatureCollection = featureCollection(features);

    const wgs84ReferencePoint = wgs84ArpDat.aerodromereferencepoint?.features[0];

    if (!wgs84ReferencePoint) {
      console.error('[OANC](loadAirportMap) Invalid airport data - aerodrome reference point not found');
      return;
    }

    const refPointLat = (wgs84ReferencePoint.geometry as Point).coordinates[1];
    const refPointLong = (wgs84ReferencePoint.geometry as Point).coordinates[0];
    const projectionScale = 1000;

    if (!refPointLat || !refPointLong || !projectionScale) {
      console.error(
        '[OANC](loadAirportMap) Invalid airport data - aerodrome reference point does not contain lat/long/scale custom properties',
      );
      return;
    }
    this.arpCoordinates.set({ lat: refPointLat, long: refPointLong });

    this.data = airportMap;

    if (wgs84ReferencePoint) {
      this.dataAirportName.set(wgs84ReferencePoint?.properties?.name ?? '');
      this.dataAirportIcao.set(icao);
      this.dataAirportIata.set(wgs84ReferencePoint?.properties?.iata ?? '');
    }

    // Figure out the boundaries of the map data
    const dataBbox = bbox(airportMap);

    if (!this.pposNotAvailable.get()) {
      this.aircraftWithinAirport.set(booleanPointInPolygon(this.projectedPpos.get(), bboxPolygon(dataBbox)));
    } else {
      this.aircraftWithinAirport.set(false);
    }

    const width = (dataBbox[2] - dataBbox[0]) * 1;
    const height = (dataBbox[3] - dataBbox[1]) * 1;

    this.canvasWidth.set(width);
    this.canvasHeight.set(height);
    this.canvasCentreX.set(Math.abs(dataBbox[0]));
    this.canvasCentreY.set(Math.abs(dataBbox[3]));

    this.canvasCenterCoordinates = this.calculateCanvasCenterCoordinates();

    this.sortDataIntoLayers(this.data);
    this.generateAllLabels(this.data);

    this.dataLoading = false;
  }

  private calculateCanvasCenterCoordinates() {
    const arpCoordinates = this.arpCoordinates.get();

    if (arpCoordinates) {
      const pxDistanceToCanvasCentre = MathUtils.pointDistance(
        this.canvasWidth.get() / 2,
        this.canvasHeight.get() / 2,
        this.canvasCentreX.get(),
        this.canvasCentreY.get(),
      );
      const nmDistanceToCanvasCentre = UnitList.NMILE.from(pxDistanceToCanvasCentre / 1_000, UnitList.KILOMETER);
      const angleToCanvasCentre = clampAngle(
        pointAngle(
          this.canvasWidth.get() / 2,
          this.canvasHeight.get() / 2,
          this.canvasCentreX.get(),
          this.canvasCentreY.get(),
        ) + 90,
      );

      return placeBearingDistance(arpCoordinates, reciprocal(angleToCanvasCentre), nmDistanceToCanvasCentre);
    }
    return null;
  }

  private createLabelElement(label: Label): HTMLDivElement {
    const element = document.createElement('div');

    element.classList.add('oanc-label');
    element.classList.add(`oanc-label-style-${label.style}`);
    element.textContent = label.text;

    if (label.style === LabelStyle.RunwayEnd) {
      element.addEventListener('click', () => {
        const thresholdFeature = this.data?.features.filter(
          (it) => it.properties.feattype === FeatureType.RunwayThreshold && it.properties?.idthr === label.text,
        );
        if (thresholdFeature && label.associatedFeature) {
          this.btvUtils.selectRunwayFromOans(
            `${this.dataAirportIcao.get()}${label.text}`,
            label.associatedFeature,
            thresholdFeature[0],
          );
        }
      });
    }
    if (
      label.style === LabelStyle.ExitLine &&
      label.associatedFeature?.properties.feattype === FeatureType.RunwayExitLine
    ) {
      element.addEventListener('click', () => {
        if (label.associatedFeature) {
          this.btvUtils.selectExitFromOans(label.text, label.associatedFeature);
        }
      });
    }

    return element;
  }

  private sortDataIntoLayers(data: AmdbFeatureCollection) {
    for (let i = 0; i < this.layerFeatures.length; i++) {
      const layer = this.layerFeatures[i];
      const layerSpec = LAYER_SPECIFICATIONS[i];

      if (layerSpec.styleRules.length === 0) {
        continue;
      }

      const featuresByStyleRule: AmdbFeature[][] = [];

      for (const styleRule of layerSpec.styleRules) {
        const matchingFeatures = data.features.filter(
          (it) =>
            (styleRule.forFeatureTypes === undefined || styleRule.forFeatureTypes.includes(it.properties.feattype)) &&
            (it.properties.plysttyp === undefined ||
              it.properties.feattype !== FeatureType.VerticalPolygonalStructure ||
              styleRule.forPolygonStructureTypes === undefined ||
              styleRule.forPolygonStructureTypes.includes(it.properties.plysttyp)),
        );

        unionFeatures: if (styleRule.unionBy !== undefined) {
          const featuresAreAllPolygons = matchingFeatures.every(
            (it) => it.geometry.type === 'Polygon' || it.geometry.type === 'MultiPolygon',
          );

          if (!featuresAreAllPolygons) {
            console.warn(
              '[Oanc](sortDataIntoLayers) Style rule had unionBy declared but not all filtered features were (multi)polygons. Not unioning them.',
            );
            break unionFeatures;
          }

          const finalFeatures: AmdbFeature<Polygon | MultiPolygon>[] = [];
          const groups = new Map<AmdbProperties[keyof AmdbProperties], AmdbFeature<Polygon | MultiPolygon>[]>();

          for (const feature of matchingFeatures) {
            const key = styleRule.unionBy !== 'all' ? feature.properties[styleRule.unionBy] : 'all';
            let existingGroup = groups.get(key);

            if (existingGroup === undefined) {
              existingGroup = [];
              groups.set(key, existingGroup);
            }

            if (styleRule.simplify !== undefined) {
              existingGroup.push(
                simplify(feature, { tolerance: styleRule.simplify }) as AmdbFeature<Polygon | MultiPolygon>,
              );
            } else {
              existingGroup.push(feature as AmdbFeature<Polygon | MultiPolygon>);
            }
          }

          const unionedGroups = Array.from(groups.values()).map((it) =>
            it.length > 1 ? union(featureCollection(it), { properties: it[0].properties }) : it[0],
          );

          for (const unionedGroup of unionedGroups) {
            if (unionedGroup === null) {
              continue;
            }

            switch (unionedGroup.geometry.type) {
              case 'Polygon':
                finalFeatures.push(unionedGroup);
                break;
              case 'MultiPolygon':
                for (const polygonCoordinates of unionedGroup.geometry.coordinates) {
                  const polygonFeature = polygon(polygonCoordinates, unionedGroup.properties);

                  finalFeatures.push(polygonFeature);
                }
            }
          }

          featuresByStyleRule.push(finalFeatures);
        } else {
          featuresByStyleRule.push(matchingFeatures);
        }
      }

      layer.features.push(...Arrays.flat(featuresByStyleRule));
    }
  }

  private generateAllLabels(data: AmdbFeatureCollection) {
    for (const feature of data.features) {
      if (!LABEL_FEATURE_TYPES.includes(feature.properties.feattype)) {
        continue;
      }

      // Only include "Taxiway" features that have a valid "idlin" property
      if (feature.properties.feattype === FeatureType.TaxiwayElement && !feature.properties.idlin) {
        continue;
      }

      // Only include "VerticalPolygonObject" features whose "plysttyp" property has what we want
      if (
        feature.properties.feattype === FeatureType.VerticalPolygonalStructure &&
        feature.properties.plysttyp &&
        !LABEL_POLYGON_STRUCTURE_TYPES.includes(feature.properties.plysttyp)
      ) {
        continue;
      }

      let labelPosition: Position = [0, 0];
      switch (feature.geometry.type) {
        case 'Point': {
          const point = feature.geometry as Point;

          labelPosition = point.coordinates;
          break;
        }
        case 'Polygon': {
          const polygon = feature.geometry as Polygon;

          labelPosition = centroid(polygon).geometry.coordinates;
          break;
        }
        case 'LineString': {
          const lineString = feature.geometry as LineString;

          labelPosition = centroid(lineString).geometry.coordinates;
          break;
        }
        default: {
          console.error(`[OANC] Cannot determine label position for geometry of type '${feature.geometry.type}'`);
        }
      }

      if (feature.properties.feattype === FeatureType.PaintedCenterline) {
        const designators: string[] = [];
        if (feature.properties.idrwy) {
          designators.push(...feature.properties.idrwy.split('.'));
        }

        if (designators.length === 0) {
          console.error(`Runway feature (id=${feature.properties.id}) does not have a valid idrwy value`);
          continue;
        }

        const runwayLine = feature.geometry as LineString;
        const runwayLineStart = runwayLine.coordinates[0];
        const runwayLineEnd = runwayLine.coordinates[runwayLine.coordinates.length - 1];
        const runwayLineBearing = clampAngle(
          -Math.atan2(runwayLineStart[1] - runwayLineEnd[1], runwayLineStart[0] - runwayLineEnd[0]) *
            MathUtils.RADIANS_TO_DEGREES +
            90,
        );

        // If reciprocal bearing doesn't match to rwy designator[0], swap designators
        if (
          Math.abs(Number(designators[0].replace(/\D/g, '')) * 10 - reciprocal(runwayLineBearing)) > 90 &&
          designators.length === 2
        ) {
          const copied = Array.from(designators);
          designators.length = 0;
          designators.push(copied[1], copied[0]);
        }

        const isFmsOrigin = this.dataAirportIcao.get() === this.fmsDataStore.origin.get();
        const isFmsDestination = this.dataAirportIcao.get() === this.fmsDataStore.origin.get();
        const depRwy = this.fmsDataStore.departureRunway.get()?.substring(4);
        const ldgRwy = this.fmsDataStore.landingRunway.get()?.substring(4);
        const isSelectedRunway =
          (isFmsOrigin && depRwy && designators.includes(depRwy)) ||
          (isFmsDestination && ldgRwy && designators.includes(ldgRwy));

        const label1: Label = {
          text: designators[0],
          style:
            this.btvUtils.btvRunway.get() === designators[0] ? LabelStyle.BtvSelectedRunwayEnd : LabelStyle.RunwayEnd,
          position: runwayLineStart,
          rotation: reciprocal(runwayLineBearing),
          associatedFeature: feature,
        };
        this.labelManager.visibleLabels.insert(label1);
        this.labelManager.labels.push(label1);

        // Sometimes, runways have only one designator (e.g. EDDF 18R)
        if (designators[1]) {
          const label2: Label = {
            text: designators[1],
            style:
              this.btvUtils.btvRunway.get() === designators[1] ? LabelStyle.BtvSelectedRunwayEnd : LabelStyle.RunwayEnd,
            position: runwayLineEnd,
            rotation: runwayLineBearing,
            associatedFeature: feature,
          };
          this.labelManager.visibleLabels.insert(label2);
          this.labelManager.labels.push(label2);

          const label3: Label = {
            text: `${designators[0]}-${designators[1]}`,
            style: isSelectedRunway ? LabelStyle.FmsSelectedRunwayAxis : LabelStyle.RunwayAxis,
            position: runwayLineEnd,
            rotation: runwayLineBearing,
            associatedFeature: feature,
          };
          this.labelManager.visibleLabels.insert(label3);
          this.labelManager.labels.push(label3);
        } else {
          const label3: Label = {
            text: designators[0],
            style: isSelectedRunway ? LabelStyle.FmsSelectedRunwayAxis : LabelStyle.RunwayAxis,
            position: runwayLineEnd,
            rotation: runwayLineBearing,
            associatedFeature: feature,
          };
          this.labelManager.visibleLabels.insert(label3);
          this.labelManager.labels.push(label3);
        }

        // Selected FMS runway (origin or destination)
        const labelFms1: Label = {
          text: designators[0],
          style: LabelStyle.FmsSelectedRunwayEnd,
          position: runwayLineStart,
          rotation: reciprocal(runwayLineBearing),
          associatedFeature: feature,
        };
        this.labelManager.labels.push(labelFms1);
        this.labelManager.visibleLabels.insert(labelFms1);

        const labelFms2: Label = {
          text: designators[1],
          style: LabelStyle.FmsSelectedRunwayEnd,
          position: runwayLineEnd,
          rotation: runwayLineBearing,
          associatedFeature: feature,
        };
        this.labelManager.labels.push(labelFms2);
        this.labelManager.visibleLabels.insert(labelFms2);

        // BTV selected runway
        const btvSelectedArrow1: Label = {
          text: designators[0],
          style: LabelStyle.BtvSelectedRunwayArrow,
          position: runwayLineStart,
          rotation: reciprocal(runwayLineBearing),
          associatedFeature: feature,
        };
        this.labelManager.labels.push(btvSelectedArrow1);
        this.labelManager.visibleLabels.insert(btvSelectedArrow1);

        const btvSelectedArrow2: Label = {
          text: designators[1],
          style: LabelStyle.BtvSelectedRunwayArrow,
          position: runwayLineEnd,
          rotation: runwayLineBearing,
          associatedFeature: feature,
        };
        this.labelManager.labels.push(btvSelectedArrow2);
        this.labelManager.visibleLabels.insert(btvSelectedArrow2);
      } else {
        const text = feature.properties.idlin ?? feature.properties.idstd ?? feature.properties.ident ?? undefined;

        if (
          feature.properties.feattype === FeatureType.ParkingStandLocation &&
          text !== undefined &&
          text.includes('_')
        ) {
          continue;
        }

        if (text !== undefined) {
          let style: LabelStyle.TerminalBuilding | LabelStyle.Taxiway | LabelStyle.ExitLine;
          switch (feature.properties.feattype) {
            case FeatureType.VerticalPolygonalStructure:
              style = LabelStyle.TerminalBuilding;
              break;
            case FeatureType.ParkingStandLocation:
              style = LabelStyle.TerminalBuilding;
              break;
            case FeatureType.RunwayExitLine:
              style = LabelStyle.ExitLine;
              break;
            default:
              style = LabelStyle.Taxiway;
              break;
          }

          const existing = this.labelManager.labels.filter((it) => it.text === text);

          const shortestDistance = existing.reduce((shortestDistance, label) => {
            const distance = MathUtils.pointDistance(
              label.position[0],
              label.position[1],
              labelPosition[0],
              labelPosition[1],
            );

            return distance > shortestDistance ? distance : shortestDistance;
          }, Number.MAX_SAFE_INTEGER);

          if (
            (feature.properties.feattype === FeatureType.ParkingStandLocation &&
              existing.some((it) => feature.properties.termref === it.associatedFeature?.properties.termref)) ||
            shortestDistance < 50
          ) {
            continue;
          }

          const label = {
            text: text.toUpperCase(),
            style,
            position: labelPosition,
            rotation: undefined,
            associatedFeature: feature,
          };

          this.labelManager.labels.push(label);
          this.labelManager.visibleLabels.insert(label);
        }
      }
    }
  }

  private lastLayerDrawnIndex = 0;

  private lastFeatureDrawnIndex = 0;

  private lastTime = 0;

  public Update() {
    const now = Date.now();
    const deltaTime = (now - this.lastTime) / 1_000;
    this.lastTime = now;

    if (this.data && this.resetPulled.get()) {
      this.unloadAirportMap(true);
    }

    if (!this.data || this.dataLoading || this.resetPulled.get()) {
      return;
    }

    this.aircraftOnGround.set(
      // FIXME use an enum...
      ![6, 7, 8, 9].includes(Vars.read('L:A32NX_FWC_FLIGHT_PHASE', VarUnit.Number)),
    );

    const arpCoordinates = this.arpCoordinates.get();
    if (!this.pposNotAvailable.get() && arpCoordinates) {
      this.aircraftWithinAirport.set(booleanPointInPolygon(this.projectedPpos.get(), bboxPolygon(bbox(this.data))));

      const distToArpt = this.arpCoordinates.get() ? distanceTo(this.ppos.get(), arpCoordinates) : 9999;

      // If in ARC mode and airport more than 30nm away, apply a hack to not create a huge canvas (only shift airport a little bit out of view with a static offset)
      this.airportTooFarAwayAndInArcNavMode.set(
        !this.pposNotAvailable.get() && this.usingPposAsReference.get() && distToArpt > 30,
      );

      if (this.arpCoordinates.get()) {
        this.airportWithinRange.set(distToArpt < this.props.zoomValues[this.zoomLevelIndex.get()] + 3); // Add 3nm for airport dimension, FIXME better estimation
        this.airportBearing.set(bearingTo(this.ppos.get(), arpCoordinates));
      } else {
        this.airportWithinRange.set(true);
        this.airportBearing.set(0);
      }
    } else {
      this.airportTooFarAwayAndInArcNavMode.set(false);
      this.aircraftWithinAirport.set(false);
      this.airportWithinRange.set(true);
    }

    if (this.usingPposAsReference.get() || !arpCoordinates) {
      this.referencePos = this.ppos.get();
    } else {
      this.referencePos = arpCoordinates;
    }

    if (!this.pposNotAvailable.get() && arpCoordinates) {
      const position = this.positionComputer.computePosition();

      if (position) {
        this.positionVisible.set(true);
        this.positionString.set(position);
      } else {
        this.positionVisible.set(false);
      }

      this.btvUtils.updateRwyAheadAdvisory(
        this.ppos.get(),
        arpCoordinates,
        this.trueHeadingWord.get().value,
        this.layerFeatures[2],
      );
    } else {
      this.positionVisible.set(false);
    }

    // If OANS is not visible on this side (i.e. range selector is not on ZOOM), don't continue here to save runtime
    if (this.oansVisible.get().side === this.props.side && !this.oansVisible.get().show) {
      return;
    }

    const mapTargetHeading = this.modeAnimationMapNorthUp.get() ? 0 : this.trueHeadingWord.get().value;
    this.mapHeading.set(mapTargetHeading);

    const interpolatedMapHeading = this.interpolatedMapHeading.get();

    if (Math.abs(mapTargetHeading - interpolatedMapHeading) > 0.1) {
      const rotateLeft = MathUtils.diffAngle(interpolatedMapHeading, mapTargetHeading) < 0;

      if (rotateLeft) {
        this.interpolatedMapHeading.set(clampAngle(interpolatedMapHeading - deltaTime * 90));

        if (MathUtils.diffAngle(this.interpolatedMapHeading.get(), mapTargetHeading) > 0) {
          this.interpolatedMapHeading.set(mapTargetHeading);
        }
      } else {
        this.interpolatedMapHeading.set(clampAngle(interpolatedMapHeading + deltaTime * 90));

        if (MathUtils.diffAngle(this.interpolatedMapHeading.get(), mapTargetHeading) < 0) {
          this.interpolatedMapHeading.set(mapTargetHeading);
        }
      }
    }

    const mapCurrentHeading = this.interpolatedMapHeading.get();

    if (this.canvasCenterCoordinates) {
      this.canvasCentreReferencedMapParams.compute(this.canvasCenterCoordinates, 0, 0.539957, 1_000, mapCurrentHeading);
    }
    if (arpCoordinates) {
      this.arpReferencedMapParams.compute(arpCoordinates, 0, 0.539957, 1_000, mapCurrentHeading);
    }

    let [offsetX, offsetY]: [number, number] = [0, 0];
    if (this.airportTooFarAwayAndInArcNavMode.get()) {
      const shiftBy = 5 * Math.max(this.canvasWidth.get(), this.canvasHeight.get());
      [offsetX, offsetY] = [shiftBy, shiftBy];
    } else {
      [offsetX, offsetY] = this.canvasCentreReferencedMapParams.coordinatesToXYy(this.referencePos);
    }

    // TODO figure out how to not need this
    offsetY *= -1;

    const rotate = -mapCurrentHeading;

    // Transform layers
    for (let i = 0; i < this.layerCanvasRefs.length; i++) {
      const layerSpec = LAYER_SPECIFICATIONS[i];

      const canvas = this.layerCanvasRefs[i].target;
      const canvasScaleContainer = this.layerCanvasScaleContainerRefs[i].target;

      const scale = this.getZoomLevelInverseScale();

      const translateX = -((this.canvasWidth.get() * layerSpec.renderScale) / 2) + OANC_RENDER_WIDTH / 2;
      const translateY = -((this.canvasHeight.get() * layerSpec.renderScale) / 2) + OANC_RENDER_HEIGHT / 2;

      canvas.style.transform = `translate(${-offsetX}px, ${offsetY}px) scale(${1 / layerSpec.renderScale}) rotate(${rotate}deg)`;

      canvasScaleContainer.style.left = `${translateX}px`;
      canvasScaleContainer.style.top = `${translateY}px`;
      canvasScaleContainer.style.transform = `scale(${scale})`;

      const context = canvas.getContext('2d');
      if (context) {
        context.resetTransform();
        context.translate(
          this.canvasCentreX.get() * layerSpec.renderScale,
          this.canvasCentreY.get() * layerSpec.renderScale,
        );
        context.scale(layerSpec.renderScale, layerSpec.renderScale);
      }
    }

    // Transform airplane
    this.aircraftX.set(384);
    this.aircraftY.set(384);
    this.aircraftRotation.set(this.trueHeadingWord.get().value - mapCurrentHeading);

    // FIXME Use this to update pan offset when zooming
    /* if (this.previousZoomLevelIndex.get() !== this.zoomLevelIndex.get()) {
            // In PLAN mode, re-pan to zoom in to center of screen
            this.panOffsetX.set(this.panOffsetX.get() / this.zoomLevelScales[this.previousZoomLevelIndex.get()] * this.zoomLevelScales[this.zoomLevelIndex.get()]);
            this.panOffsetY.set(this.panOffsetY.get() / this.zoomLevelScales[this.previousZoomLevelIndex.get()] * this.zoomLevelScales[this.zoomLevelIndex.get()]);

            this.previousZoomLevelIndex.set(this.zoomLevelIndex.get());
        } */

    // Reflow labels if necessary
    if (this.lastLayerDrawnIndex > this.layerCanvasRefs.length - 1) {
      this.doneDrawing = true;

      this.airportLoading.set(false);

      const depRwy = this.fmsDataStore.departureRunway.get();
      const ldgRwy = this.fmsDataStore.landingRunway.get();
      const btvRwy = this.btvUtils.btvRunway.get();
      const btvExit = this.btvUtils.btvExit.get();

      this.labelManager.reflowLabels(
        depRwy !== null ? depRwy : undefined,
        ldgRwy !== null ? ldgRwy : undefined,
        btvRwy !== null ? btvRwy : undefined,
        btvExit !== null ? btvExit : undefined,
      );
      return;
    }

    const layerFeatures = this.layerFeatures[this.lastLayerDrawnIndex];
    const layerCanvas = this.layerCanvasRefs[this.lastLayerDrawnIndex].target.getContext('2d');

    if (this.lastFeatureDrawnIndex < layerFeatures.features.length && layerCanvas) {
      renderFeaturesToCanvas(
        this.lastLayerDrawnIndex,
        layerCanvas,
        layerFeatures,
        this.lastFeatureDrawnIndex,
        this.lastFeatureDrawnIndex + FEATURE_DRAW_PER_FRAME,
      );

      this.lastFeatureDrawnIndex += FEATURE_DRAW_PER_FRAME;
    } else {
      this.lastLayerDrawnIndex++;
      this.lastFeatureDrawnIndex = 0;
    }
  }

  private updateLabelClasses() {
    const btvRwy = this.btvUtils.btvRunway.get();
    const btvExit = this.btvUtils.btvExit.get();
    this.labelManager.updateLabelClasses(
      this.fmsDataStore,
      this.dataAirportIcao.get() === this.fmsDataStore.origin.get(),
      this.dataAirportIcao.get() === this.fmsDataStore.destination.get(),
      btvRwy !== null ? btvRwy : undefined,
      btvExit !== null ? btvExit : undefined,
    );
  }

  private clearData(): void {
    for (const layer of this.layerFeatures) {
      layer.features.length = 0;
    }
    pathCache.clear();
    pathIdCache.clear();

    this.labelManager.clearLabels();
    this.markerManager.eraseAllFlags();
    this.markerManager.eraseAllCrosses();
  }

  private clearMap(): void {
    this.lastLayerDrawnIndex = 0;
    this.lastFeatureDrawnIndex = 0;

    for (const layer of this.layerCanvasRefs) {
      const ctx = layer.target.getContext('2d');
      const cw = this.canvasWidth.get();
      const ch = this.canvasHeight.get();

      if (ctx) {
        ctx.clearRect(0, 0, cw, ch);
      }
    }

    this.canvasWidth.set(0);
    this.canvasHeight.set(0);

    this.canvasCentreX.set(0);
    this.canvasCentreY.set(0);

    this.panOffsetX.set(0);
    this.panOffsetY.set(0);
  }

  public async disablePanningTransitions(): Promise<void> {
    for (const container of this.panContainerRef) {
      container.target.style.transition = 'reset';
    }

    await Waits.frames(1);

    this.panBeingAnimated.set(false);
  }

  public async enablePanningTransitions(): Promise<void> {
    for (const container of this.panContainerRef) {
      container.target.style.transition = `transform ${ZOOM_TRANSITION_TIME_MS}ms linear`;
    }

    await Waits.frames(1);

    this.panBeingAnimated.set(true);
  }

  private async handleNDModeChange(newMode: EfisNdMode) {
    if (this.panOffsetX.get() !== 0 || this.panOffsetY.get() !== 0) {
      // We need to first animate to the default position
      await this.enablePanningTransitions();

      this.panOffsetX.set(0);
      this.panOffsetY.set(0);
      await Waits.delay(ZOOM_TRANSITION_TIME_MS);

      await this.disablePanningTransitions();
    }

    switch (newMode) {
      case EfisNdMode.ROSE_NAV:
        this.modeAnimationOffsetX.set(0);
        this.modeAnimationOffsetY.set(0);
        break;
      case EfisNdMode.ARC:
        this.modeAnimationOffsetX.set(0);
        this.modeAnimationOffsetY.set(620 - OANC_RENDER_HEIGHT / 2);
        break;
      case EfisNdMode.PLAN:
        this.modeAnimationOffsetX.set(0);
        this.modeAnimationOffsetY.set(0);
        break;
      default:
      // noop
    }

    this.ndModeSwitchDelayDebouncer.arm(() => {
      this.overlayNDModeSub.set(newMode);

      switch (newMode) {
        case EfisNdMode.ROSE_NAV:
          this.modeAnimationMapNorthUp.set(false);
          break;
        case EfisNdMode.ARC:
          this.modeAnimationMapNorthUp.set(false);
          break;
        case EfisNdMode.PLAN:
          this.modeAnimationMapNorthUp.set(true);
          break;
        default:
        // noop
      }
    }, ZOOM_TRANSITION_TIME_MS);
  }

  private handleLayerVisibilities() {
    for (let i = 0; i < this.layerCanvasScaleContainerRefs.length; i++) {
      const shouldBeVisible = LAYER_SPECIFICATIONS[i].zoomLevelVisibilities[this.zoomLevelIndex.get()];

      const layerContainer = this.layerCanvasScaleContainerRefs[i].target;

      layerContainer.style.visibility = shouldBeVisible ? 'inherit' : 'hidden';
    }
  }

  public handleZoomIn(): void {
    if (this.zoomLevelIndex.get() !== 0) {
      this.zoomLevelIndex.set(this.zoomLevelIndex.get() - 1);
    }
  }

  public handleZoomOut(): void {
    if (this.zoomLevelIndex.get() !== this.props.zoomValues.length - 1) {
      this.zoomLevelIndex.set(this.zoomLevelIndex.get() + 1);
    }
  }

  public handleCursorPanStart(event: MouseEvent): void {
    if (this.dataAirportIcao.get()) {
      this.isPanningArmed = true;
      this.panArmedX.set(event.screenX);
      this.panArmedY.set(event.screenY);
    }
  }

  public handleCursorPanMove(event: MouseEvent): void {
    if (this.isPanningArmed) {
      const adx = Math.abs(event.screenX - this.panArmedX.get());
      const ady = Math.abs(event.screenY - this.panArmedY.get());

      // We only actually start panning if we move more than a certain amount - this is to ensure we can differentiate between panning
      // and opening the context menu
      if (adx > PAN_MIN_MOVEMENT || ady > PAN_MIN_MOVEMENT) {
        this.isPanningArmed = false;
        this.isPanning = true;
      }
    }

    if (this.isPanning) {
      this.panOffsetX.set(this.panOffsetX.get() + event.screenX - this.lastPanX);
      this.panOffsetY.set(this.panOffsetY.get() + event.screenY - this.lastPanY);
    }

    this.lastPanX = event.screenX;
    this.lastPanY = event.screenY;
  }

  public handleCursorPanStop(event: MouseEvent): void {
    this.props.contextMenuX?.set(event.screenX);
    this.props.contextMenuY?.set(event.screenY);
    if (!this.isPanning) {
      this.isPanningArmed = false;
      this.props.contextMenuVisible?.set(!this.props.contextMenuVisible.get());
    }
    this.isPanning = false;
  }

  public projectPoint(coordinates: Position): [number, number] {
    const labelX = coordinates[0];
    const labelY = coordinates[1];

    // eslint-disable-next-line prefer-const
    let [offsetX, offsetY] = this.arpReferencedMapParams.coordinatesToXYy(this.referencePos);

    // TODO figure out how to not need this
    offsetY *= -1;

    const mapCurrentHeading = this.interpolatedMapHeading.get();
    const rotate = -mapCurrentHeading;

    const hypotenuse = Math.sqrt(labelX ** 2 + labelY ** 2) * this.getZoomLevelInverseScale();
    const angle = clampAngle(Math.atan2(labelY, labelX) * MathUtils.RADIANS_TO_DEGREES);

    const rotationAdjustX = hypotenuse * Math.cos((angle - rotate) * MathUtils.DEGREES_TO_RADIANS);
    const rotationAdjustY = hypotenuse * Math.sin((angle - rotate) * MathUtils.DEGREES_TO_RADIANS);

    const scaledOffsetX = offsetX * this.getZoomLevelInverseScale();
    const scaledOffsetY = offsetY * this.getZoomLevelInverseScale();

    let labelScreenX = OANC_RENDER_WIDTH / 2 + rotationAdjustX + -scaledOffsetX + this.panOffsetX.get();
    let labelScreenY = OANC_RENDER_HEIGHT / 2 + -rotationAdjustY + scaledOffsetY + this.panOffsetY.get();

    labelScreenX += this.modeAnimationOffsetX.get();
    labelScreenY += this.modeAnimationOffsetY.get();

    return [labelScreenX, labelScreenY];
  }

  public unprojectPoint(screenCoordinates: [number, number]): Position {
    let [labelScreenX, labelScreenY] = screenCoordinates;

    // Undo animation offsets
    labelScreenX -= this.modeAnimationOffsetX.get() + OANC_RENDER_WIDTH / 2 + this.panOffsetX.get();
    labelScreenY -= this.modeAnimationOffsetY.get() + OANC_RENDER_HEIGHT / 2 + this.panOffsetY.get();

    const zoomLevelScale = this.getZoomLevelInverseScale();
    const [offsetX, offsetY] = this.arpReferencedMapParams.coordinatesToXYy(this.referencePos);

    // Undo scaling offsets
    const scaledOffsetX = offsetX * zoomLevelScale;
    const scaledOffsetY = -1 * (offsetY * zoomLevelScale);
    labelScreenX += scaledOffsetX;
    labelScreenY -= scaledOffsetY;

    // Reverse rotation
    const mapCurrentHeading = this.interpolatedMapHeading.get();
    const rotate = -mapCurrentHeading; // Original rotation
    const reverseRotate = -rotate; // Undo rotation

    const hypotenuse = Math.sqrt(labelScreenX ** 2 + labelScreenY ** 2);
    const angle = clampAngle(Math.atan2(-labelScreenY, labelScreenX) * MathUtils.RADIANS_TO_DEGREES);

    const originalX = hypotenuse * Math.cos((angle - reverseRotate) * MathUtils.DEGREES_TO_RADIANS);
    const originalY = hypotenuse * Math.sin((angle - reverseRotate) * MathUtils.DEGREES_TO_RADIANS);

    // Scale back the original position
    const labelX = originalX / zoomLevelScale;
    const labelY = originalY / zoomLevelScale;

    return [labelX, labelY];
  }

  public offsetToPoint(coordinates: Position): [number, number] {
    const projected = this.projectPoint(coordinates);
    const xOffset = -(projected[0] - this.panOffsetX.get() - OANC_RENDER_WIDTH / 2 - this.modeAnimationOffsetX.get());
    const yOffset = -(projected[1] - this.panOffsetY.get() - OANC_RENDER_HEIGHT / 2 - this.modeAnimationOffsetY.get());

    return [xOffset, yOffset];
  }

  async centerOnAcft() {
    await this.enablePanningTransitions();
    this.panOffsetX.set(0);
    this.panOffsetY.set(0);
    await Waits.delay(ZOOM_TRANSITION_TIME_MS);
    await this.disablePanningTransitions();
  }

  /**
   * Centers map on point supplied in parameters
   * @param x X position in local coordinate system
   * @param y Y position in local coordinate system
   */
  async centerMapOn(pos: Position) {
    const xy = this.offsetToPoint(pos);
    await this.enablePanningTransitions();
    this.panOffsetX.set(xy[0]);
    this.panOffsetY.set(xy[1]);
    await Waits.delay(ZOOM_TRANSITION_TIME_MS);
    await this.disablePanningTransitions();
  }

  render(): ViewNode | null {
    return (
      <>
        <div
          class="oanc-flag-container FontSmall"
          style={{ visibility: this.pleaseWaitFlagVisible.derive((v) => (v ? 'inherit' : 'hidden')) }}
        >
          PLEASE WAIT
        </div>
        <div
          class="oanc-flag-container amber FontLarge"
          style={{ visibility: this.arptNavPosLostFlagVisible.derive((v) => (v ? 'inherit' : 'hidden')) }}
        >
          ARPT NAV POS LOST
        </div>

        <div style={{ display: this.anyFlagVisible.derive((v) => (v ? 'none' : 'block')) }}>
          <svg viewBox="0 0 768 768" style="position: absolute;">
            <defs>
              <clipPath id="oanc-rose-mode-map-clip">
                <path d="M45,155 L282,155 a250,250 0 0 1 204,0 L723,155 L723,562 L648,562 L591,625 L591,768 L174,768 L174,683 L122,625 L45,625 L45,155" />
              </clipPath>
              <clipPath id="oanc-rose-mode-wx-terr-clip">
                <path d="M45,155 L282,155 a250,250 0 0 1 204,0 L723,155 L723,384 L45,384 L45,155" />
              </clipPath>
              <clipPath id="oanc-rose-mode-tcas-clip">
                <path d="M45,155 L282,155 a250,250 0 0 1 204,0 L723,155 L723,562 L648,562 L591,625 L591,768 L174,768 L174,683 L122,625 L45,625 L45,155" />
              </clipPath>
              <clipPath id="oanc-arc-mode-map-clip">
                <path d="M0,312 a492,492 0 0 1 768,0 L768,562 L648,562 L591,625 L591,768 L174,768 L174,683 L122,625 L0,625 L0,312" />
              </clipPath>
              <clipPath id="oanc-arc-mode-wx-terr-clip">
                <path d="M0,312 a492,492 0 0 1 768,0 L768,562 L648,562 L591,625 L0,625 L0,312" />
              </clipPath>
              <clipPath id="oanc-arc-mode-tcas-clip">
                <path d="M0,312 a492,492 0 0 1 768,0 L768,562 L648,562 L591,625 L591,768 L174,768 L174,683 L122,625 L0,625 L0,312" />
              </clipPath>
              <clipPath id="oanc-arc-mode-overlay-clip-4">
                <path d="m 6 0 h 756 v 768 h -756 z" />
              </clipPath>
              <clipPath id="oanc-arc-mode-overlay-clip-3">
                <path d="m 0 564 l 384 145 l 384 -145 v -564 h -768 z" />
              </clipPath>
              <clipPath id="oanc-arc-mode-overlay-clip-2">
                <path d="m 0 532 l 384 155 l 384 -146 v -512 h -768 z" />
              </clipPath>
              <clipPath id="oanc-arc-mode-overlay-clip-1">
                <path d="m 0 519 l 384 145 l 384 -86 v -580 h -768 z" />
              </clipPath>
            </defs>
          </svg>

          <div
            ref={this.animationContainerRef[0]}
            style={`position: absolute; transition: transform ${ZOOM_TRANSITION_TIME_MS}ms linear;`}
          >
            <div ref={this.panContainerRef[0]} style="position: absolute;">
              {this.layerCanvasScaleContainerRefs.map((ref, index) => (
                <div ref={ref} style={`position: absolute; transition: transform ${ZOOM_TRANSITION_TIME_MS}ms linear;`}>
                  <canvas
                    ref={this.layerCanvasRefs[index]}
                    width={this.canvasWidth.derive((it) => it * LAYER_SPECIFICATIONS[index].renderScale)}
                    height={this.canvasHeight.derive((it) => it * LAYER_SPECIFICATIONS[index].renderScale)}
                  />
                </div>
              ))}
            </div>
          </div>

          <div
            ref={this.labelContainerRef}
            style={`position: absolute; width: ${OANC_RENDER_WIDTH}px; height: ${OANC_RENDER_HEIGHT}px; pointer-events: auto;`}
          />

          <OancStaticModeOverlay
            bus={this.props.bus}
            oansRange={this.zoomLevelIndex.derive((it) => this.props.zoomValues[it])}
            ndMode={this.overlayNDModeSub}
            rotation={this.interpolatedMapHeading}
            isMapPanned={this.isMapPanned}
            airportWithinRange={this.airportWithinRange}
            airportBearing={this.airportBearing}
            airportIcao={this.dataAirportIcao}
          />

          <div
            ref={this.animationContainerRef[1]}
            style={`position: absolute; transition: transform ${ZOOM_TRANSITION_TIME_MS}ms linear; pointer-events: none;`}
          >
            <OancMovingModeOverlay
              bus={this.props.bus}
              oansRange={this.zoomLevelIndex.derive((it) => this.props.zoomValues[it])}
              ndMode={this.overlayNDModeSub}
              rotation={this.interpolatedMapHeading}
              isMapPanned={this.isMapPanned}
              airportWithinRange={this.airportWithinRange}
              airportBearing={this.airportBearing}
              airportIcao={this.dataAirportIcao}
            />

            <svg
              class="nd-svg nd-top-layer"
              viewBox="0 0 768 768"
              style={this.efisNDModeSub.derive(
                (mode) =>
                  `transform: translateY(${mode === EfisNdMode.ARC ? -236 : 0}px); pointer-events: none; z-index: 99;`,
              )}
            >
              <LubberLine
                bus={this.props.bus}
                visible={Derived.from(
                  ([ac, mode]) => ac && mode !== EfisNdMode.PLAN,
                  this.showAircraft,
                  this.efisNDModeSub,
                )}
                rotation={Signal.of(0)}
                ndMode={this.efisNDModeSub}
                colorClass="Magenta"
              />
            </svg>

            <div ref={this.panContainerRef[1]} style="position: absolute;">
              <OancAircraftIcon
                isVisible={this.showAircraft}
                x={this.aircraftX}
                y={this.aircraftY}
                rotation={this.aircraftRotation}
              />
            </div>
          </div>

          <div
            style={`position: absolute; width: ${OANC_RENDER_WIDTH}px; height: ${OANC_RENDER_HEIGHT}px; pointer-events: none`}
          >
            <div class="oanc-top-mask" />
            <div class="oanc-bottom-mask">
              <span
                class="oanc-position"
                style={{
                  display: this.positionVisible.derive((it) => (it ? 'block' : 'none')),
                }}
              >
                {this.positionString}
              </span>

              <span
                class="oanc-bottom-flag FontSmall"
                style={{
                  display: this.pposNotAvailable.derive((it) => (it ? 'block' : 'none')),
                }}
              >
                ARPT NAV POS LOST
              </span>
            </div>

            <span class="oanc-airport-info" id="oanc-airport-info-line1">
              {this.airportInfoLine1}
            </span>
            <span class="oanc-airport-info" id="oanc-airport-info-line2">
              {this.airportInfoLine2}
            </span>
            <span
              class="oanc-airport-not-in-active-fpln"
              style={{ display: this.airportNotInActiveFpln.derive((it) => (it ? 'inherit' : 'none')) }}
            >
              ARPT NOT IN
              <br />
              ACTIVE F/PLN
            </span>
          </div>
        </div>
      </>
    );
  }
}

const pathCache = new Map<string, Path2D[]>();
const pathIdCache = new Map<Feature, string>();

function renderFeaturesToCanvas(
  layer: number,
  ctx: CanvasRenderingContext2D,
  data: FeatureCollection<Geometry, AmdbProperties>,
  startIndex: number,
  endIndex: number,
) {
  const layerSpec = LAYER_SPECIFICATIONS[layer];

  for (let i = startIndex; i < Math.min(endIndex, data.features.length); i++) {
    const feature = data.features[i];
    let doStroke = false;

    let doFill = false;

    const matchingRule = layerSpec.styleRules.find((it) => {
      if (feature.properties.feattype === FeatureType.VerticalPolygonalStructure) {
        return (
          it.forFeatureTypes?.includes(feature.properties.feattype) &&
          feature.properties.plysttyp &&
          it.forPolygonStructureTypes?.includes(feature.properties.plysttyp)
        );
      }

      return it.forFeatureTypes?.includes(feature.properties.feattype);
    });

    if (!matchingRule) {
      console.error(
        `No matching style rule for feature (feattype=${feature.properties.feattype}) in rules for layer #${layer}`,
      );
      continue;
    }

    if (matchingRule.styles.doStroke !== undefined) {
      doStroke = matchingRule.styles.doStroke;
    }

    if (matchingRule.styles.doFill !== undefined) {
      doFill = matchingRule.styles.doFill;
    }

    if (matchingRule.styles.strokeStyle !== undefined) {
      ctx.strokeStyle = matchingRule.styles.strokeStyle;
    }

    if (matchingRule.styles.lineWidth !== undefined) {
      ctx.lineWidth = matchingRule.styles.lineWidth;
    }

    if (matchingRule.styles.fillStyle !== undefined) {
      ctx.fillStyle = matchingRule.styles.fillStyle;
    }

    let id = pathIdCache.get(feature);
    if (!id) {
      id = `${feature.properties.id}-${feature.properties.feattype}`;

      pathIdCache.set(feature, id);
    }

    const usePathCache = matchingRule.unionBy === undefined;
    const cachedPaths = pathCache.get(id);

    switch (feature.geometry.type) {
      case 'LineString': {
        const outline = feature.geometry as LineString;

        let path: Path2D;
        if (usePathCache && cachedPaths) {
          // eslint-disable-next-line prefer-destructuring
          path = cachedPaths[0];
        } else {
          path = new Path2D();

          path.moveTo(outline.coordinates[0][0], outline.coordinates[0][1] * -1);

          for (let i = 1; i < outline.coordinates.length; i++) {
            const point = outline.coordinates[i];
            path.lineTo(point[0], point[1] * -1);
          }

          pathCache.set(`${feature.properties.id}-${feature.properties.feattype}`, [path]);
        }

        if (doFill) {
          ctx.fill(path);
        }

        if (doStroke) {
          ctx.stroke(path);
        }

        break;
      }
      case 'Polygon': {
        const polygon = feature.geometry as Polygon;

        let paths: Path2D[] = [];
        if (usePathCache && cachedPaths) {
          paths = cachedPaths;
        } else {
          for (const outline of polygon.coordinates) {
            const toCachePath = new Path2D();

            toCachePath.moveTo(outline[0][0], outline[0][1] * -1);

            for (let i = 1; i < outline.length; i++) {
              if (i === outline.length - 1) {
                toCachePath.closePath();
              } else {
                const point = outline[i];
                toCachePath.lineTo(point[0], point[1] * -1);
              }
            }

            paths.push(toCachePath);
          }

          pathCache.set(`${feature.properties.id}-${feature.properties.feattype}`, paths);
        }

        for (const path of paths) {
          if (doStroke) {
            ctx.stroke(path);
          }

          if (doFill) {
            ctx.fill(path);
          }
        }
        break;
      }
      default: {
        console.log(`Could not draw geometry of type: ${feature.geometry.type}`);
        break;
      }
    }
  }
}

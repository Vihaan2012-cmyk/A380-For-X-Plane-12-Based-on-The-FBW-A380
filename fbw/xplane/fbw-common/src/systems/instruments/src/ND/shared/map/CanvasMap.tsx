// @ts-strict-ignore
// Copyright (c) 2021-2023 FlyByWire Simulations
//
// SPDX-License-Identifier: GPL-3.0

import {
  Bits,
  ClockTopics,
  View,
  Bus,
  UI,
  Derived,
  Signal,
  Readable,
  ViewNode,
} from 'xplane-js/ui';
import {
  EfisNdMode,
  EfisVectorsGroup,
  NdSymbol,
  NdSymbolTypeFlags,
  NdTraffic,
  MathUtils,
  PathVector,
  NdPwpSymbolTypeFlags,
  SimVarString,
} from '@flybywiresim/fbw-sdk';

import { Coordinates, distanceTo, placeBearingDistance } from 'msfs-geo';
import { FmsSymbolsData } from '../../FmsSymbolsPublisher';
import { MapParameters } from '../utils/MapParameters';
import { NDSimvars } from '../../NDSimvarPublisher';
import { WaypointLayer } from './WaypointLayer';
import { ConstraintsLayer } from './ConstraintsLayer';
import { RunwayLayer } from './RunwayLayer';
import { TrafficLayer } from './TrafficLayer';
import { FixInfoLayer } from './FixInfoLayer';
import { NDControlEvents } from '../../NDControlEvents';
import { PseudoWaypointLayer } from './PseudoWaypointLayer';
import { GenericFcuEvents } from '../../types/GenericFcuEvents';
import { MapOptions } from '../../types/MapOptions';
import { GenericTawsEvents } from '../../types/GenericTawsEvents';
import { PaintUtils } from './PaintUtils';

/** The FMS's FLS beam, published by aircraft that have one (A380 FGDataPublisher). */
interface FlsBeamTopics {
  'fg.fls.beam': number;
  'fg.fls.anchorLat': number;
  'fg.fls.anchorLong': number;
  'fg.fls.course': number;
  'fg.fls.anchorPseudo': number;
  'fg.fls.ident0': number;
  'fg.fls.ident1': number;
}

/** FCOM DSC-22-FMS: the FLS beam is a magenta dotted line 80 NM long. */
const FLS_BEAM_NM = 80;
const FLS_DOTS = [3, 7];

// TODO move this somewhere better, need to move TCAS stuff into fbw-sdk
enum TaRaIntrusion {
  TRAFFIC = 0,
  PROXIMITY = 1,
  TA = 2,
  RA = 3,
}

const ARC_CLIP = new Path2D(
  'M0,312 a492,492 0 0 1 768,0 L768,562 L648,562 L591,625 L591,768 L174,768 L174,683 L122,625 L0,625 L0,312',
);

const ROSE_CLIP = new Path2D(
  'M45,155 L282,155 a250,250 0 0 1 204,0 L723,155 L723,562 L648,562 L591,625 L591,768 L174,768 L174,683 L122,625 L45,625 L45,155',
);

const PLAN_CLIP = new Path2D('M45,112 L140 112 280 56 488 56 628 112 723 112 723 720 114 720 114 633 45 633z');

const DEFAULT_CLIP = new Path2D(
  'M0,312 a492,492 0 0 1 768,0 L768,562 L648,562 L591,625 L591,768 L174,768 L174,683 L122,625 L0,625 L0,312',
);

const DASHES = [15, 12];
const NO_DASHES = [];

export interface CanvasMapProps {
  bus: Bus;
  x: Readable<number>;
  y: Readable<number>;
  options?: Partial<MapOptions>;
}

export class CanvasMap extends View<CanvasMapProps> {
  private readonly canvasRef = UI.ref<HTMLCanvasElement>();

  private readonly touchContainerRef = UI.ref<HTMLDivElement>();

  private readonly mapCenterLat = Signal.of<number>(-1);

  private readonly mapCenterLong = Signal.of<number>(-1);

  private readonly mapCenterYBias = Signal.of<number>(-1);

  private readonly mapRotation = Signal.of<number>(-1);

  private readonly mapPixelRadius = Signal.of<number>(-1);

  private readonly mapRangeRadius = Signal.of<number>(-1);

  private readonly mapMode = Signal.of<EfisNdMode | -1>(-1);

  private readonly mapVisible = Signal.of<boolean>(false);

  private readonly mapRecomputing = Signal.of<boolean>(false);

  private readonly vectors: { [k in EfisVectorsGroup]: PathVector[] } = {
    [EfisVectorsGroup.ACTIVE]: [],
    [EfisVectorsGroup.DASHED]: [],
    [EfisVectorsGroup.OFFSET]: [],
    [EfisVectorsGroup.TEMPORARY]: [],
    [EfisVectorsGroup.SECONDARY]: [],
    [EfisVectorsGroup.SECONDARY_DASHED]: [],
    [EfisVectorsGroup.MISSED]: [],
    [EfisVectorsGroup.ALTERNATE]: [],
    [EfisVectorsGroup.ACTIVE_EOSID]: [],
  };

  private readonly symbols: NdSymbol[] = [];

  private readonly traffic: NdTraffic[] = [];

  private readonly mapParams = new MapParameters();

  public pointerX = 0;

  public pointerY = 0;

  private readonly constraintsLayer = new ConstraintsLayer();

  private readonly waypointLayer = new WaypointLayer(this, this.props.options);

  private readonly fixInfoLayer = new FixInfoLayer();

  private readonly runwayLayer = new RunwayLayer();

  private readonly pwpLayer = new PseudoWaypointLayer(this.props.bus);

  private readonly trafficLayer = new TrafficLayer(this);

  private lastFrameTimestamp: number = 0;

  private readonly fls = { beam: 0, lat: 0, long: 0, course: 0, pseudo: 0, ident: [0, 0], ls: false };

  /** Something on the map changed since the canvas was last drawn. */
  private dirty = true;

  /** The view the canvas was last drawn for. */
  private drawn = { lat: 0, long: 0, rotation: 0, range: 0, radius: 0, bias: 0, mode: -2, at: 0 };

  afterRender(node: ViewNode) {
    super.afterRender(node);

    const sub = this.props.bus.inbox<NDControlEvents & GenericFcuEvents & GenericTawsEvents>();

    sub.topic('set_show_map').listen((show) => this.mapVisible.set(show));
    sub.topic('set_map_recomputing').listen((show) => this.mapRecomputing.set(show));
    sub.topic('set_map_center_lat').listen((v) => this.mapCenterLat.set(v));
    sub.topic('set_map_center_lon').listen((v) => this.mapCenterLong.set(v));
    sub.topic('set_map_center_y_bias').listen((v) => this.mapCenterYBias.set(v));
    sub.topic('set_map_up_course').listen((v) => this.mapRotation.set(v));
    sub.topic('set_map_pixel_radius').listen((v) => this.mapPixelRadius.set(v));
    sub.topic('set_map_range_radius').listen((v) => this.mapRangeRadius.set(v));

    sub
      .topic('ndMode')
      .distinct()
      .listen((v) => this.mapMode.set(v));

    this.setupCallbacks();
    this.setupEvents();
  }

  private setupCallbacks() {
    const sub = this.props.bus.inbox<NDSimvars & FmsSymbolsData & ClockTopics>();

    this.mapCenterLat.watch(() => {
      this.handleRecomputeMapParameters();
    });

    this.mapCenterLong.watch(() => {
      this.handleRecomputeMapParameters();
    });

    this.mapCenterYBias.watch(() => {
      this.handleRecomputeMapParameters();
    });

    this.mapRotation.watch(() => {
      this.handleRecomputeMapParameters();
    });

    this.mapPixelRadius.watch(() => {
      this.handleRecomputeMapParameters();
    });

    this.mapRangeRadius.watch(() => {
      this.handleRecomputeMapParameters();
    });

    Derived.from(
      ([mapVisible, recomputing]) => {
        const visible = mapVisible && !recomputing;

        this.canvasRef.target.style.visibility = visible ? 'visible' : 'hidden';
      },
      this.mapVisible,
      this.mapRecomputing,
    );

    sub.topic('symbols').listen((data: NdSymbol[]) => {
      this.handleNewSymbols(data);
    });

    sub.topic('vectorsActive').listen((data: PathVector[]) => {
      this.dirty = true;
      this.vectors[EfisVectorsGroup.ACTIVE].length = 0;
      if (data) {
        this.vectors[EfisVectorsGroup.ACTIVE].push(...data);
      }
    });

    sub.topic('vectorsActiveEosid').listen((data: PathVector[]) => {
      this.dirty = true;
      this.vectors[EfisVectorsGroup.ACTIVE_EOSID].length = 0;
      if (data) {
        this.vectors[EfisVectorsGroup.ACTIVE_EOSID].push(...data);
      }
    });

    sub.topic('vectorsDashed').listen((data: PathVector[]) => {
      this.dirty = true;
      this.vectors[EfisVectorsGroup.DASHED].length = 0;
      if (data) {
        this.vectors[EfisVectorsGroup.DASHED].push(...data);
      }
    });

    sub.topic('vectorsTemporary').listen((data: PathVector[]) => {
      this.dirty = true;
      this.vectors[EfisVectorsGroup.TEMPORARY].length = 0;
      if (data) {
        this.vectors[EfisVectorsGroup.TEMPORARY].push(...data);
      }
    });

    sub.topic('vectorsMissed').listen((data: PathVector[]) => {
      this.dirty = true;
      this.vectors[EfisVectorsGroup.MISSED].length = 0;
      if (data) {
        this.vectors[EfisVectorsGroup.MISSED].push(...data);
      }
    });

    sub.topic('vectorsAlternate').listen((data: PathVector[]) => {
      this.dirty = true;
      this.vectors[EfisVectorsGroup.ALTERNATE].length = 0;
      if (data) {
        this.vectors[EfisVectorsGroup.ALTERNATE].push(...data);
      }
    });

    sub.topic('vectorsSecondary').listen((data: PathVector[]) => {
      this.dirty = true;
      this.vectors[EfisVectorsGroup.SECONDARY].length = 0;
      if (data) {
        this.vectors[EfisVectorsGroup.SECONDARY].push(...data);
      }
    });

    const fls = this.props.bus.inbox<FlsBeamTopics & GenericFcuEvents>();
    fls.topic('fg.fls.beam').listen((v) => {
      this.fls.beam = v;
      this.dirty = true;
    });
    fls.topic('fg.fls.anchorLat').listen((v) => {
      this.fls.lat = v;
      this.dirty = true;
    });
    fls.topic('fg.fls.anchorLong').listen((v) => {
      this.fls.long = v;
      this.dirty = true;
    });
    fls.topic('fg.fls.course').listen((v) => {
      this.fls.course = v;
      this.dirty = true;
    });
    fls.topic('fg.fls.anchorPseudo').listen((v) => {
      this.fls.pseudo = v;
      this.dirty = true;
    });
    fls.topic('fg.fls.ident0').listen((v) => {
      this.fls.ident[0] = v;
      this.dirty = true;
    });
    fls.topic('fg.fls.ident1').listen((v) => {
      this.fls.ident[1] = v;
      this.dirty = true;
    });
    fls.topic('efisLsActive').listen((v) => {
      this.fls.ls = v;
      this.dirty = true;
    });

    sub.topic('traffic').listen((data: NdTraffic[]) => {
      this.handleNewTraffic(data);
    });

    sub
      .topic('simTime')
      .distinctBy(8)
      .listen((value) => {
        this.handleFrame(value - this.lastFrameTimestamp);

        this.lastFrameTimestamp = value;
      });
  }

  private setupEvents() {
    const touchContainer = this.touchContainerRef.target;

    touchContainer.addEventListener('mousemove', (e) => {
      this.pointerX = e.offsetX;
      this.pointerY = e.offsetY;
    });
  }

  private handleRecomputeMapParameters() {
    this.mapParams.compute(
      { lat: this.mapCenterLat.get(), long: this.mapCenterLong.get() },
      this.mapCenterYBias.get(),
      this.mapRangeRadius.get(),
      this.mapPixelRadius.get(),
      this.mapRotation.get(),
    );
  }

  private handleNewSymbols(symbols: NdSymbol[]) {
    this.dirty = true;
    this.symbols.length = 0;
    this.symbols.push(...symbols);

    const waypoints = this.symbols.filter(
      (it) =>
        it.constraints ||
        (Bits.any(
          it.type,
          NdSymbolTypeFlags.Waypoint |
            NdSymbolTypeFlags.FlightPlan |
            NdSymbolTypeFlags.FixInfo |
            NdSymbolTypeFlags.VorDme |
            NdSymbolTypeFlags.Vor |
            NdSymbolTypeFlags.Dme |
            NdSymbolTypeFlags.Ndb |
            NdSymbolTypeFlags.Airport,
        ) &&
          !(it.type & NdSymbolTypeFlags.Runway)),
    );

    this.waypointLayer.data = waypoints;

    const fixInfoSymbols = this.symbols.filter((it) => it.type & NdSymbolTypeFlags.FixInfo);

    this.fixInfoLayer.data = fixInfoSymbols;

    const constraints = this.symbols.filter((it) => it.type & NdSymbolTypeFlags.Constraint);

    this.constraintsLayer.data = constraints;

    const runways = this.symbols.filter((it) => it.type & NdSymbolTypeFlags.Runway);

    this.runwayLayer.data = runways;

    const pseudoWaypoints = this.symbols.filter(
      (it) =>
        it.type & (NdSymbolTypeFlags.CourseReversalLeft | NdSymbolTypeFlags.CourseReversalRight) ||
        (it.typePwp !== undefined &&
          it.typePwp &
            (NdPwpSymbolTypeFlags.PwpEndOfVdMarker |
              NdPwpSymbolTypeFlags.PwpStartOfClimb |
              NdPwpSymbolTypeFlags.PwpClimbLevelOff |
              NdPwpSymbolTypeFlags.PwpTopOfDescent |
              NdPwpSymbolTypeFlags.PwpDescentLevelOff |
              NdPwpSymbolTypeFlags.PwpInterceptProfile |
              NdPwpSymbolTypeFlags.PwpCdaFlap1 |
              NdPwpSymbolTypeFlags.PwpCdaFlap2 |
              NdPwpSymbolTypeFlags.PwpDecel |
              NdPwpSymbolTypeFlags.PwpTimeMarker |
              NdPwpSymbolTypeFlags.PwpSpeedChange)),
    );

    this.pwpLayer.data = pseudoWaypoints;
  }

  private handleNewTraffic(newTraffic: NdTraffic[]) {
    this.dirty = true;
    this.traffic.length = 0; // Reset traffic display
    if (this.mapMode.get() !== EfisNdMode.PLAN) {
      newTraffic.forEach((intruder: NdTraffic) => {
        const latLong: Coordinates = { lat: intruder.lat, long: intruder.lon };
        let [x, y] = this.mapParams.coordinatesToXYy(latLong);

        let tcasMask;
        switch (this.mapMode.get()) {
          case EfisNdMode.ARC:
            tcasMask = [
              [-384, -83],
              [-384, 227],
              [-264, 227],
              [-210, 286],
              [-210, 370],
              [210, 370],
              [210, 227],
              [267, 166],
              [384, 166],
              [384, -83],
              [340, -128],
              [300, -163],
              [240, -204.5],
              [180, -233],
              [100, -255],
              [0, -265],
              [-100, -255],
              [-180, -233],
              [-240, -204.5],
              [-300, -163],
              [-340, -128],
              [-384, -83],
            ];
            break;
          case EfisNdMode.ROSE_NAV:
          case EfisNdMode.ROSE_ILS:
          case EfisNdMode.ROSE_VOR:
            tcasMask = [
              [-340, -227],
              [-103, -227],
              [-50, -244],
              [0, -250],
              [50, -244],
              [103, -227],
              [340, -227],
              [340, 180],
              [267, 180],
              [210, 241],
              [210, 383],
              [-210, 383],
              [-210, 300],
              [-264, 241],
              [-340, 241],
              [-340, -227],
            ];
            break;
          default:
            break;
        }

        // Full time option installed: For all ranges except in ZOOM ranges NDRange > 9NM
        if (tcasMask !== undefined && !MathUtils.pointInPolygon(x, y, tcasMask)) {
          if (intruder.intrusionLevel < TaRaIntrusion.TA) {
            // Remove if beyond viewable range
            return;
          }
          const ret: [number, number] | null = MathUtils.intersectWithPolygon(x, y, 0, 0, tcasMask);
          if (ret) [x, y] = ret;
        }
        intruder.posX = x;
        intruder.posY = y;
        this.traffic.push(intruder);
      });
    }
    this.trafficLayer.data = this.traffic; // Populate with new traffic
  }

  /**
   * Whether the canvas needs drawing again: new data, or the view moved by half a pixel, turned by a tenth of a
   * degree or changed scale. Redrawing an unchanged map made Chromium repaint the whole ND every frame.
   */
  private needsDraw(now: number): boolean {
    const d = this.drawn;
    const view = {
      lat: this.mapCenterLat.get(),
      long: this.mapCenterLong.get(),
      rotation: this.mapRotation.get(),
      range: this.mapRangeRadius.get(),
      radius: this.mapPixelRadius.get(),
      bias: this.mapCenterYBias.get(),
      mode: this.mapMode.get(),
    };
    const [x, y] = this.mapParams.coordinatesToXYy({ lat: d.lat, long: d.long });
    const moved = Math.hypot(x, y - view.bias);
    const turned = Math.abs(((view.rotation - d.rotation + 540) % 360) - 180);
    const changed =
      this.dirty ||
      !(moved < 0.5) ||
      turned > 0.1 ||
      view.range !== d.range ||
      view.radius !== d.radius ||
      view.bias !== d.bias ||
      view.mode !== d.mode ||
      now - d.at > 1000;
    if (changed) {
      this.drawn = { ...view, at: now };
      this.dirty = false;
    }
    return changed;
  }

  private handleFrame(_deltaTime: number) {
    if (!this.needsDraw(this.lastFrameTimestamp)) {
      return;
    }
    const canvas = this.canvasRef.target;
    const context = canvas.getContext('2d');

    const size = 768;

    context.clearRect(0, 0, size, size);

    switch (this.mapMode.get()) {
      case EfisNdMode.ARC:
        context.clip(ARC_CLIP);
        break;
      case EfisNdMode.ROSE_NAV:
      case EfisNdMode.ROSE_ILS:
      case EfisNdMode.ROSE_VOR:
        context.clip(ROSE_CLIP);
        break;
      case EfisNdMode.PLAN:
        context.clip(PLAN_CLIP);
        break;
      default:
        context.clip(DEFAULT_CLIP);
        break;
    }
    context.resetTransform();

    this.constraintsLayer.paintShadowLayer(context, size, size, this.mapParams);
    this.constraintsLayer.paintColorLayer(context, size, size, this.mapParams);

    for (const key in this.vectors) {
      if (this.vectors[key].length > 0) {
        context.beginPath();
        for (const vector of this.vectors[key]) {
          this.drawVector(context, vector, parseInt(key));
        }
        context.stroke();
      }
    }
    this.paintFlsBeam(context, size);
    // reset line dash
    context.setLineDash(NO_DASHES);

    this.waypointLayer.paintShadowLayer(context, size, size, this.mapParams);
    this.waypointLayer.paintColorLayer(context, size, size, this.mapParams);

    this.fixInfoLayer.paintShadowLayer(context, size, size, this.mapParams);
    this.fixInfoLayer.paintColorLayer(context, size, size, this.mapParams);

    this.runwayLayer.paintShadowLayer(context, size, size, this.mapParams);
    this.runwayLayer.paintColorLayer(context, size, size, this.mapParams);

    this.pwpLayer.paintShadowLayer(context, size, size, this.mapParams);
    this.pwpLayer.paintColorLayer(context, size, size, this.mapParams);

    this.trafficLayer.paintShadowLayer(context, size, size);
    this.trafficLayer.paintColorLayer(context, size, size);
  }

  /** Shown with an NPA and LS on, to 80 NM range in ARC or PLAN and 160 NM in ROSE NAV (FCOM DSC-22-FMS). */
  private paintFlsBeam(context: CanvasRenderingContext2D, size: number) {
    const mode = this.mapMode.get();
    const radius = this.mapRangeRadius.get();
    const range = mode === EfisNdMode.ARC ? radius : radius * 2;
    const shown =
      ((mode === EfisNdMode.ARC || mode === EfisNdMode.PLAN) && range <= 80) ||
      (mode === EfisNdMode.ROSE_NAV && range <= 160);
    if (this.fls.beam < 1 || !this.fls.ls || !shown) {
      return;
    }
    const anchor = { lat: this.fls.lat, long: this.fls.long };
    const [ax, ay] = this.mapParams.coordinatesToXYy(anchor);
    const [ex, ey] = this.mapParams.coordinatesToXYy(placeBearingDistance(anchor, this.fls.course + 180, FLS_BEAM_NM));
    const [x, y] = [ax + size / 2, ay + size / 2];
    context.beginPath();
    context.strokeStyle = '#ff94ff';
    context.lineWidth = 1.75;
    context.setLineDash(FLS_DOTS);
    context.moveTo(x, y);
    context.lineTo(ex + size / 2, ey + size / 2);
    context.stroke();
    context.setLineDash(NO_DASHES);
    if (this.fls.pseudo >= 1) {
      context.beginPath();
      context.moveTo(x - 7, y);
      context.lineTo(x, y - 7);
      context.lineTo(x + 7, y);
      context.lineTo(x, y + 7);
      context.closePath();
      context.stroke();
      PaintUtils.paintText(true, context, x + 15, y + 17, SimVarString.unpack(this.fls.ident), '#ff94ff');
    }
  }

  private drawVector(context: CanvasRenderingContext2D, vector: PathVector, group: EfisVectorsGroup) {
    switch (group) {
      case EfisVectorsGroup.ACTIVE:
        context.strokeStyle = '#0f0';
        context.setLineDash(NO_DASHES);
        break;
      case EfisVectorsGroup.ACTIVE_EOSID:
        context.strokeStyle = '#ffff00';
        context.setLineDash(NO_DASHES);
        break;
      case EfisVectorsGroup.DASHED:
        context.strokeStyle = '#0f0';
        context.setLineDash(DASHES);
        break;
      case EfisVectorsGroup.TEMPORARY:
        context.strokeStyle = '#ffff00';
        context.setLineDash(DASHES);
        break;
      case EfisVectorsGroup.MISSED:
        context.strokeStyle = '#00ffff';
        context.setLineDash(NO_DASHES);
        break;
      case EfisVectorsGroup.ALTERNATE:
        context.strokeStyle = '#00ffff';
        context.setLineDash(DASHES);
        break;
      case EfisVectorsGroup.SECONDARY:
        context.strokeStyle = '#ffffff';
        context.setLineDash(NO_DASHES);
        break;
      default:
        context.strokeStyle = '#f00';
        context.setLineDash(NO_DASHES);
        break;
    }

    context.lineWidth = 1.75;

    const size = 768;

    switch (vector.type) {
      case 0: {
        const [sx, sy] = this.mapParams.coordinatesToXYy(vector.startPoint);
        const rsx = sx + size / 2;
        const rsy = sy + size / 2;

        const [ex, ey] = this.mapParams.coordinatesToXYy(vector.endPoint);
        const rex = ex + size / 2;
        const rey = ey + size / 2;

        context.moveTo(rsx, rsy);
        context.lineTo(rex, rey);
        break;
      }
      case 1: {
        const [sx, sy] = this.mapParams.coordinatesToXYy(vector.startPoint);
        const rsx = sx + size / 2;
        const rsy = sy + size / 2;

        const [ex, ey] = this.mapParams.coordinatesToXYy(vector.endPoint);
        const rex = ex + size / 2;
        const rey = ey + size / 2;

        const pathRadius = distanceTo(vector.centrePoint, vector.endPoint) * this.mapParams.nmToPx;

        // TODO find a way to batch that as well?
        // TODO beginPath needed here?
        context.stroke(
          new Path2D(
            `M ${rsx} ${rsy} A ${pathRadius} ${pathRadius} 0 ${Math.abs(vector.sweepAngle) >= 180 ? 1 : 0} ${vector.sweepAngle > 0 ? 1 : 0} ${rex} ${rey}`,
          ),
        );

        break;
      }
      default:
        break;
    }
  }

  render(): ViewNode | null {
    return (
      <>
        <canvas
          ref={this.canvasRef}
          width={768}
          height={768}
          class="nd-canvas-map"
          style={Derived.from(
            ([r, x, y]) => {
              return `width: ${r}px; height: ${r}px; position: absolute; top: 0; left: 0; transform: translate(${-(r / 2) + x}px, ${-(r / 2) + y}px)`;
            },
            Signal.of(768),
            this.props.x,
            this.props.y,
          )}
        />
        <div
          ref={this.touchContainerRef}
          style={Derived.from(
            ([r, x, y]) => {
              return `width: ${r}px; height: ${r}px; position: absolute; top: 0; left: 0; transform: translate(${-(r / 2) + x}px, ${-(r / 2) + y}px)`;
            },
            Signal.of(768),
            this.props.x,
            this.props.y,
          )}
        />
      </>
    );
  }
}

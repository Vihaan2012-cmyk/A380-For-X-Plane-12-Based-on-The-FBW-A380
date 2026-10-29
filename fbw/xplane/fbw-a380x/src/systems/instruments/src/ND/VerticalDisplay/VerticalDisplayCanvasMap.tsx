import {
  Arinc429LocalVarConsumerSubject,
  Arinc429WordData,
  ArincEventBus,
  EfisSide,
  FmsData,
  NdPwpSymbolTypeFlags,
  NdSymbolTypeFlags,
  VerticalPathCheckpoint,
} from '@flybywiresim/fbw-sdk';
import {
  Bits,
  ViewProps,
  BusSignal,
  View,
  UI,
  Derived,
  Readable,
  Bond,
  ViewNode,
} from 'xplane-js/ui';
import { DmcLogicEvents } from '../../MsfsAvionicsCommon/providers/DmcPublisher';
import { NDControlEvents } from '../NDControlEvents';
import { NDSimvars } from '../NDSimvarPublisher';
import {
  VD_FPA_TO_DISPLAY_ANGLE,
  VERTICAL_DISPLAY_CANVAS_HEIGHT,
  VERTICAL_DISPLAY_CANVAS_WIDTH,
  VerticalDisplay,
} from './VerticalDisplay';
import { VerticalDisplayWaypointLayer } from './VerticalDisplayWaypointLayer';
import { VdPseudoWaypointLayer } from './VdPseudoWaypointLayer';
import { VerticalDisplayRunwayLayer } from './VerticalDisplayRunwayLayer';
import { GenericFmsEvents } from '../../../../../../../fbw-common/src/systems/instruments/src/ND/types/GenericFmsEvents';
import { GenericFcuEvents } from '@flybywiresim/navigation-display';
import { FGVars } from '../../MsfsAvionicsCommon/providers/FGDataPublisher';
import { MfdSurvEvents } from '../../MsfsAvionicsCommon/providers/MfdSurvPublisher';

export interface VerticalDisplayCanvasMapProps extends ViewProps {
  bus: ArincEventBus;
  side: EfisSide;
  visible: Readable<'block' | 'none'>;
  vdAvailable: Readable<boolean>;
  fmsTargetVdProfile: Readable<VerticalPathCheckpoint[]>;
  vdRange: Readable<number>;
  verticalRange: Readable<[number, number]>;
  isSelectedVerticalMode: Readable<boolean>;
  shouldShowTrackLine: Readable<boolean>;
  selectedAltitude: Readable<number>;
  fpa: Readable<Arinc429WordData>;
}

export class VerticalDisplayCanvasMap extends View<VerticalDisplayCanvasMapProps> {
  private readonly subscriptions: Bond[] = [];

  private readonly sub = this.props.bus.getArincSubscriber<
    GenericFcuEvents &
      NDSimvars &
      DmcLogicEvents &
      NDControlEvents &
      GenericFmsEvents &
      FGVars &
      FmsData &
      MfdSurvEvents
  >();

  private readonly fmsActualVdProfile = BusSignal.from(this.sub.on('a32nx_fms_vertical_actual_profile'), []);
  private readonly fmsDescentVdProfile = BusSignal.from(this.sub.on('a32nx_fms_vertical_descent_profile'), []);
  private readonly fmsConstraints = BusSignal.from(this.sub.on('a32nx_fms_vertical_constraints'), []);

  private readonly fmsSymbols = BusSignal.from(this.sub.on(`vdSymbols_${this.props.side}`), []);

  private readonly pposLat = Arinc429LocalVarConsumerSubject.create(this.sub.on('latitude'), 0);
  private readonly pposLon = Arinc429LocalVarConsumerSubject.create(this.sub.on('longitude'), 0);

  private readonly baroCorrectedAltitude = Arinc429LocalVarConsumerSubject.create(
    this.sub.on('baroCorrectedAltitude'),
    0,
  );

  public readonly canvasInvalid = Derived.from(
    ([lat, long, baro]) => lat.isFailureWarning() || long.isFailureWarning() || baro.isFailureWarning(),
    this.pposLat,
    this.pposLon,
    this.baroCorrectedAltitude,
  );

  private readonly offsetDistance = this.props.fmsTargetVdProfile.derive((_path) => 0);

  private readonly activeVerticalMode = BusSignal.from(this.sub.on('fg.fma.verticalMode'), 0);

  private readonly mapRecomputing = BusSignal.from(this.sub.on('set_map_recomputing'), false);

  private readonly pathVisibility = Derived.from(
    ([mapVisible, vdAvailable, recomputing]) =>
      mapVisible === 'block' && vdAvailable && !recomputing ? 'visible' : 'hidden',
    this.props.visible,
    this.props.vdAvailable,
    this.mapRecomputing,
  );

  private readonly canvasRef = UI.ref<HTMLCanvasElement>();

  private readonly waypointLayer = new VerticalDisplayWaypointLayer();

  private readonly runwayLayer = new VerticalDisplayRunwayLayer();

  private readonly pwpLayer = new VdPseudoWaypointLayer();

  private handlePathFrame() {
    const canvas = this.canvasRef.target;
    const context = canvas.getContext('2d');

    if (!context) {
      return;
    }

    context.clearRect(0, 0, VERTICAL_DISPLAY_CANVAS_WIDTH, VERTICAL_DISPLAY_CANVAS_HEIGHT);
    context.resetTransform();

    if (
      this.pposLat.get().isFailureWarning() ||
      this.pposLon.get().isFailureWarning() ||
      this.baroCorrectedAltitude.get().isFailureWarning()
    ) {
      return;
    }

    const vdRange = this.props.vdRange.get();
    const verticalRange = this.props.verticalRange.get();

    context.strokeStyle = '#0f0';
    context.lineWidth = 2;

    const targetIsDashed = this.fmsActualVdProfile.get().length > 1;

    if (this.props.fmsTargetVdProfile.get().length > 0) {
      context.beginPath();
      if (targetIsDashed) {
        context.setLineDash([10, 10]);
      }

      context.moveTo(
        VerticalDisplayCanvasMap.distanceToX(
          this.props.fmsTargetVdProfile.get()[0].distanceFromAircraft,
          vdRange,
          this.offsetDistance.get(),
        ),
        VerticalDisplayCanvasMap.altToY(this.props.fmsTargetVdProfile.get()[0].altitude, verticalRange),
      );

      for (const pe of this.props.fmsTargetVdProfile.get()) {
        context.lineTo(
          VerticalDisplayCanvasMap.distanceToX(pe.distanceFromAircraft, vdRange, this.offsetDistance.get()),
          VerticalDisplayCanvasMap.altToY(pe.altitude, verticalRange),
        );
      }
      context.stroke();
    }

    // FIXME add descent profile when its display conditions are better understood

    if (targetIsDashed) {
      context.beginPath();
      context.setLineDash([]);
      context.moveTo(
        VerticalDisplayCanvasMap.distanceToX(
          this.fmsActualVdProfile.get()[0].distanceFromAircraft,
          vdRange,
          this.offsetDistance.get(),
        ),
        VerticalDisplayCanvasMap.altToY(this.fmsActualVdProfile.get()[0].altitude, verticalRange),
      );

      for (const pe of this.fmsActualVdProfile.get()) {
        context.lineTo(
          VerticalDisplayCanvasMap.distanceToX(pe.distanceFromAircraft, vdRange, this.offsetDistance.get()),
          VerticalDisplayCanvasMap.altToY(pe.altitude, verticalRange),
        );
      }

      const lastElement = this.fmsActualVdProfile.get()[this.fmsActualVdProfile.get().length - 1];
      context.lineTo(
        VerticalDisplayCanvasMap.distanceToX(540, vdRange, this.offsetDistance.get()),
        VerticalDisplayCanvasMap.altToY(lastElement.altitude, verticalRange),
      );
      context.stroke();
    }

    this.waypointLayer.paintShadowLayer(
      context,
      this.props.vdRange.get(),
      this.props.verticalRange.get(),
      this.offsetDistance.get(),
      this.props.isSelectedVerticalMode.get(),
    );
    this.waypointLayer.paintColorLayer(
      context,
      this.props.vdRange.get(),
      this.props.verticalRange.get(),
      this.offsetDistance.get(),
      this.props.isSelectedVerticalMode.get(),
    );

    this.runwayLayer.paintShadowLayer(
      context,
      this.props.vdRange.get(),
      this.props.verticalRange.get(),
      this.offsetDistance.get(),
    );
    this.runwayLayer.paintColorLayer(
      context,
      this.props.vdRange.get(),
      this.props.verticalRange.get(),
      this.offsetDistance.get(),
    );

    this.pwpLayer.paintShadowLayer(
      context,
      this.props.vdRange.get(),
      this.props.verticalRange.get(),
      this.offsetDistance.get(),
    );
    this.pwpLayer.paintColorLayer(
      context,
      this.props.vdRange.get(),
      this.props.verticalRange.get(),
      this.offsetDistance.get(),
    );
  }

  private handleNewSymbols() {
    const waypoints = this.fmsSymbols
      .get()
      .filter(
        (it) =>
          it.altConstraint ||
          (Bits.any(it.type, NdSymbolTypeFlags.Waypoint | NdSymbolTypeFlags.FlightPlan) &&
            !(it.type & NdSymbolTypeFlags.Runway)),
      );

    this.waypointLayer.data = waypoints;

    const runways = this.fmsSymbols.get().filter((it) => it.type & NdSymbolTypeFlags.Runway && it.location); // FIXME: Need to somehow include runways in the vertical path, or filter out runways before the active leg

    this.runwayLayer.data = runways;

    const pseudoWaypoints = this.fmsSymbols
      .get()
      .filter((it) => it.typePwp && it.typePwp & (NdPwpSymbolTypeFlags.PwpDecel | NdPwpSymbolTypeFlags.PwpSpeedChange));

    this.pwpLayer.data = pseudoWaypoints;
  }

  public static distanceToX(distance: number, vdRange: number, offsetDistance: number = 0) {
    return ((distance + offsetDistance) / vdRange) * 540;
  }

  public static altToY(alt: number, verticalRange: [number, number]) {
    return VerticalDisplay.altToY(alt, verticalRange) - 800;
  }

  public afterRender(node: ViewNode): void {
    super.afterRender(node);

    this.subscriptions.push(
      this.props.fmsTargetVdProfile.watch(() => this.handlePathFrame()),
      this.fmsDescentVdProfile.watch(() => this.handlePathFrame()),
      this.fmsActualVdProfile.watch(() => this.handlePathFrame()),
      this.fmsSymbols.watch(() => {
        this.handleNewSymbols();
        this.handlePathFrame();
      }),
    );

    this.subscriptions.push(
      this.fmsActualVdProfile,
      this.fmsDescentVdProfile,
      this.fmsConstraints,
      this.fmsSymbols,
      this.pposLat,
      this.pposLon,
      this.baroCorrectedAltitude,
      this.activeVerticalMode,
      this.mapRecomputing,
      this.pathVisibility,
    );
  }

  destroy(): void {
    for (const s of this.subscriptions) {
      s.destroy();
    }

    super.destroy();
  }

  render(): ViewNode {
    return (
      <canvas
        ref={this.canvasRef}
        width={VERTICAL_DISPLAY_CANVAS_WIDTH}
        height={VERTICAL_DISPLAY_CANVAS_HEIGHT}
        style={{
          width: `${VERTICAL_DISPLAY_CANVAS_WIDTH}px`,
          height: `${VERTICAL_DISPLAY_CANVAS_HEIGHT}px`,
          position: 'absolute',
          top: '800px',
          left: '150px',
          visibility: this.pathVisibility,
        }}
      />
    );
  }
}

// Copyright (c) 2021-2023 FlyByWire Simulations
//
// SPDX-License-Identifier: GPL-3.0

import {
  UI,
  ViewProps,
  Readable,
  ViewNode,
  Signal,
  Bus,
  BusSignal,
} from 'xplane-js/ui';

import { Arinc429Register, Arinc429WordData, EfisNdMode, GenericAdirsEvents } from '@flybywiresim/fbw-sdk';

import { PlanModeUnderlay } from './PlanModeUnderlay';
import { MapParameters } from '../../shared/utils/MapParameters';
import { NDPage } from '../NDPage';
import { NDControlEvents } from '../../NDControlEvents';
import { GenericFcuEvents } from '../../types/GenericFcuEvents';
import { GenericFmsEvents } from '../../types/GenericFmsEvents';

export interface PlanModePageProps<T extends number> extends ViewProps {
  bus: Bus;
  rangeValues: T[];
  aircraftTrueHeading: Readable<Arinc429WordData>;
}

export class PlanModePage<T extends number> extends NDPage<PlanModePageProps<T>> {
  public isVisible = Signal.of(false);

  private readonly controlPublisher = this.props.bus.outbox<NDControlEvents>();

  private readonly subs = this.props.bus.inbox<NDControlEvents & GenericAdirsEvents & GenericFmsEvents>();

  private readonly pposLatSub = BusSignal.from(this.subs.topic('latitude').distinct(), -1);

  private readonly pposLatRegister = Arinc429Register.empty();

  private readonly pposLongSub = BusSignal.from(this.subs.topic('longitude').distinct(), -1);

  private readonly pposLongRegister = Arinc429Register.empty();

  private readonly mapCenterLatSub = BusSignal.from(this.subs.topic('set_map_center_lat').distinct(), -1);

  private readonly mapCenterLonSub = BusSignal.from(this.subs.topic('set_map_center_lon').distinct(), -1);

  private readonly mapRangeRadiusSub = BusSignal.from(this.subs.topic('set_map_range_radius').distinct(), -1);

  private readonly selectedWaypointLatRegister = Arinc429Register.empty();

  private readonly selectedWaypointLatSub = BusSignal.from(this.subs.topic('mrpLat').distinct(), -1);

  private readonly selectedWaypointLongRegister = Arinc429Register.empty();

  private readonly selectedWaypointLongSub = BusSignal.from(this.subs.topic('mrpLong').distinct(), -1);

  private readonly mapRangeSub = BusSignal.from(
    this.props.bus.inbox<GenericFcuEvents>().topic('ndRangeSetting').distinct(),
    -1,
  );

  private readonly mapParams = new MapParameters();

  onShow() {
    this.handleSelectedWaypointPos();
    this.handleMovePlane();
    this.handleRotatePlane();
    this.handleScaleMap();
    this.handlePlaneVisibility();
  }

  afterRender(node: ViewNode) {
    super.afterRender(node);

    this.props.aircraftTrueHeading.watch(() => {
      this.handlePlaneVisibility();
      this.handleRotatePlane();
    });

    this.pposLatSub.watch((lat) => {
      this.pposLatRegister.set(lat);

      this.handlePlaneVisibility();
      this.handleMovePlane();
    });

    this.pposLongSub.watch((long) => {
      this.pposLongRegister.set(long);

      this.handlePlaneVisibility();
      this.handleMovePlane();
    });

    this.mapCenterLatSub.watch(() => {
      this.handleRecomputeMapParameters();
      this.handleMovePlane();
    });

    this.mapCenterLonSub.watch(() => {
      this.handleRecomputeMapParameters();
      this.handleMovePlane();
    });

    this.mapRangeRadiusSub.watch(() => {
      this.handleRecomputeMapParameters();
      this.handleMovePlane();
    });

    this.mapRangeSub.watch(() => this.handleScaleMap());

    this.selectedWaypointLatSub.watch(() => this.handleSelectedWaypointPos());
    this.selectedWaypointLongSub.watch(() => this.handleSelectedWaypointPos());
  }

  private handleMovePlane() {
    if (this.isVisible.get()) {
      const latRegister = this.pposLatRegister;
      const longRegister = this.pposLongRegister;

      if (latRegister.isNormalOperation() && longRegister.isNormalOperation()) {
        const lat = latRegister.value;
        const long = longRegister.value;

        const [x, y] = this.mapParams.coordinatesToXYy({ lat, long });

        this.controlPublisher.post('set_plane_x', 768 / 2 + x);
        this.controlPublisher.post('set_plane_y', 768 / 2 + y);
      }
    }
  }

  private handleRotatePlane() {
    if (this.isVisible.get()) {
      const planeRotation = this.props.aircraftTrueHeading.get();

      if (planeRotation.isNormalOperation()) {
        const ndRotation = this.mapParams.rotation(planeRotation.value);
        this.controlPublisher.post('set_plane_rotation', ndRotation);
      }
    }
  }

  private handlePlaneVisibility() {
    if (this.isVisible.get()) {
      const planeRotation = this.props.aircraftTrueHeading.get();
      if (
        planeRotation.isNormalOperation() &&
        this.pposLongRegister.isNormalOperation() &&
        this.pposLatRegister.isNormalOperation()
      ) {
        this.controlPublisher.post('set_show_plane', true);
      } else {
        this.controlPublisher.post('set_show_plane', false);
      }
    }
  }

  private handleScaleMap() {
    if (this.isVisible.get()) {
      const rangeSetting = this.mapRangeSub.get();
      const range = this.props.rangeValues[rangeSetting];

      this.controlPublisher.post('set_map_efis_mode', EfisNdMode.PLAN);
      this.controlPublisher.post('set_map_pixel_radius', 250);
      this.controlPublisher.post('set_map_range_radius', range / 2);
      this.controlPublisher.post('set_map_center_y_bias', 0);
    }
  }

  private handleSelectedWaypointPos() {
    this.selectedWaypointLatRegister.set(this.selectedWaypointLatSub.get());
    this.selectedWaypointLongRegister.set(this.selectedWaypointLongSub.get());

    // FIXME what to do when the selected waypoint is not valid?
    if (
      this.isVisible.get() &&
      this.selectedWaypointLatRegister.isNormalOperation() &&
      this.selectedWaypointLongRegister.isNormalOperation()
    ) {
      this.controlPublisher.post('set_show_map', true);
      this.controlPublisher.post('set_map_center_lat', this.selectedWaypointLatRegister.value);
      this.controlPublisher.post('set_map_center_lon', this.selectedWaypointLongRegister.value);
      this.controlPublisher.post('set_map_up_course', 0);
    }
  }

  private handleRecomputeMapParameters() {
    this.mapParams.compute(
      { lat: this.mapCenterLatSub.get(), long: this.mapCenterLonSub.get() },
      0,
      this.mapRangeRadiusSub.get(),
      250,
      0, // FIXME true north ?
    );
  }

  render(): ViewNode | null {
    return (
      <g visibility={this.isVisible.derive((visible) => (visible ? 'visible' : 'hidden'))}>
        <PlanModeUnderlay mapRange={this.mapRangeRadiusSub} />
      </g>
    );
  }
}

// @ts-strict-ignore
// Copyright (c) 2021-2024 FlyByWire Simulations
//
// SPDX-License-Identifier: GPL-3.0

import {
  UI,
  View,
  ViewNode,
  Signal,
  Bus,
  Derived,
  Readable,
} from 'xplane-js/ui';
import { EfisNdMode, Arinc429ConsumerSubject, GenericAdirsEvents } from '@flybywiresim/fbw-sdk';

import { NDControlEvents } from '../NDControlEvents';
import { LubberLine } from '../pages/arc/LubberLine';
import { GenericDisplayManagementEvents } from '../types/GenericDisplayManagementEvents';
import { Layer } from '../../MsfsAvionicsCommon/Layer';

const PLANE_X_OFFSET = -41;
const PLANE_Y_OFFSET = 0;

export class Airplane extends View<{ bus: Bus; ndMode: Readable<EfisNdMode> }> {
  private readonly headingWord = Arinc429ConsumerSubject.create(null);

  private readonly headingWordValid = this.headingWord.derive((it) => it.isNormalOperation());

  private readonly showPlane = Signal.of(false);

  private readonly planeVisibility = Derived.from(
    ([headingValid, showPlane]) => headingValid && showPlane,
    this.headingWordValid,
    this.showPlane,
  );

  private readonly lubberVisibility = Derived.from(
    ([planeVisibility, ndMode]) => ndMode !== EfisNdMode.PLAN && planeVisibility,
    this.planeVisibility,
    this.props.ndMode,
  );

  private readonly circleVisibility = Derived.from(
    ([headingValid, showPlane]) => !headingValid && showPlane,
    this.headingWordValid,
    this.showPlane,
  );

  private readonly x = Signal.of(0);

  private readonly y = Signal.of(0);

  private readonly rotation = Signal.of(0);

  afterRender(node: ViewNode) {
    super.afterRender(node);

    const sub = this.props.bus.inbox<GenericAdirsEvents & GenericDisplayManagementEvents & NDControlEvents>();

    this.headingWord.setConsumer(sub.topic('heading'));

    sub.topic('set_show_plane').listen((show) => {
      this.showPlane.set(show);
    });

    sub.topic('set_plane_x').listen((x) => {
      this.x.set(x);
    });

    sub.topic('set_plane_y').listen((y) => {
      this.y.set(y);
    });

    sub.topic('set_plane_rotation').listen((rotation) => {
      this.rotation.set(rotation);
    });
  }

  render(): ViewNode | null {
    return (
      <>
        <Layer x={this.x.derive((x) => x + PLANE_X_OFFSET)} y={this.y.derive((y) => y + PLANE_Y_OFFSET)}>
          <g
            visibility={this.planeVisibility.derive((it) => (it ? 'inherit' : 'hidden'))}
            transform={this.rotation.derive((rotation) => `rotate(${rotation} ${-PLANE_X_OFFSET} 0)`)}
          >
            <path
              class="shadow"
              stroke-width={8}
              stroke-linecap="round"
              d="M 0, 0 h 82 m -41, -29.5 v 70.25 m -11.5, -9.75 h 23.5"
            />
            <path
              class="Yellow"
              stroke-width={5}
              stroke-linecap="round"
              d="M 0, 0 h 82 m -41, -29.5 v 70.25 m -11.5, -9.75 h 23.5"
            />
          </g>

          <circle
            class="Red"
            stroke-width={2}
            visibility={this.circleVisibility.derive((it) => (it ? 'inherit' : 'hidden'))}
            cx={-PLANE_X_OFFSET}
            cy={-PLANE_Y_OFFSET}
            r={9}
          />
        </Layer>

        <LubberLine
          bus={this.props.bus}
          visible={this.lubberVisibility}
          rotation={this.rotation}
          ndMode={this.props.ndMode}
        />
      </>
    );
  }
}

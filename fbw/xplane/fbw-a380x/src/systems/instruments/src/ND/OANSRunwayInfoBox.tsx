// Copyright (c) 2024 FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0

import './oans-style.scss';

import { ControlPanelMapDataSearchMode } from '@flybywiresim/oanc';
import { View, UI, Readable, ViewNode } from 'xplane-js/ui';

interface OansRunwayInfoBoxProps {
  rwyOrStand: Readable<ControlPanelMapDataSearchMode | null>;
  selectedEntity: Readable<string | null>;
  tora: Readable<string>;
  lda: Readable<string>;
  ldaIsReduced: Readable<boolean>;
  coordinate: Readable<string>;
  lengthUnit: Readable<string>;
}
export class OansRunwayInfoBox extends View<OansRunwayInfoBoxProps> {
  private rwyDivRef = UI.ref<HTMLDivElement>();

  private standDivRef = UI.ref<HTMLDivElement>();

  private setDivs() {
    this.rwyDivRef.target.style.display = 'none';
    this.standDivRef.target.style.display = 'none';

    if (this.props.rwyOrStand.get() === ControlPanelMapDataSearchMode.Runway && this.props.selectedEntity.get()) {
      this.rwyDivRef.target.style.display = 'grid';
      this.standDivRef.target.style.display = 'none';
    } else if (this.props.rwyOrStand.get() === ControlPanelMapDataSearchMode.Stand && this.props.selectedEntity.get()) {
      this.rwyDivRef.target.style.display = 'none';
      this.standDivRef.target.style.display = 'flex';
    } else {
      this.rwyDivRef.target.style.display = 'none';
      this.standDivRef.target.style.display = 'none';
    }
  }

  afterRender(node: ViewNode): void {
    super.afterRender(node);

    this.setDivs();

    this.props.rwyOrStand.watch(() => this.setDivs());
    this.props.selectedEntity.watch(() => this.setDivs());
  }

  render(): ViewNode {
    return (
      <>
        <div
          ref={this.rwyDivRef}
          class="oans-info-box"
          style="display: none; grid-template-columns: 2fr 1fr 1fr; width: 75%; margin-top: 20px; align-self: center;"
        >
          <div>
            <span class="mfd-label">RWY: </span>
            <span class="mfd-value smaller" style="text-align: left;">
              {this.props.selectedEntity}
            </span>
          </div>
          <span class="mfd-label" style="text-align: right; margin-right: 15px;">
            TORA:{' '}
          </span>
          <span class="mfd-value smaller">
            {this.props.tora}
            <span style="color: rgb(33, 33, 255)">{this.props.lengthUnit}</span>
          </span>
          <span
            class="mfd-label"
            style="grid-column: span 2; text-align: right; margin-right: 15px;"
          >{`${this.props.ldaIsReduced.get() ? 'REDUCED ' : ''}LDA: `}</span>
          <span class="mfd-value smaller" style={this.props.ldaIsReduced.get() ? 'color: cyan;' : ''}>
            {this.props.lda}
            <span style="color: rgb(33, 33, 255)">{this.props.lengthUnit}</span>
          </span>
        </div>
        <div
          ref={this.standDivRef}
          class="oans-info-box"
          style="display: none; flex-direction: column; width: 75%; margin: 10px; align-self: center; align-items: center;"
        >
          <span class="mfd-label">
            STAND:
            <span style="color: #00ff00">{this.props.selectedEntity}</span>
          </span>
          <span class="mfd-label" style="align-self: flex-end; color: #00ff00">
            {this.props.coordinate}
          </span>
        </div>
      </>
    );
  }
}

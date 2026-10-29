// Copyright (c) 2021-2024 FlyByWire Simulations
//
// SPDX-License-Identifier: GPL-3.0

import { Arinc429LocalVarConsumerSubject, MathUtils } from '@flybywiresim/fbw-sdk';
import {
  BusSignal,
  View,
  Bus,
  UI,
  Derived,
  Readable,
  Logic,
  Bond,
  ViewNode,
} from 'xplane-js/ui';
import { PFDSimvars } from './shared/PFDSimvarPublisher';
import PitchTrimUtils from '@shared/PitchTrimUtils';
import { FqmsBusEvents } from '@shared/publishers/FqmsBusPublisher';

enum PitchTrimStatus {
  AfterLanding,
  OutOfRange,
  AtTarget,
  NotAtTarget,
}

const TRIM_WHEEL_HEIGHT = 600;
const TRIM_POS_TO_WHEEL_GAIN = 140;
const SVG_HEIGHT = 252;

export class PitchTrimDisplay extends View<{ bus: Bus; visible: Readable<boolean> }> {
  private readonly subscriptions: Bond[] = [];
  private readonly sub = this.props.bus.inbox<PFDSimvars & FqmsBusEvents>();

  private readonly cgGroup = UI.ref<SVGGElement>();

  private readonly cgValue = UI.ref<SVGTextElement>();

  private readonly outOfRangeGroup = UI.ref<SVGGElement>();

  private readonly arrowRef = UI.ref<SVGPolygonElement>();

  private readonly pitchTrimVisibility = this.props.visible.derive((it) => (it ? 'visible' : 'hidden'));

  private readonly pitchTrimVisibilitySub = this.props.visible.watch((v) => {
    if (v) {
      this.subscriptions.forEach((s) => (v ? s.release(true) : s.hold()));
    }
  });

  private readonly trimPosition = BusSignal.from(this.sub.topic('trimPosition').distinct(), 0).derive(
    (it) => it * MathUtils.RADIANS_TO_DEGREES,
  );
  private readonly thsForText = this.trimPosition.derive((it) =>
    (Math.round(PitchTrimUtils.pitchTrimToCg(it) * 10) / 10).toFixed(1),
  );

  // Use modulo TRIM_WHEEL_HEIGHT / 4 to wrap around, otherwise the trim indication would not be long enough.
  private readonly rightWheelY = this.trimPosition.derive(
    (it) => ((it * TRIM_POS_TO_WHEEL_GAIN) % (TRIM_WHEEL_HEIGHT / 4)) - TRIM_WHEEL_HEIGHT / 2 + SVG_HEIGHT / 2,
  );

  private readonly trimAreasTransform = this.trimPosition.derive(
    (it) => `translate(276 ${-(this.degreesToPixel(it) - 103.5) + 23})`,
  );

  private readonly fwcFlightPhase = BusSignal.from(this.sub.topic('fwcFlightPhase'), 0);

  private readonly groundSpeed = Arinc429LocalVarConsumerSubject.create(this.sub.topic('groundSpeed'), 0);

  private readonly maintenancePositionWhiteLineVisibility = Derived.from(
    ([fp, gs]) => (fp >= 11 && gs.isNormalOperation() && gs.value <= 30 ? 'inherit' : 'hidden'),
    this.fwcFlightPhase,
    this.groundSpeed,
  );

  private readonly engOneRunning = BusSignal.from(this.sub.topic('engOneRunning'), false);
  private readonly engTwoRunning = BusSignal.from(this.sub.topic('engTwoRunning'), false);
  private readonly engThreeRunning = BusSignal.from(this.sub.topic('engThreeRunning'), false);
  private readonly engFourRunning = BusSignal.from(this.sub.topic('engFourRunning'), false);
  private readonly anyEngRunning = Derived.from(
    Logic.or(),
    this.engOneRunning,
    this.engTwoRunning,
    this.engThreeRunning,
    this.engFourRunning,
  );

  // TODO: check what happens when CG is not available
  // TODO: take WBBC CG if FQMS unavailable
  private readonly cgPercent = Arinc429LocalVarConsumerSubject.create(this.sub.topic('fqms_center_of_gravity_mac'));
  private readonly cgPercentText = this.cgPercent.derive((cg) => (cg.isNormalOperation() ? cg.value.toFixed(1) : ''));

  private readonly greenHydPressurized = BusSignal.from(this.sub.topic('hydGreenSysPressurized'), false);
  private readonly yellowHydPressurized = BusSignal.from(this.sub.topic('hydYellowSysPressurized'), false);
  private readonly oneHydPressurized = Derived.from(
    Logic.or(),
    this.greenHydPressurized,
    this.yellowHydPressurized,
  );

  private readonly cgStatus = Derived.from(
    ([trim, cg, phase, engRunning]) => {
      if (!engRunning || phase > 9) {
        return PitchTrimStatus.AfterLanding;
      } else {
        if (PitchTrimUtils.pitchTrimOutOfRange(trim)) {
          return PitchTrimStatus.OutOfRange;
        } else {
          return cg.isNormalOperation() && PitchTrimUtils.pitchTrimInCyanBand(cg.value, trim)
            ? PitchTrimStatus.AtTarget
            : PitchTrimStatus.NotAtTarget;
        }
      }
    },
    this.trimPosition,
    this.cgPercent,
    this.fwcFlightPhase,
    this.anyEngRunning,
  );

  private readonly optimalPitchTrim = this.cgPercent.derive((cg) =>
    cg.isNormalOperation() ? PitchTrimUtils.cgToPitchTrim(cg.value) : null,
  );

  degreesToPixel(deg: number) {
    return (10 - deg) * 17.25 + 103.5;
  }

  /** in pixels, where magenta GW CG box is */
  private readonly gwCgPositionTransform = Derived.from(
    ([pitch, optimal]) => (optimal === null ? '' : `translate(0 ${(pitch - optimal) * 17.25 - 5})`),
    this.trimPosition,
    this.optimalPitchTrim,
  );

  private readonly gwCgVisibility = Derived.from(
    ([flightPhase, hydPress, engRunning, cg]) =>
      cg.isNormalOperation() && engRunning && flightPhase <= 10 && hydPress ? 'inherit' : 'hidden',
    this.fwcFlightPhase,
    this.oneHydPressurized,
    this.anyEngRunning,
    this.cgPercent,
  );

  /** in pixels, where upper magenta box starts */
  private readonly optimalPitchTrimUpperBoxStart = this.optimalPitchTrim.derive((it) =>
    this.degreesToPixel(Math.min((it ?? 0) + 1.5, 5.8)),
  );

  /** in pixels, where middle of magenta boxes is */
  private readonly optimalPitchTrimCenter = this.optimalPitchTrim.derive((it) => this.degreesToPixel(it ?? 0));

  private readonly optimalPitchTrimUpperBoxHeight = Derived.from(
    ([start, center]) => center - start,
    this.optimalPitchTrimUpperBoxStart,
    this.optimalPitchTrimCenter,
  );

  /** in pixels, where lower magenta box ends */
  private readonly optimalPitchTrimLowerBoxEnd = this.optimalPitchTrim.derive((it) =>
    this.degreesToPixel(Math.max((it ?? 0) - 1.5, -0.2)),
  );

  private readonly optimalPitchTrimLowerBoxHeight = Derived.from(
    ([end, center]) => end - center,
    this.optimalPitchTrimLowerBoxEnd,
    this.optimalPitchTrimCenter,
  );

  updateCgStatus() {
    this.cgValue.target.classList.remove(...this.cgValue.target.classList);
    this.arrowRef.target.classList.remove(...this.cgValue.target.classList);

    switch (this.cgStatus.get()) {
      case PitchTrimStatus.AfterLanding:
        this.cgGroup.target.style.visibility = 'inherit';
        this.outOfRangeGroup.target.style.visibility = 'hidden';
        this.cgValue.target.classList.add('White');
        this.arrowRef.target.style.fill = 'white';
        break;
      case PitchTrimStatus.OutOfRange:
        this.cgGroup.target.style.visibility = 'hidden';
        this.outOfRangeGroup.target.style.visibility = 'inherit';
        this.arrowRef.target.style.fill = 'red';
        break;
      case PitchTrimStatus.AtTarget:
        this.cgGroup.target.style.visibility = 'inherit';
        this.outOfRangeGroup.target.style.visibility = 'hidden';
        this.cgValue.target.classList.add('Green');
        this.arrowRef.target.style.fill = '#00ff00';
        break;
      case PitchTrimStatus.NotAtTarget:
        this.cgGroup.target.style.visibility = 'inherit';
        this.outOfRangeGroup.target.style.visibility = 'hidden';
        this.cgValue.target.classList.add('Amber');
        this.arrowRef.target.style.fill = '#e68000';
        break;
    }
  }

  afterRender(node: ViewNode): void {
    super.afterRender(node);

    this.subscriptions.push(this.cgStatus.watch(() => this.updateCgStatus(), true));

    this.subscriptions.push(
      this.pitchTrimVisibility,
      this.trimPosition,
      this.thsForText,
      this.rightWheelY,
      this.trimAreasTransform,
      this.fwcFlightPhase,
      this.maintenancePositionWhiteLineVisibility,
      this.anyEngRunning,
      this.cgPercent,
      this.cgPercentText,
      this.greenHydPressurized,
      this.yellowHydPressurized,
      this.oneHydPressurized,
      this.cgStatus,
      this.optimalPitchTrim,
      this.gwCgPositionTransform,
      this.gwCgVisibility,
      this.optimalPitchTrimCenter,
      this.optimalPitchTrimLowerBoxEnd,
      this.optimalPitchTrimLowerBoxHeight,
      this.optimalPitchTrimUpperBoxHeight,
      this.optimalPitchTrimUpperBoxStart,
    );
  }

  destroy(): void {
    for (const s of this.subscriptions) {
      s.destroy();
    }

    this.pitchTrimVisibilitySub.destroy();

    super.destroy();
  }

  render(): ViewNode {
    return (
      <div style={{ visibility: this.pitchTrimVisibility }}>
        <img src="/Images/fbw-a380x/TRIM_INDICATOR.png" class="TrimIndicatorImage" />
        <svg class="TrimIndicatorSvg" xmlns="http://www.w3.org/2000/svg">
          <defs>
            <linearGradient id="rightWheelGradient" x1="0%" x2="0%" y1="0%" y2="100%">
              {Array(16)
                .fill(1)
                .map((_, index) => {
                  return (
                    <>
                      <stop offset={`${index * 6.25 + 0.001}%`} stop-color="#555f72" />
                      <stop offset={`${index * 6.25 + 2.6}%`} stop-color="#555f72" />
                      <stop offset={`${index * 6.25 + 2.6001}%`} stop-color="white" />
                      <stop offset={`${(index + 1) * 6.25 + 0.001}%`} stop-color="white" />
                    </>
                  );
                })}
            </linearGradient>
            <linearGradient id="shadowGradient" x1="0%" x2="0%" y1="0%" y2="100%">
              <stop offset="0%" stop-color="black" stop-opacity="1" />
              <stop offset="10%" stop-color="black" stop-opacity="0" />
              <stop offset="90%" stop-color="black" stop-opacity="0" />
              <stop offset="100%" stop-color="black" stop-opacity="1" />
            </linearGradient>
            <linearGradient id="markerGradient1" x1="0%" x2="0%" y1="0%" y2="100%">
              <stop offset="0%" stop-color="#8fadc0" />
              <stop offset="90%" stop-color="#6387a1" />
            </linearGradient>
            <linearGradient id="markerGradient2" x1="0%" x2="0%" y1="0%" y2="100%">
              <stop offset="0%" stop-color="#8fadc0" />
              <stop offset="60%" stop-color="#444444" />
              <stop offset="100%" stop-color="#8fadc0" />
            </linearGradient>
            <clipPath id="cut-right">
              <rect x="319" y="23" width="16" height="207" />
            </clipPath>
            <clipPath id="cut-left">
              <rect x="276" y="23" width="30" height="207" />
            </clipPath>
          </defs>
          <g>
            <text x={2} y={138} font-size={21.7} class="White">
              T.O THS
            </text>
            <g ref={this.cgGroup}>
              <text x={110} y={138} font-size={21.7} class="White">
                FOR
              </text>
              <text x={162} y={140} font-size={26.6} class="White" ref={this.cgValue}>
                {this.thsForText}
              </text>
              <text x={232} y={140} font-size={21.7} class="Cyan">
                %
              </text>
            </g>
            <g ref={this.outOfRangeGroup}>
              <text x={125} y={130} font-size={26.6} class="Red">
                OUT OF
              </text>
              <text x={130} y={160} font-size={26.6} class="Red">
                RANGE
              </text>
            </g>
            <g transform={this.gwCgPositionTransform} visibility={this.gwCgVisibility}>
              <rect x={360} y={117} width={71} height={30} stroke="#ff94ff" stroke-width={2} />
              <text x={362} y={140} font-size={21.7} class="Magenta">
                {this.cgPercentText}
              </text>
              <text x={416} y={140} font-size={21.7} class="Cyan">
                %
              </text>
            </g>
          </g>
          <g clip-path="url(#cut-right)">
            <rect
              width="14"
              height={TRIM_WHEEL_HEIGHT}
              x="320"
              y={this.rightWheelY}
              stroke="black"
              fill="url(#rightWheelGradient)"
            />
            <rect x="319" y="23" width="16" height="207" fill="url(#shadowGradient)" />
          </g>
          <g clip-path="url(#cut-left)">
            <g transform={this.trimAreasTransform}>
              <rect width="28" height="414" x="0" y="0" stroke="black" fill="#555f72" />
              <rect width="25" height="207" x="2" y="103.5" fill="none" stroke="white" stroke-width="2" />
              <rect width="23" height="103.5" x="3" y="175.95" fill="none" stroke="#00ff00" stroke-width="4" />
              <rect
                width="16"
                height={this.optimalPitchTrimUpperBoxHeight}
                x="6"
                y={this.optimalPitchTrimUpperBoxStart}
                fill="none"
                stroke="#ff94ff"
                stroke-width="4"
                visibility={this.gwCgVisibility}
              />
              <rect
                width="16"
                height={this.optimalPitchTrimLowerBoxHeight}
                x="6"
                y={this.optimalPitchTrimCenter}
                fill="none"
                stroke="#ff94ff"
                stroke-width="4"
                visibility={this.gwCgVisibility}
              />
              <rect
                width="20"
                height="4"
                x="5"
                y="274"
                fill="white"
                visibility={this.maintenancePositionWhiteLineVisibility}
              />
            </g>
            <rect x="275" y="23" width="30" height="207" fill="url(#shadowGradient)" />
          </g>
          <polygon points="265,115 265,138 277,126.5" ref={this.arrowRef} />
          <rect x="277" y="118" width="28" height="4" fill="url(#markerGradient1)" />
          <rect x="277" y="130" width="28" height="4" fill="url(#markerGradient1)" />
          <rect x="318" y="113" width="2" height="25" fill="url(#markerGradient2)" />
          <rect x="334" y="113" width="2" height="25" fill="url(#markerGradient2)" />
        </svg>
      </div>
    );
  }
}

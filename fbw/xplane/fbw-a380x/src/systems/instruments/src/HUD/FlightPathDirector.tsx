import {
  BusSignal,
  View,
  Bus,
  UI,
  Derived,
  Readable,
  ViewNode,
} from 'xplane-js/ui';
import { Arinc429ConsumerSubject, Arinc429LocalVarConsumerSubject } from '@flybywiresim/fbw-sdk';

import { calculateHorizonOffsetFromPitch, HudElems } from './HUDUtils';
import { Arinc429Values } from './shared/ArincValueProvider';
import { HUDSimvars } from './shared/HUDSimvarPublisher';
import { ONE_DEG, FIVE_DEG, OutlinedPath } from './HUDUtils';
import { SelectedFdEvents } from './shared/FdSelectionProvider';
import { PrimFgBusBaseEvents } from '@shared/publishers/PrimFgPublisher';
import { FlashOneHertz } from '../MsfsAvionicsCommon/FlashingElementUtils';
const DistanceSpacing = FIVE_DEG;
const ValueSpacing = 5;

export class FlightPathDirector extends View<{ bus: Bus; isAttExcessive: Readable<boolean> }> {
  private readonly sub = this.props.bus.inbox<
    Arinc429Values & SelectedFdEvents & PrimFgBusBaseEvents & HUDSimvars & HudElems
  >();

  private readonly fdRollCommand = Arinc429LocalVarConsumerSubject.create(this.sub.topic('prim_roll_fd_command'));

  private readonly fdPitchCommand = Arinc429LocalVarConsumerSubject.create(this.sub.topic('prim_pitch_fd_command'));

  private readonly fdYawCommand = Arinc429LocalVarConsumerSubject.create(this.sub.topic('prim_yaw_fd_command'));

  private readonly fdActive = BusSignal.from(this.sub.topic('fd_engaged'), false);

  private readonly leftMainGearCompressed = BusSignal.from(this.sub.topic('leftMainGearCompressed'), false);

  private readonly rightMainGearCompressed = BusSignal.from(this.sub.topic('leftMainGearCompressed'), false);
  private readonly fwcFlightPhase = BusSignal.from(this.sub.topic('fwcFlightPhase'), 0);
  private readonly crosswindMode = BusSignal.from(this.sub.topic('cWndMode'), false);

  private readonly roll = Arinc429ConsumerSubject.create(this.sub.topic('rollAr'));
  private readonly pitch = Arinc429ConsumerSubject.create(this.sub.topic('pitchAr'));
  private readonly fpa = Arinc429ConsumerSubject.create(this.sub.topic('fpa'));
  private readonly da = Arinc429ConsumerSubject.create(this.sub.topic('da'));

  private flightPhase = 0;

  private needsUpdate = false;

  private birdPath = UI.ref<SVGGElement>();

  private birdPathCircle = UI.ref<SVGPathElement>();
  private birdPathCircleBg = UI.ref<SVGPathElement>();

  private readonly onGround = Derived.from(
    ([leftMainGearCompressed, rightMainGearCompressed]) => leftMainGearCompressed || rightMainGearCompressed,
    this.leftMainGearCompressed,
    this.rightMainGearCompressed,
  );

  private readonly fdVisible = Derived.from(
    ([fdRollCommand, fdPitchCommand, fwcFlightPhase]) => {
      if (fwcFlightPhase < 4 || fwcFlightPhase >= 10) {
        return 'none';
      } else {
        if (
          !(fdRollCommand.isNoComputedData() || fdRollCommand.isFailureWarning()) &&
          !(fdPitchCommand.isNoComputedData() || fdPitchCommand.isFailureWarning())
        ) {
          return 'block';
        } else {
          return 'none';
        }
      }
    },
    this.fdRollCommand,
    this.fdPitchCommand,
    this.fwcFlightPhase,
  );

  private readonly fdFlagVisible = Derived.from(
    ([fdActive, fdPitchCommand, fdRollCommand, fdYawCommand, onGround]) =>
      fdActive &&
      (fdRollCommand.isFailureWarning() ||
        fdPitchCommand.isFailureWarning() ||
        (fdYawCommand.isFailureWarning() && onGround)),
    this.fdActive,
    this.fdPitchCommand,
    this.fdRollCommand,
    this.fdYawCommand,
    this.onGround,
  );

  private readonly fdTransform = Derived.from(
    ([roll, pitch, fpa, da, fdRollCommand, fdPitchCommand, crosswindMode]) => {
      let xOffsetLim;

      const daLimConv = (da.value * DistanceSpacing) / ValueSpacing;
      const pitchSubFpaConv = calculateHorizonOffsetFromPitch(pitch.value) - calculateHorizonOffsetFromPitch(fpa.value);
      const rollCos = Math.cos((roll.value * Math.PI) / 180);
      const rollSin = Math.sin((-roll.value * Math.PI) / 180);

      const FDRollOrder = fdRollCommand.value;
      const FDRollOrderLim = Math.max(Math.min(FDRollOrder, 45), -45);
      const FDPitchOrder = fdPitchCommand.value; //in degrees on pitch scale
      const FDPitchOrderLim = Math.max(Math.min(FDPitchOrder, 45), -45);

      const xOffsetFpv = daLimConv * rollCos - pitchSubFpaConv * rollSin;
      const yOffsetFpv = pitchSubFpaConv * rollCos + daLimConv * rollSin;

      const xOffset = xOffsetFpv + FDRollOrderLim * 13;
      const yOffset = yOffsetFpv + FDPitchOrderLim * 13 + rollSin * (xOffset - xOffsetFpv); // * rollCos;

      //set lateral limit for fdCue
      if (crosswindMode == false) {
        xOffsetLim = Math.max(Math.min(xOffset, 350), -378);
      } else {
        xOffsetLim = Math.max(Math.min(xOffset, 540), -540);
      }

      return `translate3d(${xOffsetLim}px, ${yOffset - FIVE_DEG}px, 0px)`;
    },
    this.roll,
    this.pitch,
    this.fpa,
    this.da,
    this.fdRollCommand,
    this.fdPitchCommand,
    this.crosswindMode,
  );

  private readonly fdCueOffRange = Derived.from(
    ([roll, pitch, fpa, da, fdRollCommand, crosswindMode]) => {
      let fdCueOffRange = false;
      const daLimConv = (da.value * DistanceSpacing) / ValueSpacing;
      const pitchSubFpaConv = calculateHorizonOffsetFromPitch(pitch.value) - calculateHorizonOffsetFromPitch(fpa.value);

      const FDRollOrder = fdRollCommand.value;
      const FDRollOrderLim = Math.max(Math.min(FDRollOrder, 45), -45);
      const rollCos = Math.cos((roll.value * Math.PI) / 180);
      const rollSin = Math.sin((-roll.value * Math.PI) / 180);
      const xOffsetFpv = daLimConv * rollCos - pitchSubFpaConv * rollSin;

      const xOffset = xOffsetFpv + FDRollOrderLim * 13;
      //set lateral limit for fdCue
      if (crosswindMode == false) {
        if (xOffset < -378 || xOffset > 350) {
          fdCueOffRange = true;
        } else {
          fdCueOffRange = false;
        }
      } else {
        if (xOffset < -540 || xOffset > 540) {
          fdCueOffRange = true;
        } else {
          fdCueOffRange = false;
        }
      }
      return fdCueOffRange ? '3 6' : '';
    },
    this.roll,
    this.pitch,
    this.fpa,
    this.da,
    this.fdRollCommand,
    this.crosswindMode,
  );

  render(): ViewNode {
    return (
      <>
        <g ref={this.birdPath}>
          <svg>
            <g id="FlightPathDirector" display={this.fdVisible}>
              <g style={{ transform: this.fdTransform }} stroke-dasharray={this.fdCueOffRange}>
                {OutlinedPath(
                  'M 640 512 m 7 0 a 7 7 0 1 0 -14 0 a 7 7 0 1 0 14 0 Z M 640 512 m 7 0 a 7 7 0 1 0 -14 0 a 7 7 0 1 0 14 0 Z',
                  'NormalStroke InverseGreen',
                  'NormalStroke Green',
                  this.birdPathCircleBg,
                  this.birdPathCircle,
                )}
              </g>
            </g>
          </svg>
        </g>

        <FlashOneHertz bus={this.props.bus} flashDuration={9} visible={this.fdFlagVisible}>
          <text id="FDFlag" x="265" y="280" class="FontLargest EndAlign Green">
            FD
          </text>
        </FlashOneHertz>
      </>
    );
  }
}

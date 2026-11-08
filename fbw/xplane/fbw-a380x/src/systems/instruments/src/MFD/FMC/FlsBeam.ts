// Copyright (c) 2026 FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0

import { Approach, ApproachType, ApproachWaypointDescriptor, ProcedureLeg, Runway } from '@flybywiresim/fbw-sdk';
import { bearingTo, Coordinates, diffAngle, distanceTo, placeBearingDistance } from 'msfs-geo';

/** The FMS Landing System's virtual beam (FCOM DSC-22-FMS FLS beam computation). */
export interface FlsBeam {
  anchor: Coordinates;
  /** Feet, the threshold crossing height above the threshold. */
  anchorAltitude: number;
  /** Degrees true, inbound. */
  course: number;
  /** Degrees, positive. */
  slope: number;
  ident: string;
  /** True for an end point the FMS computed, not a database point; the ND draws it magenta. */
  pseudo: boolean;
  /** False when the lateral guidance stays the localizer's: LOC and LOC B/C approaches. */
  lateral: boolean;
}

/** Approaches the FLS serves; IGS, SDF and LDA have none, ILS, GLS and MLS their own beams. */
const FLS_TYPES = new Set([
  ApproachType.Vor,
  ApproachType.VorDme,
  ApproachType.Vortac,
  ApproachType.Tacan,
  ApproachType.Ndb,
  ApproachType.NdbDme,
  ApproachType.Rnav,
  ApproachType.Gps,
  ApproachType.Fms,
  ApproachType.Loc,
  ApproachType.LocBackcourse,
]);

/** A non-precision approach without a beam gets NO FLS FOR THIS APPR. */
export function isNonPrecision(type: ApproachType): boolean {
  return FLS_TYPES.has(type) || type === ApproachType.Igs || type === ApproachType.Sdf || type === ApproachType.Lda;
}

/** A MAP within this of the threshold is on it. */
const ON_THRESHOLD_NM = 0.14;
/** An anchor this close to the threshold takes the threshold's ident. */
const THRESHOLD_IDENT_NM = 0.1;
/** No beam when the final course is further than this off the runway's. */
const MAX_COURSE_OFFSET_DEG = 50;

function location(leg: ProcedureLeg | undefined): Coordinates | undefined {
  return leg?.waypoint?.location;
}

/** The beam for an approach to a runway, or null with NO FLS FOR THIS APPR. */
export function computeFlsBeam(approach: Approach | undefined, runway: Runway | undefined, airportIdent: string): FlsBeam | null {
  if (!approach || !runway || !FLS_TYPES.has(approach.type)) {
    return null;
  }
  const legs = approach.legs;
  let mapIndex = legs.findIndex((l) => l.approachWaypointDescriptor === ApproachWaypointDescriptor.MissedApproachPoint);
  if (mapIndex < 0) {
    mapIndex = legs.length - 1;
  }
  const mapLeg = legs[mapIndex];
  const map = location(mapLeg);
  const before = location(legs.slice(0, mapIndex).reverse().find((l) => location(l) !== undefined));
  if (!map || !before || mapLeg.verticalAngle === undefined) {
    return null;
  }
  const course = bearingTo(before, map);
  if (Math.abs(diffAngle(course, runway.bearing)) > MAX_COURSE_OFFSET_DEG) {
    return null;
  }
  const threshold = runway.thresholdLocation;
  const anchorAltitude = threshold.alt + (runway.thresholdCrossingHeight > 0 ? runway.thresholdCrossingHeight : 50);
  const thresholdIdent = `${airportIdent}${runway.ident.replace(/^RW/, '')}`;
  const endIdent = `EP${runway.ident.replace(/^RW/, '')}`;
  const toThreshold = distanceTo(map, threshold);
  // How far down the final course the threshold lies from the MAP: positive when the MAP is before it.
  const along = toThreshold * Math.cos((diffAngle(course, bearingTo(map, threshold)) * Math.PI) / 180);

  let anchor: Coordinates;
  let ident: string;
  let pseudo = false;
  if (toThreshold <= ON_THRESHOLD_NM) {
    anchor = threshold;
    ident = thresholdIdent;
  } else if (along > 0) {
    anchor = placeBearingDistance(map, course, along);
    pseudo = distanceTo(anchor, threshold) >= THRESHOLD_IDENT_NM;
    ident = pseudo ? endIdent : thresholdIdent;
  } else {
    const endLeg = legs.find((l) => l.approachWaypointDescriptor === ApproachWaypointDescriptor.FinalEndpointFix) ?? mapLeg;
    anchor = location(endLeg) ?? map;
    ident = distanceTo(anchor, threshold) < THRESHOLD_IDENT_NM ? thresholdIdent : (endLeg.waypoint?.ident ?? endIdent);
  }
  return {
    anchor,
    anchorAltitude,
    course,
    slope: Math.abs(mapLeg.verticalAngle),
    ident,
    pseudo,
    lateral: approach.type !== ApproachType.Loc && approach.type !== ApproachType.LocBackcourse,
  };
}

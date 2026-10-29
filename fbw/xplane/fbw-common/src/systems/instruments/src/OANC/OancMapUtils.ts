// Copyright (c) 2023-2024 FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0

import { MathUtils } from '@flybywiresim/fbw-sdk';
import { Position } from 'geojson';
import { clampAngle } from 'msfs-geo';

export function fractionalPointAlongLine(
  x1: number,
  y1: number,
  x2: number,
  y2: number,
  fraction: number,
): [number, number] {
  return [x1 + (x2 - x1) * fraction, y1 + (y2 - y1) * fraction];
}

export function midPoint(x1: number, y1: number, x2: number, y2: number): [number, number] {
  return fractionalPointAlongLine(x1, y1, x2, y2, 0.5);
}

export function pointAngle(x1: number, y1: number, x2: number, y2: number): number {
  return clampAngle(Math.atan2(y2 - y1, x2 - x1) * MathUtils.RADIANS_TO_DEGREES);
}

// Code adapted from https://www.jeffreythompson.org/collision-detection/line-rect.php

export function intersectLineWithRectangle(
  x1: number,
  y1: number,
  x2: number,
  y2: number,
  rx: number,
  ry: number,
  rw: number,
  rh: number,
): [number, number][] {
  // check if the line has hit any of the rectangle's sides
  // uses the Line/Line function below
  const left = lineLine(x1, y1, x2, y2, rx, ry, rx, ry + rh);
  const right = lineLine(x1, y1, x2, y2, rx + rw, ry, rx + rw, ry + rh);
  const top = lineLine(x1, y1, x2, y2, rx, ry, rx + rw, ry);
  const bottom = lineLine(x1, y1, x2, y2, rx, ry + rh, rx + rw, ry + rh);

  const ret = [];

  if (left) {
    ret.push(left);
  }
  if (right) {
    ret.push(right);
  }
  if (top) {
    ret.push(top);
  }
  if (bottom) {
    ret.push(bottom);
  }

  return ret;
}

export function isPointInRectangle(px: number, py: number, rx: number, ry: number, rw: number, rh: number): boolean {
  const insideX = rx <= px && rx + rw >= px;
  return insideX && ry <= py && ry + rh >= ry;
}

/** Where segment (x1, y1)-(x2, y2) crosses segment (x3, y3)-(x4, y4), if it does. */
function lineLine(x1: number, y1: number, x2: number, y2: number, x3: number, y3: number, x4: number, y4: number): [number, number] | undefined {
  const dx12 = x2 - x1;
  const dy12 = y2 - y1;
  const dx34 = x4 - x3;
  const dy34 = y4 - y3;
  const denominator = dy34 * dx12 - dx34 * dy12;
  const along12 = (dx34 * (y1 - y3) - dy34 * (x1 - x3)) / denominator;
  const along34 = (dx12 * (y1 - y3) - dy12 * (x1 - x3)) / denominator;
  if (!(along12 >= 0 && along12 <= 1 && along34 >= 0 && along34 <= 1)) {
    return undefined;
  }
  return [x1 + along12 * dx12, y1 + along12 * dy12];
}

export function pointToLineDistance(point: Position, lineStart: Position, lineEnd: Position) {
  return (
    Math.abs(
      (lineEnd[1] - lineStart[1]) * point[0] -
        (lineEnd[0] - lineStart[0]) * point[1] +
        lineEnd[0] * lineStart[1] -
        lineEnd[1] * lineStart[0],
    ) /
    ((lineEnd[1] - lineStart[1]) ** 2 + (lineEnd[0] - lineStart[0]) ** 2) ** 0.5
  );
}

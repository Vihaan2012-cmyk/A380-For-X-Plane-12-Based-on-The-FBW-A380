(() => {
  const fmc = (globalThis.__xpFmc || []).find(Boolean);
  if (!fmc) return JSON.stringify({ error: 'no FMC handle' });
  const path = new Set();
  const plain = (v, depth) => {
    if (v === null || v === undefined) return null;
    const t = typeof v;
    if (t === 'number') return Number.isFinite(v) ? v : String(v);
    if (t === 'string' || t === 'boolean') return v;
    if (t !== 'object') return undefined;
    if (depth <= 0 || path.has(v)) return undefined;
    if (typeof v.get === 'function' && typeof v.sub === 'function') return plain(v.get(), depth);
    path.add(v);
    try {
      if (Array.isArray(v) || ArrayBuffer.isView(v)) return Array.from(v).slice(0, 400).map((x) => plain(x, depth - 1));
      if (v instanceof Map) return Object.fromEntries([...v.entries()].slice(0, 400).map(([k, x]) => [String(k), plain(x, depth - 1)]));
      const out = {};
      for (const k of Object.keys(v)) {
        if (k.startsWith('_') || k === 'bus' || k === 'subs' || k === 'flightPlan' || k === 'parent' || k === 'segment') continue;
        const x = plain(v[k], depth - 1);
        if (x !== undefined) out[k] = x;
      }
      return out;
    } finally {
      path.delete(v);
    }
  };
  const leg = (e, i) => {
    if (!e) return null;
    if (e.isDiscontinuity) return { i, disco: true };
    const d = e.definition || {};
    const loc = d.waypoint && d.waypoint.location;
    const c = e.calculated;
    return {
      i,
      ident: e.ident,
      type: e.type,
      effectiveType: e.effectiveType,
      flags: e.flags,
      annotation: e.annotation,
      lat: loc ? loc.lat : null,
      lon: loc ? loc.long : null,
      descriptor: e.waypointDescriptor,
      constraintType: e.constraintType,
      alt: plain(e.altitudeConstraint, 3),
      spd: plain(e.speedConstraint, 3),
      hold: plain(e.modifiedHold || e.defaultHold, 3),
      calc: c
        ? {
            distance: c.distance,
            cumulativeDistance: c.cumulativeDistance,
            cumulativeDistanceToEnd: c.cumulativeDistanceToEnd,
            distanceWithTransitions: c.distanceWithTransitions,
            trueTrack: c.trueTrack,
            endsInTooSteepPath: c.endsInTooSteepPath,
          }
        : null,
      def: plain(d, 3),
    };
  };
  const plan = (p) =>
    p
      ? {
          version: p.version,
          activeLegIndex: p.activeLegIndex,
          origin: p.originAirport ? p.originAirport.ident : null,
          destination: p.destinationAirport ? p.destinationAirport.ident : null,
          legs: p.allLegs.map(leg),
          performance: plain(p.performanceData, 2),
        }
      : null;
  const fps = fmc.flightPlanInterface;
  const secondary = [];
  for (let n = 1; n <= 3; n++) {
    if (fps.hasSecondary && fps.hasSecondary(n)) secondary.push(plan(fps.secondary(n)));
  }
  const active = fps.hasActive ? fps.active : null;
  return JSON.stringify({
    plans: { active: plan(active), temporary: fps.hasTemporary ? plan(fps.temporary) : null, secondary },
    alternate: active && active.alternateFlightPlan ? plan(active.alternateFlightPlan) : null,
    guidance: plain(fmc.guidanceController && fmc.guidanceController.vnavDriver, 3),
    fmgc: plain(fmc.fmgc && fmc.fmgc.data, 2),
  });
})()

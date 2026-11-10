const exEcamBeforeStatus = ecamDiagram;
ecamDiagram = function (p) {
  const d = exEcamBeforeStatus(p);
  if (d) return d;
  const t = p.t.toUpperCase();
  const star = t.match(/^\*([A-Z/]+)/);
  if (star) return exEcamBeforeStatus({ ...p, s: star[1] === 'BLEED' ? 'AIR' : star[1], t: star[1] + ' SYS' });
  if (/^APU FIRE/.test(t)) return exPlanView(new Set(['APU']), true);
  if (/AUTOLAND/.test(t)) return exUnits('AUTO FLT', { ...p, t: 'AP SYS FMS 1 FMS 2 ' + p.t });
  if (/^T\.O SPEEDS|^T\.O V1/.test(t)) return exUnits('AUTO FLT', { ...p, t: 'FMS 1 FMS 2 FMC A FMC B FMC C ' + p.t });
  if (/^T\.O ACCELERATION/.test(t)) return exPlanView(new Set(['ENG1', 'ENG2', 'ENG3', 'ENG4']), false);
  if (/SIDESTICK/.test(t)) return exEcamBeforeStatus({ ...p, s: 'F/CTL' }) || exPlanView(new Set(['AILALL', 'ELEVALL']), false);
  if (/VIDEO|OIS/.test(t)) return exCds(p);
  if (/EMER EVAC/.test(t)) return exSideView(new Set(['DALL']), false);
  if (/VOLCANIC ASH/.test(t)) return exPlanView(new Set(['ENG1', 'ENG2', 'ENG3', 'ENG4']), false);
  if (/OVERWEIGHT LDG/.test(t)) return exPlanView(new Set(['GEAR', 'LGL', 'LGR', 'MLG']), false);
  return null;
};

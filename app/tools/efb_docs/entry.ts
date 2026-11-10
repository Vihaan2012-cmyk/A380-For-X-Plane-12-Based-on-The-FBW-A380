import {
  EcamAbnormalSensedProcedures,
  EcamDeferredProcedures,
  EcamLimitations,
  EcamInopSys,
  EcamInfos,
  ChecklistLineStyle,
} from '../../../../fbw-aircraft/fbw-a380x/src/systems/instruments/src/MsfsAvionicsCommon/EcamMessages';
import { EcamNormalProcedures, deferredProcedureIds } from '../../../../fbw-aircraft/fbw-a380x/src/systems/instruments/src/MsfsAvionicsCommon/EcamMessages/NormalProcedures';
import * as Perf from '../../../../fbw-aircraft/fbw-a380x/src/systems/shared/src/PerformanceConstants';
import { AbnormalNonSensedProceduresOverview } from '../../../../fbw-aircraft/fbw-a380x/src/systems/instruments/src/MsfsAvionicsCommon/EcamMessages/AbnormalNonSensedProcedures';

const styles = Object.fromEntries(Object.entries(ChecklistLineStyle).filter(([k]) => isNaN(Number(k))));
process.stdout.write(
  JSON.stringify({
    styles,
    normal: EcamNormalProcedures,
    deferredNormal: deferredProcedureIds,
    abnormal: EcamAbnormalSensedProcedures,
    nonSensed: AbnormalNonSensedProceduresOverview,
    deferred: EcamDeferredProcedures,
    limitations: EcamLimitations,
    inop: EcamInopSys,
    infos: EcamInfos,
    perf: Perf,
  }),
);

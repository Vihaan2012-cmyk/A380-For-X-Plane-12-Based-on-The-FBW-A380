import {
  EcamMemos,
  EcamInfos,
  EcamLimitations,
  EcamInopSys,
  ChecklistLineStyle,
  pfdMemoDisplay,
  AThrOffMemoKey,
  WD_NUM_LINES,
  WD_LINE_CHARACTERS,
} from '../../../fbw-aircraft/fbw-a380x/src/systems/instruments/src/MsfsAvionicsCommon/EcamMessages';
import {
  EcamNormalProcedures,
  deferredProcedureIds,
} from '../../../fbw-aircraft/fbw-a380x/src/systems/instruments/src/MsfsAvionicsCommon/EcamMessages/NormalProcedures';

const styles = Object.fromEntries(Object.entries(ChecklistLineStyle).filter(([k]) => isNaN(Number(k))));
process.stdout.write(
  JSON.stringify({
    styles,
    normal: EcamNormalProcedures,
    deferredNormal: deferredProcedureIds,
    memos: EcamMemos,
    infos: EcamInfos,
    limitations: EcamLimitations,
    inop: EcamInopSys,
    pfdMemoDisplay,
    aThrOffMemoKey: AThrOffMemoKey,
    wdNumLines: WD_NUM_LINES,
    wdLineCharacters: WD_LINE_CHARACTERS,
  }),
);

import {
  EcamAbnormalSensedProcedures,
  EcamDeferredProcedures,
  EcamAbNormalSensedSubMenuVector,
  DeferredProcedureType,
  FMS_PRED_UNRELIABLE_CHECKLIST_ITEM,
} from '../../../fbw-aircraft/fbw-a380x/src/systems/instruments/src/MsfsAvionicsCommon/EcamMessages';
import { EcamAbnormalSensedAta212223, EcamDeferredProcAta212223 } from '../../../fbw-aircraft/fbw-a380x/src/systems/instruments/src/MsfsAvionicsCommon/EcamMessages/AbnormalSensed/ata21-22-23';
import { EcamAbnormalSensedAta24 } from '../../../fbw-aircraft/fbw-a380x/src/systems/instruments/src/MsfsAvionicsCommon/EcamMessages/AbnormalSensed/ata24';
import { EcamAbnormalSensedAta26 } from '../../../fbw-aircraft/fbw-a380x/src/systems/instruments/src/MsfsAvionicsCommon/EcamMessages/AbnormalSensed/ata26';
import { EcamAbnormalSensedAta27, EcamDeferredProcAta27 } from '../../../fbw-aircraft/fbw-a380x/src/systems/instruments/src/MsfsAvionicsCommon/EcamMessages/AbnormalSensed/ata27';
import { EcamAbnormalSensedAta28 } from '../../../fbw-aircraft/fbw-a380x/src/systems/instruments/src/MsfsAvionicsCommon/EcamMessages/AbnormalSensed/ata28';
import { EcamAbnormalSensedAta2930 } from '../../../fbw-aircraft/fbw-a380x/src/systems/instruments/src/MsfsAvionicsCommon/EcamMessages/AbnormalSensed/ata29-30';
import { EcamAbnormalSensedAta313233, EcamDeferredProcAta313233 } from '../../../fbw-aircraft/fbw-a380x/src/systems/instruments/src/MsfsAvionicsCommon/EcamMessages/AbnormalSensed/ata31-32-33';
import { EcamAbnormalSensedAta34, EcamDeferredProcAta34 } from '../../../fbw-aircraft/fbw-a380x/src/systems/instruments/src/MsfsAvionicsCommon/EcamMessages/AbnormalSensed/ata34';
import { EcamAbnormalSensedAta353642 } from '../../../fbw-aircraft/fbw-a380x/src/systems/instruments/src/MsfsAvionicsCommon/EcamMessages/AbnormalSensed/ata35-36-42';
import { EcamAbnormalSensedAta46495256 } from '../../../fbw-aircraft/fbw-a380x/src/systems/instruments/src/MsfsAvionicsCommon/EcamMessages/AbnormalSensed/ata46-49-52-56';
import { EcamAbnormalSensedAta70 } from '../../../fbw-aircraft/fbw-a380x/src/systems/instruments/src/MsfsAvionicsCommon/EcamMessages/AbnormalSensed/ata70';
import { EcamAbnormalSensedAta80Rest } from '../../../fbw-aircraft/fbw-a380x/src/systems/instruments/src/MsfsAvionicsCommon/EcamMessages/AbnormalSensed/ata80-rest';
import { EcamAbnormalSecondaryFailures } from '../../../fbw-aircraft/fbw-a380x/src/systems/instruments/src/MsfsAvionicsCommon/EcamMessages/AbnormalSensed/secondary-failures';
import { AbnormalNonSensedProceduresOverview } from '../../../fbw-aircraft/fbw-a380x/src/systems/instruments/src/MsfsAvionicsCommon/EcamMessages/AbnormalNonSensedProcedures';

process.stdout.write(
  JSON.stringify({
    groups: {
      'ata21-22-23': { sensed: EcamAbnormalSensedAta212223, deferred: EcamDeferredProcAta212223 },
      ata24: { sensed: EcamAbnormalSensedAta24 },
      ata26: { sensed: EcamAbnormalSensedAta26 },
      ata27: { sensed: EcamAbnormalSensedAta27, deferred: EcamDeferredProcAta27 },
      ata28: { sensed: EcamAbnormalSensedAta28 },
      'ata29-30': { sensed: EcamAbnormalSensedAta2930 },
      'ata31-32-33': { sensed: EcamAbnormalSensedAta313233, deferred: EcamDeferredProcAta313233 },
      ata34: { sensed: EcamAbnormalSensedAta34, deferred: EcamDeferredProcAta34 },
      'ata35-36-42': { sensed: EcamAbnormalSensedAta353642 },
      'ata46-49-52-56': { sensed: EcamAbnormalSensedAta46495256 },
      ata70: { sensed: EcamAbnormalSensedAta70 },
      'ata80-rest': { sensed: EcamAbnormalSensedAta80Rest },
      'secondary-failures': { sensed: EcamAbnormalSecondaryFailures },
    },
    merged: {
      sensedIds: Object.keys(EcamAbnormalSensedProcedures),
      deferredIds: Object.keys(EcamDeferredProcedures),
    },
    nonSensed: AbnormalNonSensedProceduresOverview,
    fmsPredUnreliable: FMS_PRED_UNRELIABLE_CHECKLIST_ITEM,
    subMenu: EcamAbNormalSensedSubMenuVector,
    deferredTypes: Object.fromEntries(Object.entries(DeferredProcedureType).filter(([k]) => isNaN(Number(k)))),
  }),
);

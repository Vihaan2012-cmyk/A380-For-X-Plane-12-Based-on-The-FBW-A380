import { Bus, VarUnit, VarPublisher, VarTopic } from 'xplane-js/ui';

export interface FcuSimvars {
  lightsTest: boolean;

  eisDisplayLeftActive: boolean;
  eisDisplayRightActive: boolean;
  afsDisplayActive: boolean;
  afsBackupActive: boolean;
  eisLeftBackupActive: boolean;
  eisRightBackupActive: boolean;

  eisDisplayLeftBaroIsInhg: boolean;
  eisDisplayLeftBaroIsStd: boolean;
  eisDisplayLeftBaroValue: number;
  eisDisplayLeftBaroMode: number;
  eisDisplayLeftBaroPresetVisible: boolean;
  eisDisplayLeftNavaid1Mode: number;
  eisDisplayLeftNavaid2Mode: number;
  eisDisplayRightBaroIsInhg: boolean;
  eisDisplayRightBaroIsStd: boolean;
  eisDisplayRightBaroValue: number;
  eisDisplayRightBaroMode: number;
  eisDisplayRightBaroPresetVisible: boolean;
  eisDisplayRightNavaid1Mode: number;
  eisDisplayRightNavaid2Mode: number;

  afsDisplayTrkFpaMode: boolean;
  afsDisplayTrueMode: boolean;
  afsDisplayMachMode: boolean;
  afsDisplaySpdMachValue: number;
  afsDisplaySpdMachDashes: boolean;
  afsDisplayHdgTrkValue: number;
  afsDisplayHdgTrkDashes: boolean;
  afsDisplayAltValue: number;
  afsDisplayVsFpaValue: number;
  afsDisplayVsFpaDashes: boolean;
}

export enum FcuVars {
  lightsTest = 'L:A32NX_OVHD_INTLT_ANN',

  eisDisplayLeftActive = 'L:A32NX_FCU_EFIS_L_CP_ACTIVE',
  eisDisplayRightActive = 'L:A32NX_FCU_EFIS_R_CP_ACTIVE',
  afsDisplayActive = 'L:A32NX_FCU_AFS_CP_ACTIVE',
  afsBackupActive = 'L:A380X_FCU_BKUP_AFS_ACTIVE',
  eisLeftBackupActive = 'L:A380X_FCU_BKUP_EFIS_L_ACTIVE',
  eisRightBackupActive = 'L:A380X_FCU_BKUP_EFIS_R_ACTIVE',

  eisDisplayLeftBaroIsInhg = 'L:A32NX_FCU_EFIS_L_DISPLAY_BARO_IS_INHG',
  eisDisplayLeftBaroIsStd = 'L:A32NX_FCU_EFIS_L_DISPLAY_BARO_IS_STD',
  eisDisplayLeftBaroValue = 'L:A32NX_FCU_EFIS_L_DISPLAY_BARO_VALUE',
  eisDisplayLeftBaroMode = 'L:A32NX_FCU_EFIS_L_DISPLAY_BARO_MODE',
  eisDisplayLeftBaroPresetVisible = 'L:A32NX_FCU_EFIS_L_DISPLAY_BARO_PRESET_VISIBLE',
  eisDisplayLeftNavaid1Mode = 'L:A32NX_FCU_EFIS_L_NAVAID_1_MODE',
  eisDisplayLeftNavaid2Mode = 'L:A32NX_FCU_EFIS_L_NAVAID_2_MODE',
  eisDisplayRightBaroIsInhg = 'L:A32NX_FCU_EFIS_R_DISPLAY_BARO_IS_INHG',
  eisDisplayRightBaroIsStd = 'L:A32NX_FCU_EFIS_R_DISPLAY_BARO_IS_STD',
  eisDisplayRightBaroValue = 'L:A32NX_FCU_EFIS_R_DISPLAY_BARO_VALUE',
  eisDisplayRightBaroMode = 'L:A32NX_FCU_EFIS_R_DISPLAY_BARO_MODE',
  eisDisplayRightBaroPresetVisible = 'L:A32NX_FCU_EFIS_R_DISPLAY_BARO_PRESET_VISIBLE',
  eisDisplayRightNavaid1Mode = 'L:A32NX_FCU_EFIS_R_NAVAID_1_MODE',
  eisDisplayRightNavaid2Mode = 'L:A32NX_FCU_EFIS_R_NAVAID_2_MODE',

  afsDisplayTrkFpaMode = 'L:A32NX_FCU_AFS_DISPLAY_TRK_FPA_MODE',
  afsDisplayTrueMode = 'L:A32NX_FCU_AFS_DISPLAY_TRUE_MODE',
  afsDisplayMachMode = 'L:A32NX_FCU_AFS_DISPLAY_MACH_MODE',
  afsDisplaySpdMachValue = 'L:A32NX_FCU_AFS_DISPLAY_SPD_MACH_VALUE',
  afsDisplaySpdMachDashes = 'L:A32NX_FCU_AFS_DISPLAY_SPD_MACH_DASHES',
  afsDisplayHdgTrkValue = 'L:A32NX_FCU_AFS_DISPLAY_HDG_TRK_VALUE',
  afsDisplayHdgTrkDashes = 'L:A32NX_FCU_AFS_DISPLAY_HDG_TRK_DASHES',
  afsDisplayAltValue = 'L:A32NX_FCU_AFS_DISPLAY_ALT_VALUE',
  afsDisplayVsFpaValue = 'L:A32NX_FCU_AFS_DISPLAY_VS_FPA_VALUE',
  afsDisplayVsFpaDashes = 'L:A32NX_FCU_AFS_DISPLAY_VS_FPA_DASHES',
}

export class FCUSimvarPublisher extends VarPublisher<FcuSimvars> {
  private static simvars = new Map<keyof FcuSimvars, VarTopic<any>>([
    ['lightsTest', { name: FcuVars.lightsTest, type: VarUnit.Number, transform: (v) => v === 0 }],

    ['eisDisplayLeftActive', { name: FcuVars.eisDisplayLeftActive, type: VarUnit.Bool }],
    ['eisDisplayRightActive', { name: FcuVars.eisDisplayRightActive, type: VarUnit.Bool }],
    ['afsDisplayActive', { name: FcuVars.afsDisplayActive, type: VarUnit.Bool }],
    ['afsBackupActive', { name: FcuVars.afsBackupActive, type: VarUnit.Bool }],
    ['eisLeftBackupActive', { name: FcuVars.eisLeftBackupActive, type: VarUnit.Bool }],
    ['eisRightBackupActive', { name: FcuVars.eisRightBackupActive, type: VarUnit.Bool }],

    ['eisDisplayLeftBaroIsInhg', { name: FcuVars.eisDisplayLeftBaroIsInhg, type: VarUnit.Bool }],
    ['eisDisplayLeftBaroIsStd', { name: FcuVars.eisDisplayLeftBaroIsStd, type: VarUnit.Bool }],
    ['eisDisplayLeftBaroValue', { name: FcuVars.eisDisplayLeftBaroValue, type: VarUnit.Number }],
    ['eisDisplayLeftBaroMode', { name: FcuVars.eisDisplayLeftBaroMode, type: VarUnit.Number }],
    ['eisDisplayLeftBaroPresetVisible', { name: FcuVars.eisDisplayLeftBaroPresetVisible, type: VarUnit.Bool }],
    ['eisDisplayLeftNavaid1Mode', { name: FcuVars.eisDisplayLeftNavaid1Mode, type: VarUnit.Number }],
    ['eisDisplayLeftNavaid2Mode', { name: FcuVars.eisDisplayLeftNavaid2Mode, type: VarUnit.Number }],
    ['eisDisplayRightBaroIsInhg', { name: FcuVars.eisDisplayRightBaroIsInhg, type: VarUnit.Bool }],
    ['eisDisplayRightBaroIsStd', { name: FcuVars.eisDisplayRightBaroIsStd, type: VarUnit.Bool }],
    ['eisDisplayRightBaroValue', { name: FcuVars.eisDisplayRightBaroValue, type: VarUnit.Number }],
    ['eisDisplayRightBaroMode', { name: FcuVars.eisDisplayRightBaroMode, type: VarUnit.Number }],
    [
      'eisDisplayRightBaroPresetVisible',
      { name: FcuVars.eisDisplayRightBaroPresetVisible, type: VarUnit.Bool },
    ],
    ['eisDisplayRightNavaid1Mode', { name: FcuVars.eisDisplayRightNavaid1Mode, type: VarUnit.Number }],
    ['eisDisplayRightNavaid2Mode', { name: FcuVars.eisDisplayRightNavaid2Mode, type: VarUnit.Number }],

    ['afsDisplayTrkFpaMode', { name: FcuVars.afsDisplayTrkFpaMode, type: VarUnit.Bool }],
    ['afsDisplayTrueMode', { name: FcuVars.afsDisplayTrueMode, type: VarUnit.Bool }],
    ['afsDisplayMachMode', { name: FcuVars.afsDisplayMachMode, type: VarUnit.Bool }],
    ['afsDisplaySpdMachValue', { name: FcuVars.afsDisplaySpdMachValue, type: VarUnit.Number }],
    ['afsDisplaySpdMachDashes', { name: FcuVars.afsDisplaySpdMachDashes, type: VarUnit.Bool }],
    ['afsDisplayHdgTrkValue', { name: FcuVars.afsDisplayHdgTrkValue, type: VarUnit.Number }],
    ['afsDisplayHdgTrkDashes', { name: FcuVars.afsDisplayHdgTrkDashes, type: VarUnit.Bool }],
    ['afsDisplayAltValue', { name: FcuVars.afsDisplayAltValue, type: VarUnit.Number }],
    ['afsDisplayVsFpaValue', { name: FcuVars.afsDisplayVsFpaValue, type: VarUnit.Number }],
    ['afsDisplayVsFpaDashes', { name: FcuVars.afsDisplayVsFpaDashes, type: VarUnit.Bool }],
  ]);

  public constructor(bus: Bus) {
    super(FCUSimvarPublisher.simvars, bus);
  }
}

import { Bus, VarPublisher, VarUnit } from 'xplane-js/ui';
import { GeoPos3 } from 'xplane-js/data';

export interface VorSimVars {
  nav1Ident: string;
  nav1Frequency: number;
  nav1HasDme: boolean;
  nav1DmeDistance: NauticalMiles;
  nav1RelativeBearing: Degrees;
  nav1Obs: Degrees;
  nav1Localizer: Degrees;
  nav1RadialError: Degrees;
  nav1Available: boolean;
  nav1StationDeclination: Degrees;
  nav1Location: GeoPos3;

  nav2Ident: string;
  nav2Frequency: number;
  nav2HasDme: boolean;
  nav2DmeDistance: NauticalMiles;
  nav2RelativeBearing: Degrees;
  nav2Obs: Degrees;
  nav2Localizer: Degrees;
  nav2RadialError: Degrees;
  nav2Available: boolean;
  nav2StationDeclination: Degrees;
  nav2Location: GeoPos3;

  nav3Ident: string;
  nav3Frequency: number;
  nav3HasDme: boolean;
  nav3DmeDistance: NauticalMiles;
  nav3RelativeBearing: Degrees;
  nav3Obs: Degrees;
  nav3Localizer: Degrees;
  nav3RadialError: Degrees;
  nav3Available: boolean;
  nav3StationDeclination: Degrees;
  nav3Location: GeoPos3;

  nav4Ident: string;
  nav4Frequency: number;
  nav4HasDme: boolean;
  nav4DmeDistance: NauticalMiles;
  nav4RelativeBearing: Degrees;
  nav4Obs: Degrees;
  nav4Localizer: Degrees;
  nav4RadialError: Degrees;
  nav4Available: boolean;
  nav4StationDeclination: Degrees;
  nav4Location: GeoPos3;

  localizerValid: boolean;
  glideSlopeValid: boolean;
  glideSlopeDeviation: number;

  adf1Ident: string;
  adf1ActiveFrequency: number;
  adf1SignalStrength: number;
  adf1Radial: number;

  adf2Ident: string;
  adf2ActiveFrequency: number;
  adf2SignalStrength: number;
  adf2Radial: number;
}

export class VorBusPublisher extends VarPublisher<VorSimVars> {
  constructor(bus: Bus) {
    super(
      new Map([
        ['nav1Ident', { name: 'fbw/NAV_IDENT_1', type: VarUnit.String }],
        ['nav1Frequency', { name: 'fbw/NAV_ACTIVE_FREQUENCY_1', type: VarUnit.MHz }],
        ['nav1HasDme', { name: 'fbw/NAV_HAS_DME_1', type: VarUnit.Bool }],
        ['nav1DmeDistance', { name: 'fbw/NAV_DME_1', type: VarUnit.NM }],
        ['nav1RelativeBearing', { name: 'fbw/NAV_RELATIVE_BEARING_TO_STATION_1', type: VarUnit.Degree }],
        ['nav1Obs', { name: 'fbw/NAV_OBS_1', type: VarUnit.Degree }],
        ['nav1Localizer', { name: 'fbw/NAV_LOCALIZER_1', type: VarUnit.Degree }],
        ['nav1RadialError', { name: 'fbw/NAV_RADIAL_ERROR_1', type: VarUnit.Degree }],
        // This is fine at the moment. The database does not have the necessary information to implement proper logic.
        ['nav1Available', { name: 'fbw/NAV_HAS_NAV_1', type: VarUnit.Bool }],
        ['nav1StationDeclination', { name: 'fbw/NAV_MAGVAR_1', type: VarUnit.Degree }],
        ['nav1Location', { name: 'fbw/NAV_VOR_LATLONALT_1', type: VarUnit.LLA }],

        ['nav2Ident', { name: 'fbw/NAV_IDENT_2', type: VarUnit.String }],
        ['nav2Frequency', { name: 'fbw/NAV_ACTIVE_FREQUENCY_2', type: VarUnit.MHz }],
        ['nav2HasDme', { name: 'fbw/NAV_HAS_DME_2', type: VarUnit.Bool }],
        ['nav2DmeDistance', { name: 'fbw/NAV_DME_2', type: VarUnit.NM }],
        ['nav2RelativeBearing', { name: 'fbw/NAV_RELATIVE_BEARING_TO_STATION_2', type: VarUnit.Degree }],
        ['nav2Obs', { name: 'fbw/NAV_OBS_2', type: VarUnit.Degree }],
        ['nav2Localizer', { name: 'fbw/NAV_LOCALIZER_2', type: VarUnit.Degree }],
        ['nav2RadialError', { name: 'fbw/NAV_RADIAL_ERROR_2', type: VarUnit.Degree }],
        ['nav2Available', { name: 'fbw/NAV_HAS_NAV_2', type: VarUnit.Bool }],
        ['nav2StationDeclination', { name: 'fbw/NAV_MAGVAR_2', type: VarUnit.Degree }],
        ['nav2Location', { name: 'fbw/NAV_VOR_LATLONALT_2', type: VarUnit.LLA }],

        ['nav3Ident', { name: 'fbw/NAV_IDENT_3', type: VarUnit.String }],
        ['nav3Frequency', { name: 'fbw/NAV_ACTIVE_FREQUENCY_3', type: VarUnit.MHz }],
        ['nav3HasDme', { name: 'fbw/NAV_HAS_DME_3', type: VarUnit.Bool }],
        ['nav3DmeDistance', { name: 'fbw/NAV_DME_3', type: VarUnit.NM }],
        ['nav3RelativeBearing', { name: 'fbw/NAV_RELATIVE_BEARING_TO_STATION_3', type: VarUnit.Degree }],
        ['nav3Obs', { name: 'fbw/NAV_OBS_3', type: VarUnit.Degree }],
        ['nav3Localizer', { name: 'fbw/NAV_LOCALIZER_3', type: VarUnit.Degree }],
        ['nav3RadialError', { name: 'fbw/NAV_RADIAL_ERROR_3', type: VarUnit.Degree }],
        ['nav3Available', { name: 'fbw/NAV_HAS_NAV_3', type: VarUnit.Bool }],
        ['nav3StationDeclination', { name: 'fbw/NAV_MAGVAR_3', type: VarUnit.Degree }],
        ['nav3Location', { name: 'fbw/NAV_VOR_LATLONALT_3', type: VarUnit.LLA }],

        ['nav4Ident', { name: 'fbw/NAV_IDENT_4', type: VarUnit.String }],
        ['nav4Frequency', { name: 'fbw/NAV_ACTIVE_FREQUENCY_4', type: VarUnit.MHz }],
        ['nav4HasDme', { name: 'fbw/NAV_HAS_DME_4', type: VarUnit.Bool }],
        ['nav4DmeDistance', { name: 'fbw/NAV_DME_4', type: VarUnit.NM }],
        ['nav4RelativeBearing', { name: 'fbw/NAV_RELATIVE_BEARING_TO_STATION_4', type: VarUnit.Degree }],
        ['nav4Obs', { name: 'fbw/NAV_OBS_4', type: VarUnit.Degree }],
        ['nav4Localizer', { name: 'fbw/NAV_LOCALIZER_4', type: VarUnit.Degree }],
        ['nav4RadialError', { name: 'fbw/NAV_RADIAL_ERROR_4', type: VarUnit.Degree }],
        ['nav4Available', { name: 'fbw/NAV_HAS_NAV_4', type: VarUnit.Bool }],
        ['nav4StationDeclination', { name: 'fbw/NAV_MAGVAR_4', type: VarUnit.Degree }],
        ['nav4Location', { name: 'fbw/NAV_VOR_LATLONALT_4', type: VarUnit.LLA }],

        ['localizerValid', { name: 'L:A32NX_RADIO_RECEIVER_LOC_IS_VALID', type: VarUnit.Bool }],
        ['glideSlopeValid', { name: 'L:A32NX_RADIO_RECEIVER_GS_IS_VALID', type: VarUnit.Bool }],
        ['glideSlopeDeviation', { name: 'L:A32NX_RADIO_RECEIVER_GS_DEVIATION', type: VarUnit.Number }],

        ['adf1Ident', { name: 'fbw/ADF_IDENT_1', type: VarUnit.String }],
        ['adf1ActiveFrequency', { name: 'fbw/ADF_ACTIVE_FREQUENCY_1', type: VarUnit.KHz }],
        ['adf1SignalStrength', { name: 'fbw/ADF_SIGNAL_1', type: VarUnit.Number }],
        ['adf1Radial', { name: 'fbw/ADF_RADIAL_1', type: VarUnit.Degree }],

        ['adf2Ident', { name: 'fbw/ADF_IDENT_2', type: VarUnit.String }],
        ['adf2ActiveFrequency', { name: 'fbw/ADF_ACTIVE_FREQUENCY_2', type: VarUnit.KHz }],
        ['adf2SignalStrength', { name: 'fbw/ADF_SIGNAL_2', type: VarUnit.Number }],
        ['adf2Radial', { name: 'fbw/ADF_RADIAL_2', type: VarUnit.Degree }],
      ]),
      bus,
    );
  }
}

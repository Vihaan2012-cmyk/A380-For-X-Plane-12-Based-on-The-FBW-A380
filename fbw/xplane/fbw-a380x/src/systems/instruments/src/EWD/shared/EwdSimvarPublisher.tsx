import {
  Bus,
  IndexedTopic,
  Pacer,
  VarPublisher,
  VarTopic,
  VarUnit,
} from 'xplane-js/ui';

/* eslint-disable camelcase */
export interface BaseEwdSimvars {
  engine_state: number;
  egt: number;
  n1: number;
  n1_commanded: number;
  throttle_position_n1: number;
  throttle_position: number;
  reverser_deploying: boolean;
  reverser_deployed: boolean;
  thrust_reverse: boolean;
  eng_anti_ice: boolean;
  wing_anti_ice: boolean;
  eng_selector_position: number;
  autothrustStatus: number;
  thrust_limit_type: number;
  thrust_limit: number;
  thrust_limit_idle: number;
  thrust_limit_toga: number;
  thrust_limit_rev: number;
  satRaw: number;
  n1Idle: number;
  flex: number;
  athrTogaWarning: boolean;
  cpiomB1AgsDiscreteRaw: number;
  cpiomB2AgsDiscreteRaw: number;
  cpiomB3AgsDiscreteRaw: number;
  cpiomB4AgsDiscreteRaw: number;
  fwc_flight_phase: number;
  memo_left: number;
  memo_right: number;
  nose_gear_compressed: boolean;
  engine_fire_pb: boolean;
  engine_separated: boolean;
  fws1_is_healthy: boolean;
  fws2_is_healthy: boolean;
  afdx_3_3_reachable: boolean;
  afdx_13_13_reachable: boolean;
  afdx_4_3_reachable: boolean;
  afdx_14_13_reachable: boolean;
}

type IndexedTopics =
  | 'engine_state'
  | 'egt'
  | 'n1'
  | 'n1_commanded'
  | 'throttle_position_n1'
  | 'throttle_position'
  | 'reverser_deploying'
  | 'reverser_deployed'
  | 'thrust_reverse'
  | 'eng_anti_ice'
  | 'memo_left'
  | 'memo_right'
  | 'nose_gear_compressed'
  | 'engine_fire_pb'
  | 'engine_separated';
type EwdIndexedEvents = {
  [P in keyof Pick<BaseEwdSimvars, IndexedTopics> as IndexedTopic<P>]: BaseEwdSimvars[P];
};

/**
 * EWD events.
 */
export interface EwdSimvars extends BaseEwdSimvars, EwdIndexedEvents {}

export class EwdSimvarPublisher extends VarPublisher<EwdSimvars> {
  constructor(bus: Bus, pacer?: Pacer<EwdSimvars>) {
    const simvars: [keyof EwdSimvars, VarTopic<any>][] = [
      ['engine_state', { name: 'L:A32NX_ENGINE_STATE:#index#', type: VarUnit.Number, indexed: true }],
      ['egt', { name: 'L:A32NX_ENGINE_EGT:#index#', type: VarUnit.Number, indexed: true }],
      ['n1', { name: 'L:A32NX_ENGINE_N1:#index#', type: VarUnit.Number, indexed: true }],
      [
        'n1_commanded',
        { name: 'L:A32NX_AUTOTHRUST_N1_COMMANDED:#index#', type: VarUnit.Number, indexed: true },
      ],
      [
        'throttle_position_n1',
        { name: 'L:A32NX_AUTOTHRUST_TLA_N1:#index#', type: VarUnit.Number, indexed: true },
      ],
      ['throttle_position', { name: 'L:A32NX_AUTOTHRUST_TLA:#index#', type: VarUnit.Number, indexed: true }],
      ['reverser_deploying', { name: 'L:A32NX_REVERSER_#index#_DEPLOYING', type: VarUnit.Bool, indexed: true }],
      ['reverser_deployed', { name: 'L:A32NX_REVERSER_#index#_DEPLOYED', type: VarUnit.Bool, indexed: true }],
      ['thrust_reverse', { name: 'L:A32NX_AUTOTHRUST_REVERSE:#index#', type: VarUnit.Bool, indexed: true }],
      ['eng_anti_ice', { name: 'A:fbw/ENG_ANTI_ICE_#index#', type: VarUnit.Bool, indexed: true }],
      ['wing_anti_ice', { name: 'A:fbw/STRUCTURAL_DEICE_SWITCH', type: VarUnit.Bool }],
      ['eng_selector_position', { name: 'L:XMLVAR_ENG_MODE_SEL', type: VarUnit.Enum }],
      ['autothrustStatus', { name: 'L:A32NX_AUTOTHRUST_STATUS', type: VarUnit.Enum }],
      ['thrust_limit_type', { name: 'L:A32NX_AUTOTHRUST_THRUST_LIMIT_TYPE', type: VarUnit.Number }],
      ['thrust_limit', { name: 'L:A32NX_AUTOTHRUST_THRUST_LIMIT', type: VarUnit.Number }],
      ['thrust_limit_idle', { name: 'L:A32NX_AUTOTHRUST_THRUST_LIMIT_IDLE', type: VarUnit.Number }],
      ['thrust_limit_toga', { name: 'L:A32NX_AUTOTHRUST_THRUST_LIMIT_TOGA', type: VarUnit.Number }],
      ['thrust_limit_rev', { name: 'L:A32NX_AUTOTHRUST_THRUST_LIMIT_REV', type: VarUnit.Number }],
      ['satRaw', { name: 'L:A32NX_ADIRS_ADR_1_STATIC_AIR_TEMPERATURE', type: VarUnit.Number }],
      ['n1Idle', { name: 'L:A32NX_ENGINE_IDLE_N1', type: VarUnit.Number }],
      ['flex', { name: 'L:A32NX_AIRLINER_TO_FLEX_TEMP', type: VarUnit.Number }],
      ['athrTogaWarning', { name: 'L:A32NX_AUTOTHRUST_THRUST_LEVER_WARNING_TOGA', type: VarUnit.Bool }],
      ['cpiomB1AgsDiscreteRaw', { name: 'L:A32NX_COND_CPIOM_B1_AGS_DISCRETE_WORD', type: VarUnit.Number }],
      ['cpiomB2AgsDiscreteRaw', { name: 'L:A32NX_COND_CPIOM_B2_AGS_DISCRETE_WORD', type: VarUnit.Number }],
      ['cpiomB3AgsDiscreteRaw', { name: 'L:A32NX_COND_CPIOM_B3_AGS_DISCRETE_WORD', type: VarUnit.Number }],
      ['cpiomB4AgsDiscreteRaw', { name: 'L:A32NX_COND_CPIOM_B4_AGS_DISCRETE_WORD', type: VarUnit.Number }],
      ['fwc_flight_phase', { name: 'L:A32NX_FWC_FLIGHT_PHASE', type: VarUnit.Enum }],
      ['memo_left', { name: 'L:A32NX_EWD_LOWER_LEFT_LINE_#index#', type: VarUnit.Number, indexed: true }],
      ['memo_right', { name: 'L:A32NX_EWD_LOWER_RIGHT_LINE_#index#', type: VarUnit.Number, indexed: true }],
      [
        'nose_gear_compressed',
        { name: 'L:A32NX_LGCIU_#index#_NOSE_GEAR_COMPRESSED', type: VarUnit.Bool, indexed: true },
      ],
      ['engine_fire_pb', { name: 'L:A32NX_FIRE_BUTTON_ENG#index#', type: VarUnit.Bool, indexed: true }],
      ['engine_separated', { name: 'L:A32NX_ENGINE_SEPARATED:#index#', type: VarUnit.Bool, indexed: true }],
      ['fws1_is_healthy', { name: 'L:A32NX_FWS1_IS_HEALTHY', type: VarUnit.Bool }],
      ['fws2_is_healthy', { name: 'L:A32NX_FWS2_IS_HEALTHY', type: VarUnit.Bool }],
      ['afdx_3_3_reachable', { name: 'L:A32NX_AFDX_3_3_REACHABLE', type: VarUnit.Bool }],
      ['afdx_13_13_reachable', { name: 'L:A32NX_AFDX_13_13_REACHABLE', type: VarUnit.Bool }],
      ['afdx_4_3_reachable', { name: 'L:A32NX_AFDX_4_3_REACHABLE', type: VarUnit.Bool }],
      ['afdx_14_13_reachable', { name: 'L:A32NX_AFDX_14_13_REACHABLE', type: VarUnit.Bool }],
    ];

    super(new Map(simvars), bus, pacer);
  }
}

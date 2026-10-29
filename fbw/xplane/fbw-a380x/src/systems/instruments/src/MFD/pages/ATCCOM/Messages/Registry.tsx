import { SignalList, UI, ViewNode } from 'xplane-js/ui';
import { AtccomMfdPageProps } from '../../../MFD';
import { RequestClimb } from './Vertical/RequestClimb';
import { RequestDescend } from './Vertical/RequestDescend';
import { RequestDepartureClearance } from './Clearance/RequestDepartureClearance';
import { RequestDirect } from './Lateral/RequestDirect';
import { RequestGroundTrack } from './Lateral/RequestGroundTrack';
import { RequestHeading } from './Lateral/RequestHeading';

export const MaxRequestElements = 5;

export interface MessageVisualizationProps extends AtccomMfdPageProps {
  mode: string;
  index: number;
  messageElements: SignalList<MessageFrame>;
  onDelete: () => void;
}

export type MessageFrame = {
  id: number;
  message: object | undefined;
  readyToSend: boolean;
};

export const MessageTable: {
  [id: string]: {
    visualization: (
      props: any,
      mode: string,
      index: number,
      // messageElements: { id: string; message: undefined; readyToSend: boolean }[],
      messageElements: SignalList<MessageFrame>,
      onDelete: any,
    ) => ViewNode;
    blacklisting: string[];
    exchanging: string | undefined;
    singleMessage: boolean;
  };
} = {
  RequestClimb: {
    visualization: (props, mode, index, messageElements, onDelete): ViewNode => {
      return (
        <RequestClimb
          bus={props.bus}
          mfd={props.mfd}
          atcService={props.atcService}
          mode={mode}
          index={index}
          messageElements={messageElements}
          onDelete={onDelete}
        />
      );
    },
    blacklisting: [],
    exchanging: 'RequestDescend',
    singleMessage: false,
  },
  RequestDescend: {
    visualization: (props, mode, index, messageElements, onDelete): ViewNode => {
      return (
        <RequestDescend
          bus={props.bus}
          mfd={props.mfd}
          atcService={props.atcService}
          mode={mode}
          index={index}
          messageElements={messageElements}
          onDelete={onDelete}
        />
      );
    },
    blacklisting: [],
    exchanging: 'RequestClimb',
    singleMessage: false,
  },
  RequestDirect: {
    visualization: (props, mode, index, messageElements, onDelete): ViewNode => {
      return (
        <RequestDirect
          bus={props.bus}
          mfd={props.mfd}
          atcService={props.atcService}
          mode={mode}
          index={index}
          messageElements={messageElements}
          onDelete={onDelete}
        />
      );
    },
    blacklisting: [],
    exchanging: undefined,
    singleMessage: false,
  },
  RequestHeading: {
    visualization: (props, mode, index, messageElements, onDelete): ViewNode => {
      return (
        <RequestHeading
          bus={props.bus}
          mfd={props.mfd}
          atcService={props.atcService}
          mode={mode}
          index={index}
          messageElements={messageElements}
          onDelete={onDelete}
        />
      );
    },
    blacklisting: [],
    exchanging: undefined,
    singleMessage: false,
  },
  RequestGroundTrack: {
    visualization: (props, mode, index, messageElements, onDelete): ViewNode => {
      return (
        <RequestGroundTrack
          bus={props.bus}
          mfd={props.mfd}
          atcService={props.atcService}
          mode={mode}
          index={index}
          messageElements={messageElements}
          onDelete={onDelete}
        />
      );
    },
    blacklisting: [],
    exchanging: undefined,
    singleMessage: false,
  },
  RequestDepartureClearance: {
    visualization: (props, mode, index, messageElements, onDelete): ViewNode => {
      return (
        <RequestDepartureClearance
          bus={props.bus}
          mfd={props.mfd}
          atcService={props.atcService}
          mode={mode}
          index={index}
          messageElements={messageElements}
          onDelete={onDelete}
        />
      );
    },
    blacklisting: [],
    exchanging: undefined,
    singleMessage: true,
  },
};

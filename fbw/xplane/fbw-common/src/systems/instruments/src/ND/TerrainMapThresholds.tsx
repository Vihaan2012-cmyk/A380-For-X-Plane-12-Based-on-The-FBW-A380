import { UI, BusSignal, View, Bus, ViewNode, Derived } from 'xplane-js/ui';

import { GenericTawsEvents, TerrainLevelMode } from './types/GenericTawsEvents';

export interface TerrainMapThresholdsProps {
  bus: Bus;
  paddingText: string;
}

export class TerrainMapThresholds extends View<TerrainMapThresholdsProps> {
  private readonly sub = this.props.bus.inbox<GenericTawsEvents>();

  private readonly maxElevationSub = BusSignal.from(this.sub.topic('egpwc.maxElevation'), -1);

  private readonly maxElevationModeSub = BusSignal.from(this.sub.topic('egpwc.maxElevationMode'), 0);

  private readonly minElevationSub = BusSignal.from(this.sub.topic('egpwc.minElevation'), -1);

  private readonly minElevationModeSub = BusSignal.from(this.sub.topic('egpwc.minElevationMode'), 0);

  private readonly shown = Derived.from(
    ([min, nax]) => min >= 0 && nax >= 0,
    this.minElevationSub,
    this.maxElevationSub,
  );

  private readonly upperBorder = this.maxElevationSub.derive((elevation) => {
    let roundedElevation: string;
    if (elevation !== 0) {
      roundedElevation = Math.round(elevation / 100 + 0.5).toString();
    } else {
      roundedElevation = '0';
    }

    return roundedElevation.padStart(3, this.props.paddingText);
  });

  private readonly upperBorderColor = this.maxElevationModeSub.derive((mode) => {
    switch (mode as TerrainLevelMode) {
      case TerrainLevelMode.Caution:
        return 'rgb(255, 0, 0)';
      case TerrainLevelMode.Warning:
        return 'rgb(255, 255, 0)';
      default:
        return 'rgb(0, 255, 0)';
    }
  });

  private readonly lowerBorder = this.minElevationSub.derive((elevation) => {
    if (elevation > 0) {
      return Math.floor(elevation / 100)
        .toString()
        .padStart(3, this.props.paddingText);
    }

    return '';
  });

  private readonly lowerBorderColor = this.minElevationModeSub.derive((mode) => {
    switch (mode as TerrainLevelMode) {
      case TerrainLevelMode.Caution:
        return 'rgb(255, 0, 0)';
      case TerrainLevelMode.Warning:
        return 'rgb(255, 255, 0)';
      default:
        return 'rgb(0, 255, 0)';
    }
  });

  render(): ViewNode | null {
    return (
      <g visibility={this.shown.derive((v) => (v ? 'inherit' : 'hidden'))}>
        <text class="TerrTextLabel" x={688} y={612} font-size={23} fill="rgb(0,255,255)">
          TERR
        </text>

        <text x={709} y={639} font-size={22} fill={this.upperBorderColor}>
          {this.upperBorder}
        </text>

        <rect x={700} y={619} width={54} height={24} strokw-width={3} stroke="rgb(255,255,0)" fill-opacity={0} />
        <text x={709} y={663} font-size={22} fill={this.lowerBorderColor}>
          {this.lowerBorder}
        </text>

        <rect x={700} y={643} width={54} height={24} strokw-width={3} stroke="rgb(255,255,0)" fill-opacity={0} />
      </g>
    );
  }
}

use systems::{
    hydraulic::{
        brake_circuit::{BrakeAccumulatorCharacteristics, BrakeCircuit, BrakeCircuitController},
        linear_actuator::Actuator,
        HydraulicCircuit,
    },
    shared::{HydraulicColor, SectionPressure},
    simulation::{
        InitContext, SimulationElement, SimulationElementVisitor, SimulatorWriter,
        UpdateContext, VariableIdentifier, Write,
    },
};

use uom::si::{f64::*, pressure::psi, ratio::ratio, velocity::knot, volume::gallon};

/// Alternate Refill Valve: open once engines run, latched closed above a ground speed until
/// the engines are shut down again.
#[derive(Clone)]
pub(super) struct AlternateRefillValve {
    closing_speed: Velocity,
    is_open: bool,
    latched_closed: bool,
}
impl AlternateRefillValve {
    pub fn new(closing_speed: Velocity) -> Self {
        Self {
            closing_speed,
            is_open: false,
            latched_closed: false,
        }
    }

    pub fn update(&mut self, context: &UpdateContext, engines_started: bool) {
        if context.ground_speed().abs() > self.closing_speed {
            self.latched_closed = true;
        } else if !engines_started && context.is_on_ground() {
            self.latched_closed = false;
        }

        self.is_open = engines_started && !self.latched_closed;
    }

    pub fn is_open(&self) -> bool {
        self.is_open
    }
}

/// Alternate supply of a consumer: its LEHGS, plus the normal circuit through the ARV.
#[derive(Clone)]
pub(super) struct LocalHydraulicSupply {
    arv: AlternateRefillValve,
    lehgs_pressure: Pressure,
    pressure: Pressure,
}
impl LocalHydraulicSupply {
    const PRESSURE_SWITCH_PSI: f64 = 1750.;

    pub fn new(arv_closing_speed: Velocity, lehgs_pressure: Pressure) -> Self {
        Self {
            arv: AlternateRefillValve::new(arv_closing_speed),
            lehgs_pressure,
            pressure: Pressure::default(),
        }
    }

    pub fn update(
        &mut self,
        context: &UpdateContext,
        normal_section: &impl SectionPressure,
        engines_started: bool,
        lehgs_running: bool,
    ) {
        self.arv.update(context, engines_started);

        let from_lehgs = if lehgs_running {
            self.lehgs_pressure
        } else {
            Pressure::default()
        };
        let from_arv = if self.arv.is_open() {
            normal_section.pressure()
        } else {
            Pressure::default()
        };
        self.pressure = from_lehgs.max(from_arv);
    }

    #[cfg(test)]
    pub fn arv_is_open(&self) -> bool {
        self.arv.is_open()
    }

    /// Fluid goes to the normal circuit while the ARV is open, else to the LEHGS reservoir.
    pub fn route_volumes(&self, normal_circuit: &mut HydraulicCircuit, actuator: &mut impl Actuator) {
        if self.arv.is_open() {
            normal_circuit.update_system_actuator_volumes(actuator);
        } else {
            actuator.reset_volumes();
        }
    }
}
impl SectionPressure for LocalHydraulicSupply {
    fn pressure(&self) -> Pressure {
        self.pressure
    }

    fn pressure_downstream_leak_valve(&self) -> Pressure {
        self.pressure
    }

    fn pressure_downstream_priority_valve(&self) -> Pressure {
        self.pressure
    }

    fn is_pressure_switch_pressurised(&self) -> bool {
        self.pressure.get::<psi>() > Self::PRESSURE_SWITCH_PSI
    }
}

/// Steering LEHGS + NWS accumulator, used when the GREEN supply is lost.
#[derive(Clone)]
pub(super) struct NoseWheelSteeringAlternateSupply {
    supply: LocalHydraulicSupply,
    lehgs_running: bool,
    accumulator_total: Volume,
    accumulator_precharge: Pressure,
    accumulator_fluid: Volume,
}
impl NoseWheelSteeringAlternateSupply {
    // GENERIC sizing: NWS accumulator volume, precharge and charge rate.
    const ACCUMULATOR_TOTAL_GALLON: f64 = 1.;
    const ACCUMULATOR_PRECHARGE_PSI: f64 = 1000.;
    const ACCUMULATOR_CHARGE_FLOW_GALLON_PER_S: f64 = 0.05;
    const ARV_CLOSING_SPEED_KNOT: f64 = 30.;

    pub fn new(lehgs_pressure: Pressure) -> Self {
        let mut supply = Self {
            supply: LocalHydraulicSupply::new(
                Velocity::new::<knot>(Self::ARV_CLOSING_SPEED_KNOT),
                lehgs_pressure,
            ),
            lehgs_running: false,
            accumulator_total: Volume::new::<gallon>(Self::ACCUMULATOR_TOTAL_GALLON),
            accumulator_precharge: Pressure::new::<psi>(Self::ACCUMULATOR_PRECHARGE_PSI),
            accumulator_fluid: Volume::default(),
        };
        supply.accumulator_fluid = supply.fluid_for_pressure(lehgs_pressure);
        supply
    }

    fn fluid_for_pressure(&self, pressure: Pressure) -> Volume {
        if pressure <= self.accumulator_precharge {
            Volume::default()
        } else {
            self.accumulator_total * (1. - (self.accumulator_precharge / pressure).get::<ratio>())
        }
    }

    fn accumulator_pressure(&self) -> Pressure {
        if self.accumulator_fluid <= Volume::default() {
            Pressure::default()
        } else {
            self.accumulator_precharge
                * (self.accumulator_total / (self.accumulator_total - self.accumulator_fluid))
                    .get::<ratio>()
        }
    }

    pub fn update(
        &mut self,
        context: &UpdateContext,
        green_section: &impl SectionPressure,
        all_engines_running: bool,
    ) {
        self.lehgs_running = all_engines_running;
        self.supply
            .update(context, green_section, all_engines_running, all_engines_running);

        let target = self.fluid_for_pressure(self.supply.pressure());
        if target > self.accumulator_fluid {
            let charge = Volume::new::<gallon>(
                Self::ACCUMULATOR_CHARGE_FLOW_GALLON_PER_S * context.delta_as_secs_f64(),
            );
            self.accumulator_fluid = (self.accumulator_fluid + charge).min(target);
        }
    }

    pub fn consume(&mut self, actuator: &mut impl Actuator) {
        if self.supply.pressure() <= Pressure::default() {
            self.accumulator_fluid =
                (self.accumulator_fluid - actuator.used_volume()).max(Volume::default());
        }
        actuator.reset_volumes();
    }

    #[cfg(test)]
    pub fn arv_is_open(&self) -> bool {
        self.supply.arv_is_open()
    }

    #[cfg(test)]
    pub fn accumulator_fluid(&self) -> Volume {
        self.accumulator_fluid
    }
}
impl SectionPressure for NoseWheelSteeringAlternateSupply {
    fn pressure(&self) -> Pressure {
        self.supply.pressure().max(self.accumulator_pressure())
    }

    fn pressure_downstream_leak_valve(&self) -> Pressure {
        self.pressure()
    }

    fn pressure_downstream_priority_valve(&self) -> Pressure {
        self.pressure()
    }

    fn is_pressure_switch_pressurised(&self) -> bool {
        self.pressure().get::<psi>() > LocalHydraulicSupply::PRESSURE_SWITCH_PSI
    }
}

pub(super) struct GearBrakeDemand {
    pub pressure_limit: Pressure,
    pub left: Ratio,
    pub right: Ratio,
}
impl BrakeCircuitController for GearBrakeDemand {
    fn pressure_limit(&self) -> Pressure {
        self.pressure_limit
    }

    fn left_brake_demand(&self) -> Ratio {
        self.left
    }

    fn right_brake_demand(&self) -> Ratio {
        self.right
    }
}

/// Alternate braking with its four isolated accumulators: forward and aft wing gear brakes,
/// left and right body gear brakes. Wing gear LEHGS/ARV refill from GREEN, body gear from YELLOW.
#[derive(Clone)]
pub(super) struct A380AlternateBraking {
    left_press_id: VariableIdentifier,
    right_press_id: VariableIdentifier,
    acc_press_id: VariableIdentifier,

    wing_forward: BrakeCircuit,
    wing_aft: BrakeCircuit,
    body_left: BrakeCircuit,
    body_right: BrakeCircuit,

    wing_supply: LocalHydraulicSupply,
    body_supply: LocalHydraulicSupply,

    body_left_charac: BrakeAccumulatorCharacteristics,
    body_right_charac: BrakeAccumulatorCharacteristics,
}
impl A380AlternateBraking {
    const ARV_CLOSING_SPEED_KNOT: f64 = 65.;
    // GENERIC: per-accumulator sizing kept at FBW's former single accumulator values.
    const ACCUMULATOR_TOTAL_GALLON: f64 = 1.;
    const ACCUMULATOR_PRECHARGE_PSI: f64 = 1000.;
    const EMPTY_AFTER_MAINTENANCE_PROBABILITY: f64 = 0.01;

    const WING_GEAR_FORCE_SHARE_PER_CIRCUIT: f64 = 0.25;
    const BODY_GEAR_FORCE_SHARE: f64 = 0.5;

    pub fn new(context: &mut InitContext, target_pressure: Pressure) -> Self {
        let charac = || {
            BrakeAccumulatorCharacteristics::new(
                Volume::new::<gallon>(Self::ACCUMULATOR_TOTAL_GALLON),
                Pressure::new::<psi>(Self::ACCUMULATOR_PRECHARGE_PSI),
                target_pressure,
                Ratio::new::<ratio>(Self::EMPTY_AFTER_MAINTENANCE_PROBABILITY),
            )
        };
        let body_left_charac = charac();
        let body_right_charac = charac();
        let circuit = |context: &mut InitContext, id: &str, accu: BrakeAccumulatorCharacteristics| {
            BrakeCircuit::new(
                context,
                id,
                HydraulicColor::Yellow,
                Some(systems::hydraulic::Accumulator::new_brake_accumulator(accu)),
                Volume::new::<gallon>(0.13),
            )
        };

        Self {
            left_press_id: context.get_identifier("HYD_BRAKE_ALTN_LEFT_PRESS".to_owned()),
            right_press_id: context.get_identifier("HYD_BRAKE_ALTN_RIGHT_PRESS".to_owned()),
            acc_press_id: context.get_identifier("HYD_BRAKE_ALTN_ACC_PRESS".to_owned()),

            wing_forward: circuit(context, "ALTN_WLG_FWD", charac()),
            wing_aft: circuit(context, "ALTN_WLG_AFT", charac()),
            body_left: circuit(context, "ALTN_BLG_LEFT", body_left_charac.clone()),
            body_right: circuit(context, "ALTN_BLG_RIGHT", body_right_charac.clone()),

            wing_supply: LocalHydraulicSupply::new(
                Velocity::new::<knot>(Self::ARV_CLOSING_SPEED_KNOT),
                target_pressure,
            ),
            body_supply: LocalHydraulicSupply::new(
                Velocity::new::<knot>(Self::ARV_CLOSING_SPEED_KNOT),
                target_pressure,
            ),

            body_left_charac,
            body_right_charac,
        }
    }

    pub fn update(
        &mut self,
        context: &UpdateContext,
        green_section: &impl SectionPressure,
        yellow_section: &impl SectionPressure,
        wing_gear_demand: &impl BrakeCircuitController,
        body_gear_demand: &impl BrakeCircuitController,
        any_engine_running: bool,
    ) {
        self.wing_supply
            .update(context, green_section, any_engine_running, any_engine_running);
        self.body_supply
            .update(context, yellow_section, any_engine_running, any_engine_running);

        self.wing_forward
            .update(context, &self.wing_supply, wing_gear_demand);
        self.wing_aft.update(context, &self.wing_supply, wing_gear_demand);

        let left_only = GearBrakeDemand {
            pressure_limit: body_gear_demand.pressure_limit(),
            left: body_gear_demand.left_brake_demand(),
            right: body_gear_demand.left_brake_demand(),
        };
        let right_only = GearBrakeDemand {
            pressure_limit: body_gear_demand.pressure_limit(),
            left: body_gear_demand.right_brake_demand(),
            right: body_gear_demand.right_brake_demand(),
        };
        self.body_left.update(context, &self.body_supply, &left_only);
        self.body_right.update(context, &self.body_supply, &right_only);
    }

    pub fn route_volumes(&mut self, green: &mut HydraulicCircuit, yellow: &mut HydraulicCircuit) {
        self.wing_supply.route_volumes(green, &mut self.wing_forward);
        self.wing_supply.route_volumes(green, &mut self.wing_aft);
        self.body_supply.route_volumes(yellow, &mut self.body_left);
        self.body_supply.route_volumes(yellow, &mut self.body_right);
    }

    pub fn left_wing_brake_pressure(&self) -> Pressure {
        self.wing_forward
            .left_brake_pressure()
            .max(self.wing_aft.left_brake_pressure())
    }

    pub fn right_wing_brake_pressure(&self) -> Pressure {
        self.wing_forward
            .right_brake_pressure()
            .max(self.wing_aft.right_brake_pressure())
    }

    pub fn left_body_brake_pressure(&self) -> Pressure {
        self.body_left.left_brake_pressure()
    }

    pub fn right_body_brake_pressure(&self) -> Pressure {
        self.body_right.left_brake_pressure()
    }

    fn force(pressure: Pressure, reference: f64) -> f64 {
        50. * pressure.get::<psi>().sqrt() / reference
    }

    pub fn left_force(&self, reference_pressure_psi: f64) -> f64 {
        Self::WING_GEAR_FORCE_SHARE_PER_CIRCUIT
            * (Self::force(self.wing_forward.left_brake_pressure(), reference_pressure_psi)
                + Self::force(self.wing_aft.left_brake_pressure(), reference_pressure_psi))
            + Self::BODY_GEAR_FORCE_SHARE
                * Self::force(self.left_body_brake_pressure(), reference_pressure_psi)
    }

    pub fn right_force(&self, reference_pressure_psi: f64) -> f64 {
        Self::WING_GEAR_FORCE_SHARE_PER_CIRCUIT
            * (Self::force(self.wing_forward.right_brake_pressure(), reference_pressure_psi)
                + Self::force(self.wing_aft.right_brake_pressure(), reference_pressure_psi))
            + Self::BODY_GEAR_FORCE_SHARE
                * Self::force(self.right_body_brake_pressure(), reference_pressure_psi)
    }

    fn accumulator_pressure(
        circuit: &BrakeCircuit,
        charac: &BrakeAccumulatorCharacteristics,
        supply: &LocalHydraulicSupply,
    ) -> Pressure {
        let fluid = circuit.accumulator_fluid_volume();
        if fluid > Volume::default() {
            charac.gas_precharge()
                * (charac.total_volume() / (charac.total_volume() - fluid)).get::<ratio>()
        } else {
            supply.pressure()
        }
    }

    pub fn body_gear_accumulator_pressure(&self) -> Pressure {
        Self::accumulator_pressure(&self.body_left, &self.body_left_charac, &self.body_supply).min(
            Self::accumulator_pressure(&self.body_right, &self.body_right_charac, &self.body_supply),
        )
    }

    #[cfg(test)]
    pub fn body_gear_accumulator_fluid_volume(&self) -> Volume {
        self.body_left
            .accumulator_fluid_volume()
            .min(self.body_right.accumulator_fluid_volume())
    }

    #[cfg(test)]
    pub fn wing_gear_accumulator_fluid_volume(&self) -> Volume {
        self.wing_forward
            .accumulator_fluid_volume()
            .min(self.wing_aft.accumulator_fluid_volume())
    }

    #[cfg(test)]
    pub fn body_arv_is_open(&self) -> bool {
        self.body_supply.arv_is_open()
    }
}
impl SimulationElement for A380AlternateBraking {
    fn accept<T: SimulationElementVisitor>(&mut self, visitor: &mut T) {
        self.wing_forward.accept(visitor);
        self.wing_aft.accept(visitor);
        self.body_left.accept(visitor);
        self.body_right.accept(visitor);
        visitor.visit(self);
    }

    fn write(&self, writer: &mut SimulatorWriter) {
        writer.write(&self.left_press_id, self.left_body_brake_pressure());
        writer.write(&self.right_press_id, self.right_body_brake_pressure());
        writer.write(&self.acc_press_id, self.body_gear_accumulator_pressure());
    }
}

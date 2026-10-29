use crate::Command;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FwsHostInputs {
    pub fws1_failure_active: bool,
    pub fws2_failure_active: bool,
    pub fws_ecp_failure_active: bool,
    pub reset_panel_fws1_pulled: bool,
    pub reset_panel_fws2_pulled: bool,
    pub cpiom_c1_available: bool,
    pub cpiom_c2_available: bool,
    pub dc_ess_powered: bool,
    pub afdx_3_3_reachable: bool,
    pub afdx_13_13_reachable: bool,
    pub afdx_4_4_reachable: bool,
    pub afdx_14_14_reachable: bool,
    pub core_startup_completed: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CoreAction {
    None,
    DestroyAndWarn,
    Create,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FwsHostOutputs {
    pub fws1_failed: bool,
    pub fws2_failed: bool,
    pub fws1_healthy: bool,
    pub fws2_healthy: bool,
    pub ecp_failed: bool,
    pub core_alive: bool,
    pub core_action: CoreAction,
}

pub struct FwsHost {
    core_alive: bool,
    fws1_failed: bool,
    fws2_failed: bool,
    fws1_healthy: bool,
    fws2_healthy: bool,
    ecp_failed: bool,
}

impl FwsHost {
    pub fn new() -> Self {
        Self { core_alive: true, fws1_failed: false, fws2_failed: false, fws1_healthy: false, fws2_healthy: false, ecp_failed: false }
    }

    pub fn init(&self) -> Vec<Command> {
        vec![
            Command::flag("A32NX_FWS1_IS_HEALTHY", false),
            Command::flag("A32NX_FWS2_IS_HEALTHY", false),
            Command::flag("A32NX_FWS_ECP_FAILED", false),
        ]
    }

    pub fn core_alive(&self) -> bool {
        self.core_alive
    }

    pub fn update(&mut self, i: &FwsHostInputs) -> (FwsHostOutputs, Vec<Command>) {
        let mut commands = Vec::new();
        let mut core_action = CoreAction::None;

        let fws1_failed = i.fws1_failure_active || i.reset_panel_fws1_pulled || !i.cpiom_c1_available;
        self.set_failed(1, fws1_failed, &mut core_action);
        let fws2_failed = i.fws2_failure_active || i.reset_panel_fws2_pulled || !i.cpiom_c2_available;
        self.set_failed(2, fws2_failed, &mut core_action);

        let startup_completed = self.core_alive && core_action != CoreAction::Create && i.core_startup_completed;
        let fws1_healthy = !self.fws1_failed && i.cpiom_c1_available && startup_completed;
        let fws2_healthy = !self.fws2_failed && i.cpiom_c2_available && startup_completed;
        if fws1_healthy != self.fws1_healthy {
            self.fws1_healthy = fws1_healthy;
            commands.push(Command::flag("A32NX_FWS1_IS_HEALTHY", fws1_healthy));
        }
        if fws2_healthy != self.fws2_healthy {
            self.fws2_healthy = fws2_healthy;
            commands.push(Command::flag("A32NX_FWS2_IS_HEALTHY", fws2_healthy));
        }

        let ecp_not_reachable =
            !i.afdx_3_3_reachable && !i.afdx_13_13_reachable && !i.afdx_4_4_reachable && !i.afdx_14_14_reachable;
        let ecp_failed = i.fws_ecp_failure_active || !i.dc_ess_powered || ecp_not_reachable;
        if ecp_failed != self.ecp_failed {
            self.ecp_failed = ecp_failed;
            commands.push(Command::flag("A32NX_FWS_ECP_FAILED", ecp_failed));
        }

        let outputs = FwsHostOutputs {
            fws1_failed: self.fws1_failed,
            fws2_failed: self.fws2_failed,
            fws1_healthy: self.fws1_healthy,
            fws2_healthy: self.fws2_healthy,
            ecp_failed: self.ecp_failed,
            core_alive: self.core_alive,
            core_action,
        };
        (outputs, commands)
    }

    fn set_failed(&mut self, which: u8, value: bool, core_action: &mut CoreAction) {
        let before = self.fws1_failed && self.fws2_failed;
        if which == 1 {
            self.fws1_failed = value;
        } else {
            self.fws2_failed = value;
        }
        let after = self.fws1_failed && self.fws2_failed;
        if after != before {
            if after && self.core_alive {
                self.core_alive = false;
                *core_action = CoreAction::DestroyAndWarn;
            } else if !after && !self.core_alive {
                self.core_alive = true;
                *core_action = CoreAction::Create;
            }
        }
    }
}

impl Default for FwsHost {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn healthy_inputs() -> FwsHostInputs {
        FwsHostInputs {
            cpiom_c1_available: true,
            cpiom_c2_available: true,
            dc_ess_powered: true,
            afdx_3_3_reachable: true,
            core_startup_completed: true,
            ..Default::default()
        }
    }

    #[test]
    fn init_writes_all_three_flags_low() {
        let host = FwsHost::new();
        assert_eq!(
            host.init(),
            vec![
                Command::flag("A32NX_FWS1_IS_HEALTHY", false),
                Command::flag("A32NX_FWS2_IS_HEALTHY", false),
                Command::flag("A32NX_FWS_ECP_FAILED", false),
            ]
        );
    }

    #[test]
    fn healthy_once_core_startup_completed_and_writes_only_on_change() {
        let mut host = FwsHost::new();
        let mut i = healthy_inputs();
        i.core_startup_completed = false;
        let (o, c) = host.update(&i);
        assert!(!o.fws1_healthy && !o.fws2_healthy && !o.ecp_failed);
        assert!(c.is_empty());
        i.core_startup_completed = true;
        let (o, c) = host.update(&i);
        assert!(o.fws1_healthy && o.fws2_healthy);
        assert_eq!(c, vec![Command::flag("A32NX_FWS1_IS_HEALTHY", true), Command::flag("A32NX_FWS2_IS_HEALTHY", true)]);
        let (_, c) = host.update(&i);
        assert!(c.is_empty());
    }

    #[test]
    fn unpowered_cpiom_c1_fails_fws1_but_keeps_the_core() {
        let mut host = FwsHost::new();
        let mut i = healthy_inputs();
        host.update(&i);
        i.cpiom_c1_available = false;
        let (o, c) = host.update(&i);
        assert!(o.fws1_failed && !o.fws2_failed);
        assert!(!o.fws1_healthy && o.fws2_healthy);
        assert_eq!(o.core_action, CoreAction::None);
        assert!(o.core_alive);
        assert_eq!(c, vec![Command::flag("A32NX_FWS1_IS_HEALTHY", false)]);
    }

    #[test]
    fn both_failed_destroys_core_and_recovery_recreates_it_unstarted() {
        let mut host = FwsHost::new();
        let mut i = healthy_inputs();
        host.update(&i);
        i.fws1_failure_active = true;
        i.reset_panel_fws2_pulled = true;
        let (o, c) = host.update(&i);
        assert_eq!(o.core_action, CoreAction::DestroyAndWarn);
        assert!(!o.core_alive && !host.core_alive());
        assert!(!o.fws1_healthy && !o.fws2_healthy);
        assert_eq!(c, vec![Command::flag("A32NX_FWS1_IS_HEALTHY", false), Command::flag("A32NX_FWS2_IS_HEALTHY", false)]);
        let (o, c) = host.update(&i);
        assert_eq!(o.core_action, CoreAction::None);
        assert!(c.is_empty());
        i.fws1_failure_active = false;
        let (o, c) = host.update(&i);
        assert_eq!(o.core_action, CoreAction::Create);
        assert!(o.core_alive);
        assert!(o.fws2_failed && !o.fws1_failed);
        assert!(!o.fws1_healthy, "a fresh core has not completed startup");
        assert!(c.is_empty());
        let (o, c) = host.update(&i);
        assert!(o.fws1_healthy);
        assert_eq!(c, vec![Command::flag("A32NX_FWS1_IS_HEALTHY", true)]);
    }

    #[test]
    fn ecp_failed_when_dc_ess_lost_or_all_four_afdx_endpoints_unreachable() {
        let mut host = FwsHost::new();
        let mut i = healthy_inputs();
        host.update(&i);
        i.dc_ess_powered = false;
        let (o, c) = host.update(&i);
        assert!(o.ecp_failed);
        assert_eq!(c, vec![Command::flag("A32NX_FWS_ECP_FAILED", true)]);
        i.dc_ess_powered = true;
        i.afdx_3_3_reachable = false;
        let (o, _) = host.update(&i);
        assert!(o.ecp_failed, "no endpoint reachable");
        i.afdx_14_14_reachable = true;
        let (o, c) = host.update(&i);
        assert!(!o.ecp_failed);
        assert_eq!(c, vec![Command::flag("A32NX_FWS_ECP_FAILED", false)]);
        i.fws_ecp_failure_active = true;
        let (o, _) = host.update(&i);
        assert!(o.ecp_failed);
    }
}

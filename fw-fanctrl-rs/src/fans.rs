use ec_core::{EcCmd, EcCommand, EcError};

/// Parameters for [`SetFanDuty`].
#[repr(C, align(4))]
#[derive(Clone, Copy)]
struct EcParamsPwmSetFanDuty {
    percent: u32,
}

/// Set target fan PWM duty cycle.
struct SetFanDuty;

impl EcCommand for SetFanDuty {
    type Request = EcParamsPwmSetFanDuty;
    type Response = ();
    const CMD: EcCmd = EcCmd::PwmSetFanDuty;
}

/// Hand fan control back to the EC's own thermal loop.
struct ThermalAutoFanCtrl;

impl EcCommand for ThermalAutoFanCtrl {
    type Request = ();
    type Response = ();
    const CMD: EcCmd = EcCmd::ThermalAutoFanCtrl;
}

pub(crate) fn set_duty(percent: u8) -> Result<(), EcError> {
    SetFanDuty::call(EcParamsPwmSetFanDuty {
        percent: u32::from(percent),
    })
}

pub(crate) fn set_auto() -> Result<(), EcError> {
    ThermalAutoFanCtrl::call(())
}

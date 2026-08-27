//! The PD commands this tool declares for itself.
//!
//! Port state, the port count and charge-port selection are shared with
//! `fw-chargemon` and live in [`ec_core::usbpd`]. What is left here is the
//! role-control side of the protocol, which the FW16's EC does **not**
//! implement — `fw-pdctl supported` will show it absent. The declarations are
//! kept because they are what the protocol specifies and another board or a
//! later firmware may back them; nothing else in the workspace should depend
//! on them.
//!
//! Payload layouts mirror the `ec_commands.h` structs of the same name.

use std::fmt;

use ec_core::{EcCmd, EcCommand, EcError};

// ---------------------------------------------------------------------------
// Forcing a port's power role — EC_CMD_USB_PD_CONTROL (0x0101)
// ---------------------------------------------------------------------------

/// The role to put a port into (`enum usb_pd_control_role`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoleRequest {
    /// Leave the role alone. Used to probe whether the command exists at all.
    NoChange,
    /// Resume normal dual-role toggling. This is the resting state.
    Auto,
    /// Stop dual-role toggling, staying in whatever role the port holds.
    ToggleOff,
    /// Pin the port as a sink: it draws power, never supplies it.
    ForceSink,
    /// Pin the port as a source: it supplies power from the battery.
    ForceSource,
    /// Freeze the port's state machine where it is.
    Freeze,
}

impl RoleRequest {
    /// The wire value for this role.
    #[must_use]
    pub const fn as_raw(self) -> u8 {
        match self {
            Self::NoChange => 0,
            Self::Auto => 1,
            Self::ToggleOff => 2,
            Self::ForceSink => 3,
            Self::ForceSource => 4,
            Self::Freeze => 5,
        }
    }

    /// Whether applying this role can take away power the system is drawing.
    ///
    /// Forcing sink, or leaving the role alone, never does.
    #[must_use]
    pub const fn can_drop_input(self) -> bool {
        !matches!(self, Self::NoChange | Self::ForceSink)
    }
}

impl fmt::Display for RoleRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::NoChange => "no-change",
            Self::Auto => "auto",
            Self::ToggleOff => "toggle-off",
            Self::ForceSink => "sink",
            Self::ForceSource => "source",
            Self::Freeze => "freeze",
        })
    }
}

/// `USB_PD_CTRL_MUX_NO_CHANGE` — this tool never touches the mux.
const MUX_NO_CHANGE: u8 = 0;
/// `USB_PD_CTRL_SWAP_NONE` — this tool never requests a swap.
const SWAP_NONE: u8 = 0;

#[repr(C, packed)]
#[derive(Clone, Copy)]
struct EcParamsUsbPdControl {
    port: u8,
    role: u8,
    mux: u8,
    swap: u8,
}

/// Version 0 of `struct ec_response_usb_pd_control`. Later versions return
/// more, so this deliberately stays on v0.
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct EcResponseUsbPdControl {
    /// Non-zero if PD is enabled on the port.
    pub enabled: u8,
    /// `enum pd_power_role` as the port now holds it.
    pub role: u8,
    /// CC polarity.
    pub polarity: u8,
    /// PD state-machine index.
    pub state: u8,
}

/// Set USB type-C port role and muxes.
///
/// **Not implemented on this EC** (`EC_CMD_USB_PD_CONTROL`, 0x0101).
///
/// `ec_commands.h` marks this deprecated in favour of `TYPEC_STATUS` /
/// `TYPEC_CONTROL`, but those cover the mux, alternate modes and VDMs rather
/// than the power role. On the FW16 none of the three is implemented.
struct UsbPdControl;

impl EcCommand for UsbPdControl {
    type Request = EcParamsUsbPdControl;
    type Response = EcResponseUsbPdControl;
    const CMD: EcCmd = EcCmd::UsbPdControl;
}

/// Ask the EC to put `port` into `role`, leaving the mux alone.
///
/// # Errors
///
/// Fails if the EC is unreachable or refuses the command. On hardware that
/// does not implement role control — the FW16 included — this is a `Rejected`
/// carrying `EC_RES_INVALID_COMMAND`.
pub fn set_role(port: u8, role: RoleRequest) -> Result<EcResponseUsbPdControl, EcError> {
    UsbPdControl::call(EcParamsUsbPdControl {
        port,
        role: role.as_raw(),
        mux: MUX_NO_CHANGE,
        swap: SWAP_NONE,
    })
}

// ---------------------------------------------------------------------------
// Static per-port capabilities — EC_CMD_GET_PD_PORT_CAPS (0x0128)
// ---------------------------------------------------------------------------

#[repr(C, packed)]
#[derive(Clone, Copy)]
struct EcParamsGetPdPortCaps {
    port: u8,
}

/// `struct ec_response_get_pd_port_caps`.
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct EcResponseGetPdPortCaps {
    /// `enum ec_pd_power_role_caps`: 0 source, 1 sink, 2 dual.
    pub power_role_cap: u8,
    /// `enum ec_pd_try_power_role_caps`: 0 none, 1 sink, 2 source.
    pub try_power_role_cap: u8,
    /// `enum ec_pd_data_role_caps`: 0 DFP, 1 UFP, 2 dual.
    pub data_role_cap: u8,
    /// `enum ec_pd_port_location`.
    pub port_location: u8,
}

/// Static capabilities of a port: power role, try-power role, and data role.
///
/// **Not implemented on this EC** (`EC_CMD_GET_PD_PORT_CAPS`, 0x0128).
struct GetPdPortCaps;

impl EcCommand for GetPdPortCaps {
    type Request = EcParamsGetPdPortCaps;
    type Response = EcResponseGetPdPortCaps;
    const CMD: EcCmd = EcCmd::GetPdPortCaps;
}

/// Read one port's static role capabilities.
///
/// # Errors
///
/// Fails if the EC is unreachable or refuses the command. Not implemented on
/// the FW16.
pub fn port_caps(port: u8) -> Result<EcResponseGetPdPortCaps, EcError> {
    GetPdPortCaps::call(EcParamsGetPdPortCaps { port })
}

/// Name for a raw `enum ec_pd_power_role_caps` byte.
#[must_use]
pub const fn power_role_cap_name(raw: u8) -> Option<&'static str> {
    Some(match raw {
        0 => "source-only",
        1 => "sink-only",
        2 => "dual-role",
        _ => return None,
    })
}

/// Name for a raw `enum ec_pd_try_power_role_caps` byte.
#[must_use]
pub const fn try_power_role_cap_name(raw: u8) -> Option<&'static str> {
    Some(match raw {
        0 => "none",
        1 => "try-sink",
        2 => "try-source",
        _ => return None,
    })
}

/// Name for a raw `enum ec_pd_data_role_caps` byte.
#[must_use]
pub const fn data_role_cap_name(raw: u8) -> Option<&'static str> {
    Some(match raw {
        0 => "DFP",
        1 => "UFP",
        2 => "dual-role",
        _ => return None,
    })
}

// ---------------------------------------------------------------------------
// What to report on
// ---------------------------------------------------------------------------

/// The PD-related commands `fw-pdctl supported` asks the EC about, in numeric
/// order.
pub const PD_COMMANDS: &[(EcCmd, &str)] = &[
    (EcCmd::UsbChargeSetMode, "set USB port charging mode"),
    (EcCmd::PdExchangeStatus, "EC/PD MCU status exchange"),
    (EcCmd::UsbPdControl, "set port power role (source/sink)"),
    (EcCmd::UsbPdPorts, "number of PD ports"),
    (EcCmd::UsbPdPowerInfo, "per-port negotiated power"),
    (EcCmd::ChargePortCount, "number of charge ports"),
    (EcCmd::PdChargePortOverride, "choose the charging port"),
    (EcCmd::PdControl, "control the PD chip"),
    (EcCmd::UsbPdMuxInfo, "USB-C SS mux state"),
    (EcCmd::PdChipInfo, "PD chip identification"),
    (EcCmd::GetPdPortCaps, "static per-port role capabilities"),
    (EcCmd::TypecControl, "AP-controlled type-C device policy"),
    (EcCmd::TypecStatus, "per-port type-C status"),
];

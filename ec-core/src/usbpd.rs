//! USB-PD port state and charge-port selection.
//!
//! These commands are shared by more than one crate in the workspace, and every
//! one of them is verified present on the FW16's EC — see
//! [`introspect`](crate::introspect) for how to check that on another machine.
//! Commands that this hardware does not implement stay in the crate that wants
//! them rather than living here.
//!
//! Fields the EC header declares as a `__packed` enum are carried as a raw `u8`
//! and decoded with a checked `from_raw`, so an EC reporting a value this build
//! does not know about is a printable unknown rather than an invalid enum.

use std::fmt;

use crate::command::EcCommand;
use crate::ec_cmd::EcCmd;
use crate::error::EcError;

/// Most ports the protocol can describe (`EC_USB_PD_MAX_PORTS`).
pub const EC_USB_PD_MAX_PORTS: u8 = 8;

// ---------------------------------------------------------------------------
// How many ports there are
// ---------------------------------------------------------------------------

/// `struct ec_response_charge_port_count`.
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct EcResponseChargePortCount {
    /// Number of charge ports + number of dedicated ports present.
    pub port_count: u8,
}

/// Number of USB-PD charge ports plus the number of dedicated ports present.
pub struct ChargePortCount;

impl EcCommand for ChargePortCount {
    type Request = ();
    type Response = EcResponseChargePortCount;
    const CMD: EcCmd = EcCmd::ChargePortCount;
}

/// How many ports the EC will answer questions about.
///
/// `EC_CMD_USB_PD_PORTS` is the more obvious command for this, but it is not
/// implemented on the FW16 — it reports 0 — so this uses the charge-port count,
/// which is.
///
/// # Errors
///
/// Fails if the EC is unreachable or refuses the command.
pub fn charge_port_count() -> Result<u8, EcError> {
    Ok(ChargePortCount::call(())?
        .port_count
        .min(EC_USB_PD_MAX_PORTS))
}

// ---------------------------------------------------------------------------
// What a port is doing
// ---------------------------------------------------------------------------

/// Which way power is flowing on a port (`enum usb_power_roles`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PowerRole {
    /// Nothing attached.
    Disconnected,
    /// The port is supplying power.
    Source,
    /// The port is drawing power, and charging the system.
    Sink,
    /// The port is drawing power but not charging.
    SinkNotCharging,
}

impl PowerRole {
    /// The role for a raw `enum usb_power_roles` byte, if it is one we know.
    #[must_use]
    pub const fn from_raw(raw: u8) -> Option<Self> {
        Some(match raw {
            0 => Self::Disconnected,
            1 => Self::Source,
            2 => Self::Sink,
            3 => Self::SinkNotCharging,
            _ => return None,
        })
    }

    /// Whether power is coming into the system through this port.
    ///
    /// Both sink roles count: `SinkNotCharging` still means the port is the
    /// one holding the machine up.
    #[must_use]
    pub const fn is_drawing_power(self) -> bool {
        matches!(self, Self::Sink | Self::SinkNotCharging)
    }
}

impl fmt::Display for PowerRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Disconnected => "Disconnected",
            Self::Source => "Source",
            Self::Sink => "Sink",
            Self::SinkNotCharging => "SinkNotCharging",
        })
    }
}

/// What kind of charger is attached (`enum usb_chg_type`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ChargeType {
    None,
    Pd,
    TypeC,
    Proprietary,
    Bc12Dcp,
    Bc12Cdp,
    Bc12Sdp,
    Other,
    Vbus,
    Unknown,
    Dedicated,
}

impl ChargeType {
    /// The type for a raw `enum usb_chg_type` byte, if it is one we know.
    #[must_use]
    pub const fn from_raw(raw: u8) -> Option<Self> {
        Some(match raw {
            0 => Self::None,
            1 => Self::Pd,
            2 => Self::TypeC,
            3 => Self::Proprietary,
            4 => Self::Bc12Dcp,
            5 => Self::Bc12Cdp,
            6 => Self::Bc12Sdp,
            7 => Self::Other,
            8 => Self::Vbus,
            9 => Self::Unknown,
            10 => Self::Dedicated,
            _ => return None,
        })
    }
}

impl fmt::Display for ChargeType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::None => "None",
            Self::Pd => "PD",
            Self::TypeC => "Type-C",
            Self::Proprietary => "Proprietary",
            Self::Bc12Dcp => "BC 1.2 DCP",
            Self::Bc12Cdp => "BC 1.2 CDP",
            Self::Bc12Sdp => "BC 1.2 SDP",
            Self::Other => "Other",
            Self::Vbus => "VBUS",
            Self::Unknown => "Unknown",
            Self::Dedicated => "Dedicated",
        })
    }
}

/// `struct ec_params_usb_pd_power_info`.
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct EcParamsUsbPdPowerInfo {
    /// USB-C port number.
    pub port: u8,
}

/// `struct ec_response_usb_pd_power_info`, with the nested `usb_chg_measures`
/// flattened into it.
///
/// The two leading enums are `__packed` in the EC header, so each occupies one
/// byte and the whole struct is 16.
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct EcResponseUsbPdPowerInfo {
    /// `enum usb_power_roles`; decode with [`PowerRole::from_raw`].
    pub role: u8,
    /// `enum usb_chg_type`; decode with [`ChargeType::from_raw`].
    pub charge_type: u8,
    /// Non-zero if the attached partner is dual-role capable.
    pub dualrole: u8,
    pub reserved1: u8,
    /// Maximum voltage advertised (mV).
    pub voltage_max: u16,
    /// Present voltage (mV).
    pub voltage_now: u16,
    /// Maximum current advertised (mA).
    pub current_max: u16,
    /// Negotiated current limit (mA) — the lesser of the sink's request and
    /// the source's maximum. Named `current_lim` in `ec_commands.h`; it is not
    /// an instantaneous draw.
    pub current_lim: u16,
    /// Maximum power (microwatts).
    pub max_power: u32,
}

impl EcResponseUsbPdPowerInfo {
    /// The decoded power role, if the EC reported one we know.
    #[must_use]
    pub const fn power_role(&self) -> Option<PowerRole> {
        PowerRole::from_raw(self.role)
    }

    /// The decoded charger type, if the EC reported one we know.
    #[must_use]
    pub const fn charger_type(&self) -> Option<ChargeType> {
        ChargeType::from_raw(self.charge_type)
    }

    /// Whether this port is the one the EC selected to charge the system.
    ///
    /// Deliberately narrower than [`PowerRole::is_drawing_power`]: exactly one
    /// port reports [`Sink`](PowerRole::Sink) once the EC has chosen where to
    /// draw from, and any *other* port with a charger attached reports
    /// [`SinkNotCharging`](PowerRole::SinkNotCharging). Treating the two alike
    /// picks the wrong port whenever two chargers are plugged in.
    #[must_use]
    pub const fn is_active_charger(&self) -> bool {
        matches!(self.power_role(), Some(PowerRole::Sink))
    }
}

/// Get power information about a USB-PD port.
pub struct UsbPdPowerInfo;

impl EcCommand for UsbPdPowerInfo {
    type Request = EcParamsUsbPdPowerInfo;
    type Response = EcResponseUsbPdPowerInfo;
    const CMD: EcCmd = EcCmd::UsbPdPowerInfo;
}

/// Read one port's negotiated power state.
///
/// # Errors
///
/// Fails if the EC is unreachable or refuses the command.
pub fn power_info(port: u8) -> Result<EcResponseUsbPdPowerInfo, EcError> {
    UsbPdPowerInfo::call(EcParamsUsbPdPowerInfo { port })
}

// ---------------------------------------------------------------------------
// Which port charges the system
// ---------------------------------------------------------------------------

/// `enum usb_pd_override_ports`: refuse to charge from any port.
pub const OVERRIDE_DONT_CHARGE: i16 = -2;
/// `enum usb_pd_override_ports`: hand port selection back to the EC.
pub const OVERRIDE_OFF: i16 = -1;

/// `struct ec_params_charge_port_override`.
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct EcParamsChargePortOverride {
    /// A port index, [`OVERRIDE_OFF`], or [`OVERRIDE_DONT_CHARGE`].
    pub override_port: i16,
}

/// Override the EC's default charge-port selection.
pub struct PdChargePortOverride;

impl EcCommand for PdChargePortOverride {
    type Request = EcParamsChargePortOverride;
    type Response = ();
    const CMD: EcCmd = EcCmd::PdChargePortOverride;
}

/// Force the system to charge from `override_port`, or pass one of the
/// sentinels [`OVERRIDE_OFF`] / [`OVERRIDE_DONT_CHARGE`].
///
/// This is the only power-direction control the FW16's EC implements: it picks
/// which port power comes *in* through. There is no counterpart for making a
/// port supply power on demand.
///
/// # Errors
///
/// Fails if the EC is unreachable or refuses the command.
pub fn set_charge_override(override_port: i16) -> Result<(), EcError> {
    PdChargePortOverride::call(EcParamsChargePortOverride { override_port })
}

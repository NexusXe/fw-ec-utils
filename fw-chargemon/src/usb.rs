use std::{fmt, sync::LazyLock};

use ec_core::common::{
    CrosEcBidirectionalCommand, CrosEcCommandV2, CrosEcPayload, EcCmd, FullWriteV2Command, fire,
};

#[repr(C)]
enum UsbChargeMode {
    /// Disable USB port.
    Disabled,
    /// Set USB port to Standard Downstream Port, USB 2.0 mode.
    Sdp2,
    /// Set USB port to Charging Downstream Port, BC 1.2.
    Cdp,
    /// Set USB port to Dedicated Charging Port, BC 1.2.
    DcpShort,
    /// Enable USB port (for dumb ports).
    Enabled,
    /// Set USB port to `CONFIG_USB_PORT_POWER_SMART_DEFAULT_MODE`.
    Default,
    /// Number of USB charge modes.
    Count,
}

#[repr(C)]
enum PdChargeState {
    /// Don't change charging state
    NoChange = 0,
    /// No charging allowed
    None,
    /// 5V charging only
    FiveV,
    /// Charge at max voltage,
    Max,
}

#[repr(C, packed)]
struct EcParamsPdStatus {
    /// EC status
    status: u8,
    /// battery state of charge
    batt_soc: i8,
    /// charging state (from enum [`PdChargeState`])
    pd_charge_state: u8,
}

/// Status of PD being sent back to EC
#[repr(C)]
enum PdStatus {
    /// Forward host event to AP
    HostEvent = 1 << 0,
    /// Running RW image
    InRw = 1 << 1,
    /// Current image was jumped to
    JumpedToImage = 1 << 2,
    /// Alert active in port 0 TCPC
    TcpcAlert0 = 1 << 3,
    /// Alert active in port 1 TCPC
    TcpcAlert1 = 1 << 4,
    /// Alert active in port 2 TCPC
    TcpcAlert2 = 1 << 5,
    /// Alert active in port 3 TCPC
    TcpcAlert3 = 1 << 6,
    EcIntActive =
        (Self::TcpcAlert0 as isize | Self::TcpcAlert1 as isize | Self::HostEvent as isize),
}

#[repr(C, packed)]
struct EcResponsePdStatus {
    /// input current limit
    curr_lim_ma: u32,
    /// PD MCU status
    status: u16,
    /// active charging port
    active_charge_port: i8,
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
struct EcParamsUsbPdPowerInfo {
    port: u8,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy)]
pub(crate) enum UsbChgType {
    None,
    Pd,
    C,
    Proprietary,
    Bc12Dcp,
    Bc12Cdp,
    Bc12Sdp,
    Other,
    Vbus,
    Unknown,
    Dedicated,
}

impl fmt::Display for UsbChgType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::None => "None",
                Self::Pd => "PD",
                Self::C => "C",
                Self::Proprietary => "Proprietary",
                Self::Bc12Dcp => "BC 1.2 DCP",
                Self::Bc12Cdp => "BC 1.2 CDP",
                Self::Bc12Sdp => "BC 1.2 SDP",
                Self::Other => "Other",
                Self::Vbus => "VBUS",
                Self::Unknown => "Unknown",
                Self::Dedicated => "Dedicated",
            }
        )
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UsbPowerRoles {
    Disconnected,
    Source,
    Sink,
    SinkNotCharging,
}

impl fmt::Display for UsbPowerRoles {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Disconnected => "Disconnected",
                Self::Source => "Source",
                Self::Sink => "Sink",
                Self::SinkNotCharging => "SinkNotCharging",
            }
        )
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub(crate) struct UsbChgMeasures {
    /// Voltage in mV
    voltage_max: u16,
    /// Voltage in mV
    voltage_now: u16,
    /// Current in mA
    current_max: u16,
    /// Current in mA
    current_now: u16,
}

impl fmt::Display for UsbChgMeasures {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Voltage: {}mV/{}mV, Current: {}mA/{}mA",
            self.voltage_now, self.voltage_max, self.current_now, self.current_max
        )
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub(crate) struct EcResponseUsbPdPowerInfo {
    role: UsbPowerRoles,
    r#type: UsbChgType,
    dualrole: u8,
    reserved1: u8,
    meas: UsbChgMeasures,
    /// Power in microwatts
    max_power: u32,
}

impl EcResponseUsbPdPowerInfo {
    pub(crate) fn is_active_charger(&self) -> bool {
        self.role == UsbPowerRoles::Sink || self.role == UsbPowerRoles::SinkNotCharging
    }
}

impl fmt::Display for EcResponseUsbPdPowerInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // write with padding
        let max_power_whole = self.max_power / 1000000;
        let max_power_decimal = self.max_power % 1000000;
        write!(
            f,
            "Role: {}, Type: {}, Dualrole: {}, Reserved1: {}, Measurements: {{{}}}, Max Power: {}{} W",
            self.role,
            self.r#type,
            self.dualrole,
            self.reserved1,
            self.meas,
            max_power_whole,
            if max_power_decimal != 0 {
                format!(".{:06}", max_power_decimal)
            } else {
                "".to_string()
            }
        )
    }
}

/// Number of charge ports + number of dedicated ports present
#[repr(C, packed)]
pub struct EcResponseChargePortCount {
    pub port_count: u8,
}

/// Maximum number of PD ports on a device, num_ports will be <= this
#[allow(unused)]
const EC_USB_PD_MAX_PORTS: usize = 8;

/// Number of PD ports present. Does not include dedicated ports.
#[repr(C, packed)]
pub struct EcResponseUsbPdPorts {
    pub num_ports: u8,
}

/// Get number of charging ports + number of dedicated ports present.
/// Used in lieu of [`get_usb_pd_ports`], because for some reason on my FW16
/// that always returns 0.
pub fn get_charge_port_count() -> Result<u8, Box<dyn std::error::Error + Send + Sync>> {
    type GetUsbPdPortsCommand = FullWriteV2Command<EcResponseChargePortCount>;

    let mut cmd = GetUsbPdPortsCommand {
        header: CrosEcCommandV2 {
            command: EcCmd::ChargePortCount as u32,
            // No params sent to EC
            outsize: 0,
            // EC writes back an EcResponseUsbPdPorts
            insize: std::mem::size_of::<EcResponseChargePortCount>() as u32,
            ..
        },
        // EC will write the response here
        payload: EcResponseChargePortCount { port_count: 0 },
    };
    unsafe { fire(&raw mut cmd.header) }?;
    Ok(cmd.payload.port_count)
}

/// Number of charging ports + number of dedicated ports present
pub static CHARGE_PORT_COUNT: LazyLock<Result<u8, Box<dyn std::error::Error + Send + Sync>>> =
    LazyLock::new(get_charge_port_count);

pub(crate) fn get_port_pd_info(
    idx: u8,
) -> Result<EcResponseUsbPdPowerInfo, Box<dyn std::error::Error + Send + Sync>> {
    let num_ports = *CHARGE_PORT_COUNT.as_ref().map_err(|e| e.to_string())?;

    // Verify sane port number
    if !(0..num_ports).contains(&idx) {
        return Err(format!("Port number {idx} not within range 0..{num_ports}").into());
    }

    // bidirectional command
    let mut cmd = CrosEcBidirectionalCommand::<EcParamsUsbPdPowerInfo, EcResponseUsbPdPowerInfo> {
        header: CrosEcCommandV2 {
            command: EcCmd::UsbPdPowerInfo as u32,
            outsize: std::mem::size_of::<EcParamsUsbPdPowerInfo>() as u32,
            insize: std::mem::size_of::<EcResponseUsbPdPowerInfo>() as u32,
            ..
        },
        payload: CrosEcPayload {
            req: EcParamsUsbPdPowerInfo { port: idx },
        },
    };

    unsafe { fire(&raw mut cmd.header) }?;

    if cmd.header.result != 0 {
        return Err(format!("EC error: {:}", cmd.header.result).into());
    }

    let response = unsafe { cmd.payload.res };

    Ok(response)
}

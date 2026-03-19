use std::sync::LazyLock;

use ec_core::common::{CrosEcCommandV2, EcCmd, FullWriteV2Command, fire};

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
struct EcParamsUsbPdPowerInfo {
    port: u8,
}

#[repr(C)]
enum UsbChgType {
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

#[repr(C)]
struct UsbChgMeasures {
    voltage_max: u16,
    voltage_now: u16,
    current_max: u16,
    current_now: u16,
}

#[repr(C)]
struct EcResponseUsbPdPowerInfo {
    role: u8,
    r#type: u8,
    dualrole: u8,
    reserved1: u8,
    meas: UsbChgMeasures,
    max_power: u32,
}

/// Number of charge ports + number of dedicated ports present
#[repr(C, packed)]
pub struct EcResponseChargePortCount {
    pub port_count: u8,
}

/// Maximum number of PD ports on a device, num_ports will be <= this
const EC_USB_PD_MAX_PORTS: usize = 8;

/// Number of PD ports present. Does not include dedicated ports.
#[repr(C, packed)]
pub struct EcResponseUsbPdPorts {
    pub num_ports: u8,
}

type GetUsbPdPortsCommand = FullWriteV2Command<EcResponseChargePortCount>;

/// Get number of charging ports + number of dedicated ports present.
/// Used in lieu of [`get_usb_pd_ports`], because for some reason on my FW16
/// that always returns 0.
pub fn get_charge_port_count() -> Result<u8, Box<dyn std::error::Error + Send + Sync>> {
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
    LazyLock::new(|| get_charge_port_count());

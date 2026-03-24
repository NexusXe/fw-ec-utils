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
struct EcResponseChargePortCount {
    pub port_count: u8,
}

/// Maximum number of PD ports on a device, num_ports will be <= this
const EC_USB_PD_MAX_PORTS: usize = 8;

/// Number of PD ports present. Does not include dedicated ports.
#[repr(C, packed)]
struct EcResponseUsbPdPorts {
    pub num_ports: u8,
}

/// Get number of USB PD ports.
/// Always returns 0 on my FW16.
pub(crate) fn get_usb_pd_ports() -> Result<u8, Box<dyn std::error::Error + Send + Sync>> {
    type GetUsbPdPortsCommand = FullWriteV2Command<EcResponseUsbPdPorts>;

    let mut cmd = GetUsbPdPortsCommand {
        header: CrosEcCommandV2 {
            command: EcCmd::UsbPdPorts as u32,
            // No params sent to EC
            outsize: 0,
            // EC writes back an EcResponseUsbPdPorts
            insize: std::mem::size_of::<EcResponseUsbPdPorts>() as u32,
            ..
        },
        // EC will write the response here
        payload: EcResponseUsbPdPorts { num_ports: 0 },
    };
    unsafe { fire(&raw mut cmd.header) }?;
    Ok(cmd.payload.num_ports)
}

/// Get number of charging ports + number of dedicated ports present.
/// Used in lieu of [`get_usb_pd_ports`], because for some reason on my FW16
/// that always returns 0.
pub(crate) fn get_charge_port_count() -> Result<u8, Box<dyn std::error::Error + Send + Sync>> {
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

/// Get info about USB-C SS muxes
#[repr(C, packed)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct EcParamsUsbPdMuxInfo {
    /// USB-C port number
    port: u8,
}

/// Helper struct for USB_PD_MU flags
#[repr(C, packed)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct UsbPdMuxFlags(u8);

impl fmt::Display for UsbPdMuxFlags {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        const USB_PD_MUX_FLAGS: [&str; 8] = [
            "USB_PD_MUX_USB_ENABLED",        // USB connected
            "USB_PD_MUX_DP_ENABLED",         // DP connected
            "USB_PD_MUX_POLARITY_INVERTED",  // CC line Polarity inverted
            "USB_PD_MUX_HPD_IRQ",            // HPD IRQ is asserted
            "USB_PD_MUX_HPD_LVL",            // HPD level is asserted
            "USB_PD_MUX_SAFE_MODE",          // DP is in safe mode
            "USB_PD_MUX_TBT_COMPAT_ENABLED", // TBT compat enabled
            "USB_PD_MUX_USB4_ENABLED",       // USB4 enabled
        ];

        if self.0 == 0 {
            write!(f, "USB_PD_MUX_NONE")
        } else {
            (0..u8::BITS)
                .filter(|&i| (self.0 >> i) & 1 == 1)
                .try_for_each(|i| {
                    write!(
                        f,
                        "{}{}",
                        if i > 0 { ", " } else { "" },
                        USB_PD_MUX_FLAGS[i as usize]
                    )
                })?;

            // USB_PD_MUX_DOCK = USB_PD_MUX_USB_ENABLED | USB_PD_MUX_DP_ENABLED
            if (self.0 & (0b1 | 0b10)) != 0 {
                write!(f, ", USB_PD_MUX_DOCK")?;
            }

            Ok(())
        }
    }
}

#[repr(C, packed)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct EcResponseUsbPdMuxInfo {
    flags: UsbPdMuxFlags,
}
// struct ec_response_pd_chip_info {
// 	uint16_t vendor_id;
// 	uint16_t product_id;
// 	uint16_t device_id;
// 	union {
// 		uint8_t fw_version_string[8];
// 		uint64_t fw_version_number;
// 	};
// } __ec_align2;

// struct ec_response_pd_chip_info_v1 {
// 	uint16_t vendor_id;
// 	uint16_t product_id;
// 	uint16_t device_id;
// 	union {
// 		uint8_t fw_version_string[8];
// 		uint64_t fw_version_number;
// 	};
// 	union {
// 		uint8_t min_req_fw_version_string[8];
// 		uint64_t min_req_fw_version_number;
// 	};
// } __ec_align2;

#[repr(u8)]
#[derive(Debug, Clone, Copy)]
enum EcPdChipInfoLive {
    Hardcoded = 0,
    Live = 1,
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
struct EcParamsPdChipInfo {
    /// USB-C port number
    port: u8,
    /// Fetch the live chip info or hard-coded + cached chip info
    /// 0: hardcoded value for VID/PID, cached value for FW version
    /// 1: live chip value for VID/PID/FW Version
    live: EcPdChipInfoLive,
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub(crate) struct EcResponsePdChipInfo {
    vendor_id: u16,
    product_id: u16,
    device_id: u16,
    fw_version_number: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
union FwVersion {
    fw_version_string: [u8; 8],
    fw_version_number: u64,
}

impl fmt::Display for FwVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // just print out both; try to consume the string as a null-terminated cstring
        let string =
            unsafe { std::ffi::CStr::from_ptr(self.fw_version_string.as_ptr() as *const i8) };
        let number = unsafe { self.fw_version_number };
        write!(f, "String: {:?}, Number: {number}", string)
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
union MinReqFwVersion {
    min_req_fw_version_string: [u8; 8],
    min_req_fw_version_number: u64,
}

impl fmt::Display for MinReqFwVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // just print out both; try to consume the string as a null-terminated cstring
        let string = unsafe {
            std::ffi::CStr::from_ptr(self.min_req_fw_version_string.as_ptr() as *const i8)
        };
        let number = unsafe { self.min_req_fw_version_number };
        write!(f, "String: {:?}, Number: {number}", string)
    }
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub(crate) struct EcResponsePdChipInfoV1 {
    vendor_id: u16,
    product_id: u16,
    device_id: u16,
    fw_version: FwVersion,
    min_req_fw_version: MinReqFwVersion,
}

impl fmt::Display for EcResponsePdChipInfoV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let vendor_id = self.vendor_id;
        let product_id = self.product_id;
        let device_id = self.device_id;
        let fw_version = self.fw_version;
        let min_req_fw_version = self.min_req_fw_version;
        write!(
            f,
            "Vendor ID: {:#06x}, Product ID: {:#06x}, Device ID: {:#06x}, FW Version: {}, Min Req FW Version: {}",
            vendor_id, product_id, device_id, fw_version, min_req_fw_version
        )
    }
}

pub(crate) fn get_pd_chip_info(
    idx: u8,
) -> Result<EcResponsePdChipInfo, Box<dyn std::error::Error + Send + Sync>> {
    let num_ports = *CHARGE_PORT_COUNT.as_ref().map_err(|e| e.to_string())?;

    if !(0..num_ports).contains(&idx) {
        return Err(format!("Port number {idx} not within range 0..{num_ports}").into());
    }

    let mut cmd = CrosEcBidirectionalCommand::<EcParamsPdChipInfo, EcResponsePdChipInfo> {
        header: CrosEcCommandV2 {
            version: u32::MAX,
            command: EcCmd::PdChipInfo as u32,
            outsize: std::mem::size_of::<EcParamsPdChipInfo>() as u32,
            insize: std::mem::size_of::<EcResponsePdChipInfo>() as u32,
            ..
        },
        payload: CrosEcPayload {
            req: EcParamsPdChipInfo {
                port: idx,
                live: EcPdChipInfoLive::Live,
            },
        },
    };

    unsafe { fire(&raw mut cmd.header) }?;

    if cmd.header.result != 0 {
        return Err(format!("EC error: {:}", cmd.header.result).into());
    }

    let response = unsafe { cmd.payload.res };

    Ok(response)
}

pub(crate) fn get_pd_chip_info_v1(
    idx: u8,
) -> Result<EcResponsePdChipInfoV1, Box<dyn std::error::Error + Send + Sync>> {
    let num_ports = *CHARGE_PORT_COUNT.as_ref().map_err(|e| e.to_string())?;

    if !(0..num_ports).contains(&idx) {
        return Err(format!("Port number {idx} not within range 0..{num_ports}").into());
    }

    let mut cmd = CrosEcBidirectionalCommand::<EcParamsPdChipInfo, EcResponsePdChipInfoV1> {
        header: CrosEcCommandV2 {
            version: 1,
            command: EcCmd::PdChipInfo as u32,
            outsize: std::mem::size_of::<EcParamsPdChipInfo>() as u32,
            insize: std::mem::size_of::<EcResponsePdChipInfoV1>() as u32,
            ..
        },
        payload: CrosEcPayload {
            req: EcParamsPdChipInfo {
                port: idx,
                live: EcPdChipInfoLive::Hardcoded,
            },
        },
    };

    unsafe { fire(&raw mut cmd.header) }?;

    if cmd.header.result != 0 {
        return Err(format!("EC error: {:}", cmd.header.result).into());
    }

    let response = unsafe { cmd.payload.res };

    Ok(response)
}

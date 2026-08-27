use std::{fmt, sync::LazyLock};

use ec_core::{EcCmd, EcCommand, EcError};

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
    pub(crate) voltage_max: u16,
    /// Voltage in mV
    pub(crate) voltage_now: u16,
    /// Current in mA
    pub(crate) current_max: u16,
    /// Current in mA
    pub(crate) current_now: u16,
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
    pub(crate) role: UsbPowerRoles,
    pub(crate) r#type: UsbChgType,
    pub(crate) dualrole: u8,
    pub(crate) reserved1: u8,
    pub(crate) meas: UsbChgMeasures,
    /// Power in microwatts
    pub(crate) max_power: u32,
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
#[derive(Clone, Copy)]
struct EcResponseChargePortCount {
    pub port_count: u8,
}

/// Maximum number of PD ports on a device, num_ports will be <= this
const EC_USB_PD_MAX_PORTS: usize = 8;

/// Number of PD ports present. Does not include dedicated ports.
#[repr(C, packed)]
#[derive(Clone, Copy)]
struct EcResponseUsbPdPorts {
    pub num_ports: u8,
}

/// Number of PD ports present. Does not include dedicated ports.
struct UsbPdPorts;

impl EcCommand for UsbPdPorts {
    type Request = ();
    type Response = EcResponseUsbPdPorts;
    const CMD: EcCmd = EcCmd::UsbPdPorts;
}

/// This command will return the number of USB PD charge ports + the number of
/// dedicated ports present.
struct ChargePortCount;

impl EcCommand for ChargePortCount {
    type Request = ();
    type Response = EcResponseChargePortCount;
    const CMD: EcCmd = EcCmd::ChargePortCount;
}

/// Get number of USB PD ports.
/// Always returns 0 on my FW16.
pub(crate) fn get_usb_pd_ports() -> Result<u8, EcError> {
    Ok(UsbPdPorts::call(())?.num_ports)
}

/// Get number of charging ports + number of dedicated ports present.
/// Used in lieu of [`get_usb_pd_ports`], because for some reason on my FW16
/// that always returns 0.
pub(crate) fn get_charge_port_count() -> Result<u8, EcError> {
    Ok(ChargePortCount::call(())?.port_count)
}

/// Number of charging ports + number of dedicated ports present
pub static CHARGE_PORT_COUNT: LazyLock<Result<u8, EcError>> = LazyLock::new(get_charge_port_count);

/// Check `idx` against the port count the EC reports, so a bad index fails
/// here rather than as an opaque EC rejection.
fn validate_port(idx: u8) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let num_ports = *CHARGE_PORT_COUNT.as_ref().map_err(|e| e.to_string())?;

    if !(0..num_ports).contains(&idx) {
        return Err(format!("Port number {idx} not within range 0..{num_ports}").into());
    }

    Ok(())
}

/// Get power information about a USB PD port.
struct UsbPdPowerInfo;

impl EcCommand for UsbPdPowerInfo {
    type Request = EcParamsUsbPdPowerInfo;
    type Response = EcResponseUsbPdPowerInfo;
    const CMD: EcCmd = EcCmd::UsbPdPowerInfo;
}

pub(crate) fn get_port_pd_info(
    idx: u8,
) -> Result<EcResponseUsbPdPowerInfo, Box<dyn std::error::Error + Send + Sync>> {
    validate_port(idx)?;
    Ok(UsbPdPowerInfo::call(EcParamsUsbPdPowerInfo { port: idx })?)
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

/// Get info about the PD chip on a port.
struct PdChipInfo;

impl EcCommand for PdChipInfo {
    type Request = EcParamsPdChipInfo;
    type Response = EcResponsePdChipInfo;
    const CMD: EcCmd = EcCmd::PdChipInfo;
}

/// Version 1 of [`PdChipInfo`], which also reports the minimum required
/// firmware version.
struct PdChipInfoV1;

impl EcCommand for PdChipInfoV1 {
    type Request = EcParamsPdChipInfo;
    type Response = EcResponsePdChipInfoV1;
    const CMD: EcCmd = EcCmd::PdChipInfo;
    const VERSION: u32 = 1;
}

pub(crate) fn get_pd_chip_info(
    idx: u8,
) -> Result<EcResponsePdChipInfo, Box<dyn std::error::Error + Send + Sync>> {
    validate_port(idx)?;
    Ok(PdChipInfo::call(EcParamsPdChipInfo {
        port: idx,
        live: EcPdChipInfoLive::Live,
    })?)
}

pub(crate) fn get_pd_chip_info_v1(
    idx: u8,
) -> Result<EcResponsePdChipInfoV1, Box<dyn std::error::Error + Send + Sync>> {
    validate_port(idx)?;
    Ok(PdChipInfoV1::call(EcParamsPdChipInfo {
        port: idx,
        live: EcPdChipInfoLive::Hardcoded,
    })?)
}

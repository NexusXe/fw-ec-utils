//! USB-PD state for the D-Bus service.
//!
//! Only [`get_port_pd_info`] and [`CHARGE_PORT_COUNT`] are live; the rest is
//! reference material transcribed from `ec_commands.h`. Anything targeting a
//! command this EC does not implement is marked **Not implemented on this EC**
//! in its doc comment — grep for that phrase before building on something here.

use std::{fmt, sync::LazyLock};

use ec_core::{EcCmd, EcCommand, EcError, usbpd};

// The port-state types and the two commands behind them are shared with
// `fw-pdctl`, so they live in `ec-core::usbpd` rather than being declared
// twice. Re-exported here so this module reads as one place.
pub(crate) use ec_core::usbpd::{ChargeType, EcResponseUsbPdPowerInfo, PowerRole};

/// **Not implemented on this EC** (`EC_CMD_USB_CHARGE_SET_MODE`, 0x0030) —
/// reference only, never called.
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

/// **Not implemented on this EC** (`EC_CMD_PD_EXCHANGE_STATUS`, 0x0100) —
/// reference only, never called.
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

/// Maximum number of PD ports on a device, num_ports will be <= this
const EC_USB_PD_MAX_PORTS: usize = 8;

/// Number of PD ports present. Does not include dedicated ports.
///
/// **Not implemented on this EC** (`EC_CMD_USB_PD_PORTS`, 0x0102) — reference
/// only, never called.
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

/// Get number of USB PD ports.
///
/// **Not implemented on this EC** (`EC_CMD_USB_PD_PORTS`, 0x0102) — it is
/// absent from `GET_CMD_VERSIONS`, which is why it answers 0 rather than a
/// port count. Use [`get_charge_port_count`] instead.
pub(crate) fn get_usb_pd_ports() -> Result<u8, EcError> {
    Ok(UsbPdPorts::call(())?.num_ports)
}

/// Get number of charging ports + number of dedicated ports present.
///
/// Used in lieu of [`get_usb_pd_ports`], whose command this EC does not have.
pub(crate) fn get_charge_port_count() -> Result<u8, EcError> {
    usbpd::charge_port_count()
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

pub(crate) fn get_port_pd_info(
    idx: u8,
) -> Result<EcResponseUsbPdPowerInfo, Box<dyn std::error::Error + Send + Sync>> {
    validate_port(idx)?;
    Ok(usbpd::power_info(idx)?)
}

/// Get info about USB-C SS muxes.
///
/// **Not implemented on this EC** (`EC_CMD_USB_PD_MUX_INFO`, 0x011A) —
/// reference only, never called.
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
///
/// **Not implemented on this EC** (`EC_CMD_PD_CHIP_INFO`, 0x011B) — reference
/// only, never called.
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

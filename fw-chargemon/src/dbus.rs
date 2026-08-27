//! D-Bus server for fw-chargemon.
//!
//! Exposes EC battery and USB-PD charging information over D-Bus using
//! [zbus](https://docs.rs/zbus) 5.x with its `async-io`-backed blocking API —
//! no tokio required.
//!
//! Bus name:  `org.nexusxe.FwChargemon`
//!
//! Interfaces:
//! - `org.nexusxe.FwChargemon.Battery`  at `/org/nexusxe/FwChargemon/Battery`
//! - `org.nexusxe.FwChargemon.Usb`      at `/org/nexusxe/FwChargemon/Usb`

use serde::{Deserialize, Serialize};
use zbus::zvariant::Type;
use zbus::{blocking::Connection, interface};

use crate::{
    battery::{get_battery_dynamic_info, get_memmapped_battery_info},
    usb::{CHARGE_PORT_COUNT, get_port_pd_info},
};

// ── Battery interface ────────────────────────────────────────────────────────

/// DTO for memory-mapped battery info (from EC mmap region).
///
/// D-Bus signature: `(uuuuyyyyuuuussss)` — see field order below.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct MemMappedBatteryInfoDto {
    /// Battery Present Voltage (mV)
    pub volt: u32,
    /// Battery Present Rate (mA)
    pub rate: u32,
    /// Battery Remaining Capacity (mAh)
    pub cap: u32,
    /// Battery Flags (see EC_BATT_FLAG_* bits)
    pub state: u8,
    /// Battery Count
    pub count: u8,
    /// Current Battery Data Index
    pub index: u8,
    /// Battery Design Capacity (mAh)
    pub dcap: u32,
    /// Battery Design Voltage (mV)
    pub dvlt: u32,
    /// Battery Last Full Charge Capacity (mAh)
    pub lfcc: u32,
    /// Battery Cycle Count
    pub ccnt: u32,
    /// Battery Manufacturer String
    pub mfgr: String,
    /// Battery Model Number String
    pub model: String,
    /// Battery Serial Number String
    pub serial: String,
    /// Battery Type String
    pub r#type: String,
}

/// DTO for battery dynamic info (queried via EC command).
///
/// All values use the units from the EC protocol:
/// voltages in mV, currents in mA, capacity in mAh.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct BatteryDynamicInfoDto {
    /// Battery voltage (mV)
    pub actual_voltage: i16,
    /// Battery current (mA); negative = discharging
    pub actual_current: i16,
    /// Remaining capacity (mAh)
    pub remaining_capacity: i16,
    /// Full capacity (mAh, might change occasionally)
    pub full_capacity: i16,
    /// Flags (see `EcBattFlag`)
    pub flags: i16,
    /// Charging voltage desired by battery (mV)
    pub desired_voltage: i16,
    /// Charging current desired by battery (mA)
    pub desired_current: i16,
}

/// DTO for USB-PD power info for one port.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct PortPdInfoDto {
    /// Power role (see `UsbPowerRoles`: 0=Disconnected, 1=Source, 2=Sink, 3=SinkNotCharging)
    pub role: u8,
    /// Charger type (see `UsbChgType`)
    pub charge_type: u8,
    /// Whether the port supports dual-role operation
    pub dualrole: u8,
    pub reserved1: u8,
    /// Maximum voltage advertised (mV)
    pub voltage_max: u16,
    /// Present voltage (mV)
    pub voltage_now: u16,
    /// Maximum current advertised (mA)
    pub current_max: u16,
    /// Present current (mA)
    pub current_now: u16,
    /// Maximum power in microwatts
    pub max_power: u32,
}

// ── Interface implementations ────────────────────────────────────────────────

struct BatteryInterface;

#[interface(name = "org.nexusxe.FwChargemon.Battery")]
impl BatteryInterface {
    /// Return memory-mapped battery info read directly from the EC.
    fn get_mem_mapped_info(&self) -> zbus::fdo::Result<MemMappedBatteryInfoDto> {
        let info =
            get_memmapped_battery_info().map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;

        let c_str_to_string = |arr: &[std::ffi::c_char]| {
            let bytes = unsafe { std::slice::from_raw_parts(arr.as_ptr() as *const u8, arr.len()) };
            let len = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
            String::from_utf8_lossy(&bytes[..len]).into_owned()
        };

        Ok(MemMappedBatteryInfoDto {
            volt: info.volt,
            rate: info.rate,
            cap: info.cap,
            state: info.state.0,
            count: info.count,
            index: info.index,
            dcap: info.dcap,
            dvlt: info.dvlt,
            lfcc: info.lfcc,
            ccnt: info.ccnt,
            mfgr: c_str_to_string(&info.mfgr),
            model: c_str_to_string(&info.model),
            serial: c_str_to_string(&info.serial),
            r#type: c_str_to_string(&info.r#type),
        })
    }

    /// Return battery dynamic info queried via EC command.
    fn get_dynamic_info(&self) -> zbus::fdo::Result<BatteryDynamicInfoDto> {
        let info =
            get_battery_dynamic_info().map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;

        Ok(BatteryDynamicInfoDto {
            actual_voltage: info.actual_voltage,
            actual_current: info.actual_current,
            remaining_capacity: info.remaining_capacity,
            full_capacity: info.full_capacity,
            flags: info.flags,
            desired_voltage: info.desired_voltage,
            desired_current: info.desired_current,
        })
    }
}

struct UsbInterface;

#[interface(name = "org.nexusxe.FwChargemon.Usb")]
impl UsbInterface {
    /// Number of charging ports + dedicated ports present on this device.
    #[zbus(property(emits_changed_signal = "const"))]
    fn charge_port_count(&self) -> zbus::fdo::Result<u8> {
        CHARGE_PORT_COUNT
            .as_ref()
            .copied()
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
    }

    /// Return USB-PD power info for the given port index.
    fn get_port_pd_info(&self, port: u8) -> zbus::fdo::Result<PortPdInfoDto> {
        let info = get_port_pd_info(port).map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;

        Ok(PortPdInfoDto {
            role: info.role as u8,
            charge_type: info.r#type as u8,
            dualrole: info.dualrole,
            reserved1: info.reserved1,
            voltage_max: info.meas.voltage_max,
            voltage_now: info.meas.voltage_now,
            current_max: info.meas.current_max,
            current_now: info.meas.current_now,
            max_power: info.max_power,
        })
    }
}

// ── Server entry point ───────────────────────────────────────────────────────

const BUS_NAME: &str = "org.nexusxe.FwChargemon";
const BATTERY_PATH: &str = "/org/nexusxe/FwChargemon/Battery";
const USB_PATH: &str = "/org/nexusxe/FwChargemon/Usb";

/// Connect to the D-Bus session bus, register all interfaces, and block
/// indefinitely serving incoming method calls and property requests.
///
/// The underlying I/O is driven by `async-io` (no tokio); zbus manages
/// its own background executor thread.
pub fn serve() -> Result<(), Box<dyn std::error::Error>> {
    let conn = Connection::system()?;

    conn.request_name(BUS_NAME)?;

    conn.object_server().at(BATTERY_PATH, BatteryInterface)?;
    conn.object_server().at(USB_PATH, UsbInterface)?;

    println!("fw-chargemon D-Bus service running on bus '{BUS_NAME}'.");
    println!("  Battery : {BATTERY_PATH}");
    println!("  USB/PD  : {USB_PATH}");

    // Park the main thread; zbus runs its executor on internal threads.
    loop {
        std::thread::park();
    }
}

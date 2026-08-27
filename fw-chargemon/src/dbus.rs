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
/// D-Bus signature: `(uuuyyyuuuussss)` — see field order below, and the
/// `wire_contract` tests, which assert it.
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
///
/// D-Bus signature: `(yyyyqqqqu)`.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct PortPdInfoDto {
    /// Power role (see [`ec_core::PowerRole`]: 0=Disconnected, 1=Source,
    /// 2=Sink, 3=SinkNotCharging).
    ///
    /// Only `2` means this port is the one the EC chose to draw power from. A
    /// port at `3` has a charger attached that the EC did *not* select, so a
    /// consumer looking for "where is the power coming from" must not treat
    /// the two alike.
    pub role: u8,
    /// Charger type (see [`ec_core::ChargeType`])
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
    /// Negotiated current limit (mA) — `current_lim` in `ec_commands.h`, not an
    /// instantaneous draw. Third `q` of the signature; renaming the field does
    /// not move it on the wire.
    pub current_lim: u16,
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
            role: info.role,
            charge_type: info.charge_type,
            dualrole: info.dualrole,
            reserved1: info.reserved1,
            voltage_max: info.voltage_max,
            voltage_now: info.voltage_now,
            current_max: info.current_max,
            current_lim: info.current_lim,
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

#[cfg(test)]
mod wire_contract {
    //! The DTO layouts are the D-Bus wire format the KDE plasmoid consumes.
    //!
    //! Field *names* are not on the wire — zvariant serializes struct fields
    //! positionally — but their order and types are. These signatures were
    //! taken from `busctl introspect` against the running service; if a change
    //! moves one, the widget breaks silently, so make the break loud here
    //! instead.

    use super::{BatteryDynamicInfoDto, MemMappedBatteryInfoDto, PortPdInfoDto};
    use zbus::zvariant::Type;

    #[test]
    fn port_pd_info_signature_is_stable() {
        assert_eq!(PortPdInfoDto::SIGNATURE.to_string(), "(yyyyqqqqu)");
    }

    #[test]
    fn mem_mapped_battery_info_signature_is_stable() {
        assert_eq!(
            MemMappedBatteryInfoDto::SIGNATURE.to_string(),
            "(uuuyyyuuuussss)"
        );
    }

    #[test]
    fn battery_dynamic_info_signature_is_stable() {
        assert_eq!(BatteryDynamicInfoDto::SIGNATURE.to_string(), "(nnnnnnn)");
    }
}

use std::num::NonZeroU8;

use fw_chargemon::dbus::{MemMappedBatteryInfoDto, PortPdInfoDto};
use zbus::blocking::Connection;
use zbus::proxy;

#[proxy(
    interface = "org.nexusxe.FwChargemon.Usb",
    default_service = "org.nexusxe.FwChargemon",
    default_path = "/org/nexusxe/FwChargemon/Usb"
)]
trait FwChargeMonUsb {
    /// Number of charge ports + dedicated ports on this device.
    #[zbus(property)]
    fn charge_port_count(&self) -> zbus::Result<u8>;

    /// USB-PD power info for one port.
    fn get_port_pd_info(&self, port: u8) -> zbus::Result<PortPdInfoDto>;
}

#[proxy(
    interface = "org.nexusxe.FwChargemon.Battery",
    default_service = "org.nexusxe.FwChargemon",
    default_path = "/org/nexusxe/FwChargemon/Battery"
)]
trait FwChargeMonBattery {
    /// Memory-mapped battery info read directly from EC.
    fn get_mem_mapped_info(&self) -> zbus::Result<MemMappedBatteryInfoDto>;
}

fn main() {
    if let Err(e) = run() {
        eprintln!("fw-chargemon-query: {e}");
        std::process::exit(1);
    }
}

fn run() -> zbus::Result<()> {
    let conn = Connection::system()?;

    // ── USB/PD: find the active charging port ────────────────────────────────
    let usb = FwChargeMonUsbProxyBlocking::new(&conn)?;
    let port_count = usb.charge_port_count()?;

    let mut active_port: Option<NonZeroU8> = None;
    let mut voltage_max: u16 = 0;
    let mut current_max: u16 = 0;
    let mut max_power: u32 = 0;

    for i in 0..port_count {
        let info = usb.get_port_pd_info(i)?;
        // role 2 = Sink (charging), role 3 = SinkNotCharging (port present but full)
        if info.role == 2 || info.role == 3 {
            active_port = NonZeroU8::new(i + 1);
            voltage_max = info.voltage_max;
            current_max = info.current_max;
            max_power = info.max_power;
            break;
        }
    }

    // ── Battery: state flags from EC mmap ────────────────────────────────────
    let batt = FwChargeMonBatteryProxyBlocking::new(&conn)?;
    let batt_info = batt.get_mem_mapped_info()?;
    let batt_state = batt_info.state;

    // ── Emit key=value for the plasmoid to parse ─────────────────────────────
    println!(
        "ok=1\nport={}\nvoltage_max={voltage_max}\ncurrent_max={current_max}\nmax_power={max_power}\nbatt_state={batt_state}",
        active_port.map_or_else(|| "?".to_string(), |n| n.get().to_string())
    );

    Ok(())
}

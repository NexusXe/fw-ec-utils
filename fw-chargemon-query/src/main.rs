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

/// `USB_PD_PORT_POWER_SINK` — the port the EC selected to charge from.
const ROLE_SINK: u8 = 2;
/// `USB_PD_PORT_POWER_SINK_NOT_CHARGING` — a charger is attached, but the EC
/// chose a different port to draw from.
const ROLE_SINK_NOT_CHARGING: u8 = 3;

/// Rank for [`ROLE_SINK`]; nothing beats it.
const RANK_CHARGING: u8 = 2;

/// How good a candidate a port is for "where the power is coming from".
///
/// The EC marks exactly one port `Sink` once it has picked where to draw from.
/// Any other port with a charger attached reports `SinkNotCharging`, so taking
/// the first sink-ish port found reports the wrong one whenever two chargers
/// are plugged in. A `SinkNotCharging` port is still worth falling back to —
/// without it a plugged-in-but-unselected charger would show as no charger at
/// all — but it must never outrank the real one.
const fn rank(role: u8) -> u8 {
    match role {
        ROLE_SINK => RANK_CHARGING,
        ROLE_SINK_NOT_CHARGING => 1,
        _ => 0,
    }
}

fn run() -> zbus::Result<()> {
    let conn = Connection::system()?;

    // ── USB/PD: find the active charging port ────────────────────────────────
    let usb = FwChargeMonUsbProxyBlocking::new(&conn)?;
    let port_count = usb.charge_port_count()?;

    let mut best: Option<(u8, PortPdInfoDto)> = None;

    for i in 0..port_count {
        let info = usb.get_port_pd_info(i)?;

        if rank(info.role) == 0 {
            continue;
        }

        // Strictly greater, so among equally-ranked ports the lowest index wins.
        if best
            .as_ref()
            .is_none_or(|(_, chosen)| rank(info.role) > rank(chosen.role))
        {
            let selected = rank(info.role) == RANK_CHARGING;
            best = Some((i, info));

            // Nothing outranks the port the EC actually chose.
            if selected {
                break;
            }
        }
    }

    let (active_port, voltage_max, current_max, max_power) = match best {
        Some((i, info)) => (
            NonZeroU8::new(i + 1),
            info.voltage_max,
            info.current_max,
            info.max_power,
        ),
        None => (None, 0, 0, 0),
    };

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

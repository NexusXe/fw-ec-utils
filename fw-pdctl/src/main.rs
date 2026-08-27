//! `fw-pdctl` — read USB-PD port state and control power direction.
//!
//! What this can actually do depends on the EC in front of it, and the FW16's
//! implements only part of the PD command family. `fw-pdctl supported` asks it
//! directly. As of writing, on this hardware:
//!
//! - **Reading port state works** (`EC_CMD_USB_PD_POWER_INFO`) — this is the
//!   same command `fw-chargemon` reads.
//! - **Choosing which port charges the system works**
//!   (`EC_CMD_PD_CHARGE_PORT_OVERRIDE`) — the input side of power direction.
//! - **Pinning a port as a source does not.** `EC_CMD_USB_PD_CONTROL` is not
//!   implemented, and neither is `TYPEC_CONTROL` or `GET_PD_PORT_CAPS`. The
//!   `set` and `caps` subcommands are here because they are what the protocol
//!   specifies; on this machine they return `EC_RES_INVALID_COMMAND`.

#![warn(clippy::pedantic, clippy::nursery)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod pd;

use std::error::Error;
use std::process::ExitCode;

use clap::{Parser, Subcommand, ValueEnum};
use ec_core::{EcError, EcStatus, PowerRole, introspect, usbpd};

use pd::RoleRequest;

#[derive(Parser, Debug)]
#[command(version, about = "Read USB-PD port state and control power direction.", long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Show each port's power role and negotiated power. This is the default.
    Status {
        /// Only show this port.
        #[arg(short, long)]
        port: Option<u8>,
    },

    /// Ask the EC which PD-related host commands it implements.
    ///
    /// Worth running first on any machine: the command table describes the
    /// protocol, not what a given EC backs.
    Supported,

    /// Show each port's static role capabilities, as the EC declares them.
    ///
    /// Not implemented on the FW16.
    Caps {
        /// Only show this port.
        #[arg(short, long)]
        port: Option<u8>,
    },

    /// Send a no-op role command to check whether this EC implements role
    /// control at all. Changes nothing.
    Probe {
        /// Only probe this port.
        #[arg(short, long)]
        port: Option<u8>,
    },

    /// Pin a port's power role.
    ///
    /// `source` makes the port supply power from the battery; `sink` makes it
    /// draw power. `auto` restores the EC's own dual-role toggling and is the
    /// resting state to come back to.
    ///
    /// Not implemented on the FW16 — run `fw-pdctl probe` first.
    Set {
        /// Port index.
        port: u8,

        /// Role to pin the port to.
        role: RoleArg,

        /// Apply even when the port is currently powering the system.
        #[arg(short, long)]
        force: bool,
    },

    /// Choose which port the system charges from.
    ///
    /// Takes a port index, `off` to hand selection back to the EC, or `none`
    /// to refuse to charge from any port. This is the one power-direction
    /// control the FW16's EC implements.
    ChargeOverride {
        /// Port index, `off`, or `none`.
        target: String,
    },
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum RoleArg {
    /// Pin as a power source: supplies power from the battery.
    Source,
    /// Pin as a power sink: draws power, never supplies it.
    Sink,
    /// Resume the EC's normal dual-role toggling.
    Auto,
    /// Stop dual-role toggling, staying in the current role.
    ToggleOff,
    /// Freeze the port's state machine where it is.
    Freeze,
}

impl From<RoleArg> for RoleRequest {
    fn from(arg: RoleArg) -> Self {
        match arg {
            RoleArg::Source => Self::ForceSource,
            RoleArg::Sink => Self::ForceSink,
            RoleArg::Auto => Self::Auto,
            RoleArg::ToggleOff => Self::ToggleOff,
            RoleArg::Freeze => Self::Freeze,
        }
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("[ERROR]: {e}");
            if let Some(hint) = hint_for(e.as_ref()) {
                eprintln!("[ERROR]: {hint}");
            }
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    match args.command.unwrap_or(Command::Status { port: None }) {
        Command::Status { port } => print_status(port),
        Command::Supported => print_supported(),
        Command::Caps { port } => print_caps(port),
        Command::Probe { port } => probe(port),
        Command::Set { port, role, force } => set_role(port, role.into(), force),
        Command::ChargeOverride { target } => charge_override(&target),
    }
}

/// `EC_RES_INVALID_COMMAND`.
const NOT_IMPLEMENTED: EcStatus = EcStatus(1);
/// `EC_RES_INVALID_PARAM`.
const BAD_PARAM: EcStatus = EcStatus(3);

/// Turn an EC failure into a sentence worth adding underneath it.
fn hint_for(e: &(dyn Error + 'static)) -> Option<&'static str> {
    match e.downcast_ref::<EcError>()? {
        EcError::Unavailable(_) => Some("/dev/cros_ec is root-only; try running this under sudo."),
        EcError::Rejected { status, .. } if *status == NOT_IMPLEMENTED => Some(
            "This EC does not implement that command. Run `fw-pdctl supported` to see what it does.",
        ),
        EcError::Rejected { status, .. } if *status == BAD_PARAM => {
            Some("The EC rejected the parameters; check the port index.")
        }
        _ => None,
    }
}

/// The ports to act on: one if given, otherwise every port the EC reports.
fn ports(only: Option<u8>) -> Result<Vec<u8>, Box<dyn Error>> {
    let count = usbpd::charge_port_count()?;
    match only {
        Some(p) => {
            check_port(p, count)?;
            Ok(vec![p])
        }
        None => Ok((0..count).collect()),
    }
}

fn check_port(port: u8, count: u8) -> Result<(), Box<dyn Error>> {
    if port >= count {
        return Err(format!("port {port} is out of range; the EC reports {count} ports").into());
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Reads
// ---------------------------------------------------------------------------

fn print_status(only: Option<u8>) -> Result<(), Box<dyn Error>> {
    let ports = ports(only)?;

    println!("--- USB-PD Ports ---");
    println!(
        "{:<5} {:<16} {:<12} {:>15} {:>14} {:>11}  Partner",
        "Port", "Role", "Charger", "Volt now/max", "Curr lim/max", "Max power"
    );

    for port in ports {
        let info = match usbpd::power_info(port) {
            Ok(info) => info,
            Err(e) => {
                println!("{port:<5} {e}");
                continue;
            }
        };

        let role = info
            .power_role()
            .map_or_else(|| format!("unknown ({})", info.role), |r| r.to_string());
        let charger = info.charger_type().map_or_else(
            || format!("unknown ({})", info.charge_type),
            |t| t.to_string(),
        );

        // Copy out of the packed struct before formatting.
        let (v_now, v_max) = (info.voltage_now, info.voltage_max);
        let (c_lim, c_max) = (info.current_lim, info.current_max);
        let max_power = info.max_power;

        let connected = info.power_role() != Some(PowerRole::Disconnected);
        let (voltage, current, power) = if connected {
            (
                format!("{v_now}/{v_max}mV"),
                format!("{c_lim}/{c_max}mA"),
                format_microwatts(max_power),
            )
        } else {
            ("-".to_string(), "-".to_string(), "-".to_string())
        };

        println!(
            "{port:<5} {role:<16} {charger:<12} {voltage:>15} {current:>14} {power:>11}  {}",
            if info.dualrole == 0 { "" } else { "dual-role" }
        );
    }

    Ok(())
}

/// Microwatts as `W`, without floating point.
fn format_microwatts(uw: u32) -> String {
    format!("{}.{:02} W", uw / 1_000_000, (uw % 1_000_000) / 10_000)
}

/// Ask the EC, command by command, what it actually implements.
///
/// Cheaper and safer than sending each command to find out, and the answer
/// varies by machine.
fn print_supported() -> Result<(), Box<dyn Error>> {
    println!("--- PD host commands this EC implements ---");
    println!("{:<8} {:<24} {:<10} Purpose", "Cmd", "Name", "Versions");

    for &(cmd, purpose) in pd::PD_COMMANDS {
        // An error here is systemic — the device is unreachable, or the EC has
        // no GET_CMD_VERSIONS — not a fact about this particular command, so
        // there is nothing to learn from asking about the rest.
        let versions =
            introspect::command_versions(cmd)?.map_or_else(|| "-".to_string(), version_list);

        println!(
            "{:<8} {:<24} {versions:<10} {purpose}",
            format!("{:#06x}", cmd as u32),
            format!("{cmd:?}"),
        );
    }

    println!("(`-` means the EC has no such command.)");

    Ok(())
}

/// A `GET_CMD_VERSIONS` bitmask as a readable version list.
fn version_list(mask: u32) -> String {
    let versions: Vec<String> = (0..u32::BITS)
        .filter(|i| mask & (1 << i) != 0)
        .map(|i| format!("v{i}"))
        .collect();

    if versions.is_empty() {
        "none".to_string()
    } else {
        versions.join(",")
    }
}

fn print_caps(only: Option<u8>) -> Result<(), Box<dyn Error>> {
    let ports = ports(only)?;

    println!("--- USB-PD Port Capabilities ---");
    println!(
        "{:<5} {:<12} {:<12} {:<12} Location",
        "Port", "Power role", "Try role", "Data role"
    );

    for port in ports {
        let caps = match pd::port_caps(port) {
            Ok(caps) => caps,
            Err(e) => {
                println!("{port:<5} {e}");
                continue;
            }
        };

        let power = pd::power_role_cap_name(caps.power_role_cap).map_or_else(
            || format!("unknown ({})", caps.power_role_cap),
            str::to_string,
        );
        let try_role = pd::try_power_role_cap_name(caps.try_power_role_cap).map_or_else(
            || format!("unknown ({})", caps.try_power_role_cap),
            str::to_string,
        );
        let data = pd::data_role_cap_name(caps.data_role_cap).map_or_else(
            || format!("unknown ({})", caps.data_role_cap),
            str::to_string,
        );

        println!(
            "{port:<5} {power:<12} {try_role:<12} {data:<12} {}",
            caps.port_location
        );
    }

    Ok(())
}

/// Send `USB_PD_CONTROL` with every field set to no-change. Nothing on the port
/// moves; the point is whether the EC answers at all.
fn probe(only: Option<u8>) -> Result<(), Box<dyn Error>> {
    let ports = ports(only)?;

    println!("--- USB_PD_CONTROL probe (no-op, changes nothing) ---");
    let mut any_ok = false;

    for port in ports {
        match pd::set_role(port, RoleRequest::NoChange) {
            Ok(resp) => {
                any_ok = true;
                let (enabled, role, polarity, state) =
                    (resp.enabled, resp.role, resp.polarity, resp.state);
                println!(
                    "Port {port}: accepted (pd_enabled={enabled}, role={role}, \
                     polarity={polarity}, state={state})"
                );
            }
            Err(e) => println!("Port {port}: {e}"),
        }
    }

    if any_ok {
        println!("[INFO]: This EC implements role control; `set` should work.");
    } else {
        println!("[WARN]: No port accepted the command. Role control is unavailable here.");
        println!("[WARN]: `charge-override` is the only power-direction control this EC offers.");
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Writes
// ---------------------------------------------------------------------------

fn set_role(port: u8, role: RoleRequest, force: bool) -> Result<(), Box<dyn Error>> {
    let count = usbpd::charge_port_count()?;
    check_port(port, count)?;

    // Pinning a port that is currently feeding the machine takes its own power
    // away, and there is no getting the command back once it lands.
    if role.can_drop_input() && !force {
        match usbpd::power_info(port) {
            Ok(info) => {
                if let Some(current) = info.power_role()
                    && current.is_drawing_power()
                {
                    return Err(format!(
                        "port {port} is currently {current} — it is powering this system, and \
                         setting it to `{role}` would cut that input. Pass --force if that is \
                         what you want."
                    )
                    .into());
                }
            }
            Err(e) => {
                eprintln!("[WARN]: could not read port {port} state before setting it: {e}");
            }
        }
    }

    let resp = pd::set_role(port, role)?;
    let state = resp.state;
    println!("[INFO]: Set port {port} to `{role}` (PD state machine now at {state}).");

    // Confirm against the read path rather than trusting the write.
    match usbpd::power_info(port) {
        Ok(info) => {
            let now = info
                .power_role()
                .map_or_else(|| format!("unknown ({})", info.role), |r| r.to_string());
            println!("[INFO]: Port {port} now reports role {now}.");
        }
        Err(e) => eprintln!("[WARN]: could not read back port {port}: {e}"),
    }

    if role != RoleRequest::Auto {
        println!("[INFO]: Restore normal behaviour with `fw-pdctl set {port} auto`.");
    }

    Ok(())
}

fn charge_override(target: &str) -> Result<(), Box<dyn Error>> {
    let port = match target {
        "off" => usbpd::OVERRIDE_OFF,
        "none" => usbpd::OVERRIDE_DONT_CHARGE,
        other => {
            let port: u8 = other
                .parse()
                .map_err(|_| format!("expected a port index, `off`, or `none`, got `{other}`"))?;
            check_port(port, usbpd::charge_port_count()?)?;
            i16::from(port)
        }
    };

    // Report what is about to be given up, so a lost charger is not a surprise.
    if port == usbpd::OVERRIDE_DONT_CHARGE
        && let Ok(Some(active)) = active_charge_port()
    {
        println!("[WARN]: Port {active} is charging this system right now.");
    }

    usbpd::set_charge_override(port)?;

    match port {
        usbpd::OVERRIDE_OFF => println!("[INFO]: Charge port selection handed back to the EC."),
        usbpd::OVERRIDE_DONT_CHARGE => {
            println!("[INFO]: Charging disabled on all ports.");
            println!("[INFO]: Restore with `fw-pdctl charge-override off`.");
        }
        p => {
            println!("[INFO]: Charging forced to port {p}.");
            println!("[INFO]: Restore with `fw-pdctl charge-override off`.");
        }
    }

    Ok(())
}

/// The port currently drawing power into the system, if any.
fn active_charge_port() -> Result<Option<u8>, EcError> {
    for port in 0..usbpd::charge_port_count()? {
        if usbpd::power_info(port)?.is_active_charger() {
            return Ok(Some(port));
        }
    }
    Ok(None)
}

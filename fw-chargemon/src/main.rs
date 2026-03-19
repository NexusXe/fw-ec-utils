#![feature(default_field_values)]

use clap::Parser;
use ec_core::common::{self, CrosEcCommandV2, EcCmd, FullWriteV2Command, fire};

use crate::usb::{EcResponseChargePortCount, EcResponseUsbPdPorts};

mod battery;
mod charging;
mod ec_mmap_offsets;
mod usb;

#[derive(Parser)]
struct Args {
    #[arg(short, long)]
    path: Option<String>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _args = Args::parse();
    let num_ports = usb::get_charge_port_count().map_err(|e| e.to_string())?;
    println!("Number of charging ports: {num_ports}");
    Ok(())
}

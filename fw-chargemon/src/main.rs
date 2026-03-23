#![feature(default_field_values)]
#![feature(const_default)]
#![feature(const_trait_impl)]

use clap::Parser;

use crate::{
    battery::{get_battery_dynamic_info, get_memmapped_battery_info},
    usb::{CHARGE_PORT_COUNT, get_port_pd_info},
};

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
    let num_ports = CHARGE_PORT_COUNT.as_ref().map_err(|e| e.to_string())?;
    println!("Number of charging ports: {num_ports}");

    let tmp = get_memmapped_battery_info();
    let info = tmp.as_ref().map_err(|e| e.to_string())?;
    println!("{info}");

    let num_ports = *CHARGE_PORT_COUNT.as_ref().map_err(|e| e.to_string())?;

    for i in 0..num_ports {
        let tmp = get_port_pd_info(i);
        let info = tmp.as_ref().map_err(|e| e.to_string())?;
        if info.is_active_charger() {
            println!("Active Port:\nPort {i}: {info}");
        }
    }

    let tmp = get_battery_dynamic_info();
    let info = tmp.as_ref().map_err(|e| e.to_string())?;
    println!("{info:?}");

    Ok(())
}

//#![feature(const_default)]
#![feature(const_trait_impl)]

pub(crate) mod battery;
pub(crate) mod charging;
pub(crate) mod ec_mmap_offsets;
#[allow(unused)]
pub(crate) mod usb;
// DTOs and the serve() entry point are the public API of this crate.
pub mod dbus;

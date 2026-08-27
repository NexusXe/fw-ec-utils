//! The `/dev/cros_ec` handle and the two raw ioctls everything else is built on.
//!
//! Most callers want [`EcCommand`](crate::EcCommand) or
//! [`MemMapRegion`](crate::MemMapRegion) instead; the items here are the
//! escape hatch for transfers those two cannot describe, such as commands with
//! a variable-length response.

use std::ffi::c_int;
use std::fs::{File, OpenOptions};
use std::os::fd::{AsRawFd, RawFd};
use std::sync::LazyLock;

use nix::ioctl_readwrite;

use crate::error::EcError;
use crate::memmap::EC_MEMMAP_SIZE;

/// The process-wide `/dev/cros_ec` handle, opened on first use.
pub static CROS_EC_FILE: LazyLock<Result<File, EcError>> = LazyLock::new(|| {
    let ec = OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/cros_ec")
        .map_err(|e| EcError::Unavailable(e.to_string()));
    if ec.is_ok() {
        println!("[INFO]: Got EC file handle.");
    }
    ec
});

/// The raw file descriptor of [`CROS_EC_FILE`].
pub(crate) fn fd() -> Result<RawFd, EcError> {
    match CROS_EC_FILE.as_ref() {
        Ok(f) => Ok(f.as_raw_fd()),
        Err(EcError::Unavailable(why)) => Err(EcError::Unavailable(why.clone())),
        Err(e) => Err(EcError::Unavailable(e.to_string())),
    }
}

/// The header of a version 2 host command.
///
/// On the wire the request and response payloads both live immediately after
/// this header, sharing the same bytes: the kernel copies `outsize` bytes in
/// and `insize` bytes back out. [`EcCommand`](crate::EcCommand) builds and
/// checks this for you.
#[repr(C)]
pub struct CrosEcCommandV2 {
    /// Command version, usually 0.
    pub version: u32 = 0,
    /// Command number, an [`EcCmd`](crate::EcCmd) discriminant.
    pub command: u32,
    /// Bytes of request payload following this header.
    pub outsize: u32,
    /// Bytes of response payload the EC may write back.
    pub insize: u32 = 0,
    /// Filled in by the EC: an [`EcStatus`](crate::EcStatus) code.
    pub result: u32 = 0,
    /// Marks where the payload starts; a flexible array member in C.
    pub data: [u8; 0] = [],
}

/// Argument to the memory-map read ioctl.
#[repr(C)]
pub struct CrosEcReadmemV2 {
    /// Byte offset into the EC memory map.
    pub offset: u32,
    /// Number of bytes to read.
    pub bytes: u32,
    /// Destination the kernel fills in.
    pub buffer: [u8; EC_MEMMAP_SIZE],
}

const CROS_EC_MAGIC: u8 = 0xEC;
const CROS_EC_DEV_IOCXCMD: c_int = 0;
const CROS_EC_DEV_IOCRDMEM_V2: c_int = 1;

ioctl_readwrite!(
    cros_ec_cmd,
    CROS_EC_MAGIC,
    CROS_EC_DEV_IOCXCMD,
    CrosEcCommandV2
);

ioctl_readwrite!(
    cros_ec_readmem,
    CROS_EC_MAGIC,
    CROS_EC_DEV_IOCRDMEM_V2,
    CrosEcReadmemV2
);

/// Send a prepared command header to the EC and return how many payload bytes
/// the EC wrote back.
///
/// This does *not* inspect [`CrosEcCommandV2::result`] — a command the EC
/// refused still returns `Ok` here. [`EcCommand::call`](crate::EcCommand::call)
/// wraps this with the result and length checks.
///
/// # Errors
///
/// Fails if `/dev/cros_ec` cannot be opened or the ioctl itself fails.
///
/// # Safety
///
/// `header` must point to a [`CrosEcCommandV2`] followed, in the same
/// allocation, by at least `max(header.outsize, header.insize)` bytes of
/// initialised, writable payload storage.
pub unsafe fn xfer(header: *mut CrosEcCommandV2) -> Result<usize, EcError> {
    let bytes = unsafe { cros_ec_cmd(fd()?, header) }?;
    // `nix` maps the kernel's -1 onto `Err`, so anything here is a byte count.
    Ok(usize::try_from(bytes).unwrap_or_default())
}

/// Read `bytes` bytes of the EC memory map starting at `offset` into `dst`.
///
/// Returns how many bytes the kernel actually produced.
///
/// # Safety
///
/// `dst` must point to a writable [`CrosEcReadmemV2`].
pub(crate) unsafe fn readmem(dst: *mut CrosEcReadmemV2) -> Result<usize, EcError> {
    let bytes = unsafe { cros_ec_readmem(fd()?, dst) }?;
    Ok(usize::try_from(bytes).unwrap_or_default())
}

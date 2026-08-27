//! Reading the EC's memory-mapped region.
//!
//! The EC exposes a small block of continuously updated state — battery
//! readings, temperatures, switch positions — that can be read without issuing
//! a host command.

use std::mem::{MaybeUninit, size_of};

use crate::device::{CrosEcReadmemV2, readmem};
use crate::error::EcError;

/// Size of the EC's memory-mapped region, in bytes.
pub const EC_MEMMAP_SIZE: usize = 255;

/// Fill `buf` from the EC memory map, starting at `offset`.
///
/// Use this when the length is only known at runtime; for a fixed `#[repr(C)]`
/// view of a region, implement [`MemMapRegion`] instead.
///
/// # Errors
///
/// Fails if the requested span runs past the end of the memory map, if
/// `/dev/cros_ec` is unavailable, or if the EC returns fewer bytes than asked.
pub fn read_bytes(offset: u32, buf: &mut [u8]) -> Result<(), EcError> {
    if offset as usize + buf.len() > EC_MEMMAP_SIZE {
        return Err(EcError::MemMapRange {
            offset,
            len: buf.len(),
        });
    }

    // Bounded by `EC_MEMMAP_SIZE` per the check above.
    #[allow(clippy::cast_possible_truncation)]
    let bytes = buf.len() as u32;

    let mut mem = CrosEcReadmemV2 {
        offset,
        bytes,
        buffer: [0; EC_MEMMAP_SIZE],
    };

    // SAFETY: `mem` is a live, writable `CrosEcReadmemV2`.
    let got = unsafe { readmem(&raw mut mem) }?;

    if got < buf.len() {
        return Err(EcError::ShortRead {
            offset,
            expected: buf.len(),
            got,
        });
    }

    buf.copy_from_slice(&mem.buffer[..buf.len()]);
    Ok(())
}

/// A fixed-layout view onto one span of the EC memory map.
///
/// Implement this on a `#[repr(C)]` struct whose fields line up with the EC's
/// layout at [`OFFSET`](MemMapRegion::OFFSET), then read it with
/// [`read`](MemMapRegion::read).
///
/// # Safety
///
/// `Self` must be `#[repr(C)]` and inhabited by every bit pattern — no
/// references, no `bool`, no field-less enums whose discriminants are a subset
/// of their range. Its size and field offsets must match the EC's memory map
/// at `OFFSET` exactly, padding included.
pub unsafe trait MemMapRegion: Sized {
    /// Byte offset of the region within the EC memory map.
    const OFFSET: u32;

    /// Read this region from the EC.
    ///
    /// # Errors
    ///
    /// Fails if the region runs past the end of the memory map, if
    /// `/dev/cros_ec` is unavailable, or if the EC returns a short read.
    fn read() -> Result<Self, EcError> {
        let mut raw = MaybeUninit::<Self>::zeroed();

        // SAFETY: the trait's contract says `Self` is `#[repr(C)]` and valid
        // for any bit pattern, so its storage may be filled as plain bytes.
        let bytes = unsafe {
            std::slice::from_raw_parts_mut(raw.as_mut_ptr().cast::<u8>(), size_of::<Self>())
        };

        read_bytes(Self::OFFSET, bytes)?;

        // SAFETY: `read_bytes` filled every byte, or returned an error.
        Ok(unsafe { raw.assume_init() })
    }
}

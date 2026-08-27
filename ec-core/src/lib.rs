//! Shared plumbing for talking to a Framework laptop's embedded controller
//! through the ChromeOS EC driver at `/dev/cros_ec`.
//!
//! Everything the EC exposes arrives over one of two ioctls, and this crate
//! gives each a single declarative shape:
//!
//! - **Host commands.** Declare a marker type, `impl` [`EcCommand`] on it to
//!   name the command number and its request/response payload types, then use
//!   [`EcCommand::call`].
//! - **Memory-mapped state.** Declare a `#[repr(C)]` struct, `impl`
//!   [`MemMapRegion`] on it to name its offset, then use
//!   [`MemMapRegion::read`]. For runtime-sized reads, use
//!   [`memmap::read_bytes`].
//!
//! Both paths report failures as [`EcError`].
//!
//! # Adding a host command
//!
//! One marker type per command *and version*. Use `()` for a side the command
//! does not have — that covers write-only, read-only, bidirectional and
//! no-payload commands with the same declaration.
//!
//! ```no_run
//! use ec_core::{EcCmd, EcCommand};
//!
//! #[repr(C, align(4))]
//! #[derive(Clone, Copy)]
//! struct EcParamsPwmSetFanDuty {
//!     percent: u32,
//! }
//!
//! /// Set target fan PWM duty cycle.
//! struct SetFanDuty;
//!
//! impl EcCommand for SetFanDuty {
//!     type Request = EcParamsPwmSetFanDuty;
//!     type Response = ();
//!     const CMD: EcCmd = EcCmd::PwmSetFanDuty;
//! }
//!
//! SetFanDuty::call(EcParamsPwmSetFanDuty { percent: 50 })?;
//! # Ok::<(), ec_core::EcError>(())
//! ```
//!
//! A second version of the same command is a second marker type overriding
//! [`EcCommand::VERSION`]; [`EcCommand::CMD`] stays the same.
//!
//! # What the wrappers guarantee
//!
//! Relied upon by every caller, so worth not working around:
//!
//! - `outsize` and `insize` are derived from `Request` and `Response`. There is
//!   no place left to hand-write a `size_of` that can drift from the type it
//!   describes.
//! - [`EcCommand::call`] always checks the EC's result code and that the reply
//!   is long enough for `Response`. A command that the EC refused is an
//!   [`EcError`], never a silently-successful call returning zeroed data.
//! - The request and response share the same bytes after the header, so the
//!   payload is stored in a union sized for the larger of the two and zeroed
//!   before use — no stack garbage reaches the EC through the gap between a
//!   short request and a long response.
//!
//! # Payload alignment
//!
//! The kernel reads and writes the payload at a fixed offset: the end of the
//! header. A payload type needing more than 4-byte alignment would be padded
//! away from it and silently corrupt every transfer, so a const assertion
//! rejects one at compile time. If you hit that error, give the offending
//! struct `#[repr(C, packed)]` — matching how these structs are declared in
//! the EC's own `ec_commands.h`.
//!
//! # Escape hatch
//!
//! For a transfer neither shape can describe — a variable-length response, say
//! — [`device::xfer`] is the raw ioctl underneath. It does not check the
//! result code; that is the caller's job.
//!
//! # Module map
//!
//! | Module | Holds |
//! | --- | --- |
//! | [`command`] | [`EcCommand`], and the header/payload framing |
//! | [`memmap`] | [`MemMapRegion`], [`memmap::read_bytes`] |
//! | [`device`] | the `/dev/cros_ec` handle, both ioctls, [`device::xfer`] |
//! | [`error`] | [`EcError`], [`EcStatus`] |
//! | `ec_cmd` | the [`EcCmd`] command-number table |

#![feature(default_field_values)]

pub mod command;
pub mod device;
pub mod error;
pub mod memmap;

mod ec_cmd;

pub use command::EcCommand;
pub use device::{CrosEcCommandV2, CrosEcReadmemV2};
pub use ec_cmd::EcCmd;
pub use error::{EcError, EcStatus};
pub use memmap::MemMapRegion;

//! Asking the EC what it implements.
//!
//! Which host commands an EC answers varies by machine and by firmware, and a
//! command number existing in [`EcCmd`] says nothing about whether this EC
//! backs it. `EC_CMD_GET_CMD_VERSIONS` is the cheap, side-effect-free way to
//! find out, and is a good deal safer than sending a command to see what
//! happens.
//!
//! ```no_run
//! use ec_core::{EcCmd, introspect};
//!
//! if introspect::command_versions(EcCmd::UsbPdPowerInfo)?.is_some() {
//!     // safe to use
//! }
//! # Ok::<(), ec_core::EcError>(())
//! ```

use crate::command::EcCommand;
use crate::ec_cmd::EcCmd;
use crate::error::{EcError, EcStatus};

/// Version 1 of `struct ec_params_get_cmd_versions`.
///
/// Version 0 carries the command number in a `u8`, which cannot reach the
/// commands at `0x0100` and above, so this crate only speaks v1.
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct EcParamsGetCmdVersionsV1 {
    /// The command being asked about.
    pub cmd: u16,
}

/// `struct ec_response_get_cmd_versions`.
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct EcResponseGetCmdVersions {
    /// Bit `n` set means version `n` of the command is supported.
    pub version_mask: u32,
}

/// Read the versions supported for a command.
pub struct GetCmdVersions;

impl EcCommand for GetCmdVersions {
    type Request = EcParamsGetCmdVersionsV1;
    type Response = EcResponseGetCmdVersions;
    const CMD: EcCmd = EcCmd::GetCmdVersions;
    const VERSION: u32 = 1;
}

/// `EC_RES_INVALID_PARAM`, which is how the EC says "no such command".
const NO_SUCH_COMMAND: EcStatus = EcStatus(3);

/// Which versions of `cmd` this EC implements, as a bitmask, or `None` if it
/// does not implement the command at all.
///
/// # Errors
///
/// Fails if the EC is unreachable, or does not itself implement
/// `EC_CMD_GET_CMD_VERSIONS`.
pub fn command_versions(cmd: EcCmd) -> Result<Option<u32>, EcError> {
    // Command numbers are 16-bit on the wire; `EcCmd`'s largest variant is
    // `0x3FFF`.
    #[allow(clippy::cast_possible_truncation)]
    command_versions_raw(cmd as u32 as u16)
}

/// As [`command_versions`], but for a command number with no [`EcCmd`] variant
/// — for sweeping ranges the table does not name, such as the board-specific
/// block at `0x3E00`.
///
/// # Errors
///
/// Fails if the EC is unreachable, or does not itself implement
/// `EC_CMD_GET_CMD_VERSIONS`.
pub fn command_versions_raw(cmd: u16) -> Result<Option<u32>, EcError> {
    match GetCmdVersions::call(EcParamsGetCmdVersionsV1 { cmd }) {
        Ok(resp) => Ok(Some(resp.version_mask)),
        Err(EcError::Rejected { status, .. }) if status == NO_SUCH_COMMAND => Ok(None),
        Err(e) => Err(e),
    }
}

/// Whether this EC implements `cmd` at all.
///
/// # Errors
///
/// As [`command_versions`].
pub fn is_supported(cmd: EcCmd) -> Result<bool, EcError> {
    Ok(command_versions(cmd)?.is_some())
}

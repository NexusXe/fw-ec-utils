//! The single error type every EC access funnels through.

use std::fmt;

use crate::ec_cmd::EcCmd;

/// The result code the EC writes back into a command header (`EC_RES_*`).
///
/// Zero means success; anything else means the EC understood the transfer but
/// refused or failed the command.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EcStatus(pub u32);

impl EcStatus {
    /// `EC_RES_SUCCESS`.
    pub const SUCCESS: Self = Self(0);

    /// Whether the EC reported success.
    #[must_use]
    pub const fn is_success(self) -> bool {
        self.0 == Self::SUCCESS.0
    }

    /// The `EC_RES_*` name for this code, if it is one we know about.
    #[must_use]
    pub const fn name(self) -> Option<&'static str> {
        Some(match self.0 {
            0 => "EC_RES_SUCCESS",
            1 => "EC_RES_INVALID_COMMAND",
            2 => "EC_RES_ERROR",
            3 => "EC_RES_INVALID_PARAM",
            4 => "EC_RES_ACCESS_DENIED",
            5 => "EC_RES_INVALID_RESPONSE",
            6 => "EC_RES_INVALID_VERSION",
            7 => "EC_RES_INVALID_CHECKSUM",
            8 => "EC_RES_IN_PROGRESS",
            9 => "EC_RES_UNAVAILABLE",
            10 => "EC_RES_TIMEOUT",
            11 => "EC_RES_OVERFLOW",
            12 => "EC_RES_INVALID_HEADER",
            13 => "EC_RES_REQUEST_TRUNCATED",
            14 => "EC_RES_RESPONSE_TOO_BIG",
            15 => "EC_RES_BUS_ERROR",
            16 => "EC_RES_BUSY",
            17 => "EC_RES_INVALID_HEADER_VERSION",
            18 => "EC_RES_INVALID_HEADER_CRC",
            19 => "EC_RES_INVALID_DATA_CRC",
            20 => "EC_RES_DUP_UNAVAILABLE",
            _ => return None,
        })
    }
}

impl fmt::Display for EcStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.name() {
            Some(name) => write!(f, "{name} ({})", self.0),
            None => write!(f, "unknown EC result {}", self.0),
        }
    }
}

impl fmt::Debug for EcStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "EcStatus({self})")
    }
}

/// Anything that can go wrong talking to the EC.
#[derive(Debug)]
pub enum EcError {
    /// `/dev/cros_ec` could not be opened. Usually means the `cros_ec_dev`
    /// module is missing, or the process is not running as root.
    Unavailable(String),

    /// The `ioctl` itself failed, before the EC ever saw the command.
    Ioctl(nix::Error),

    /// The command reached the EC, and the EC refused it.
    Rejected {
        /// The command that was refused.
        command: EcCmd,
        /// Version of that command.
        version: u32,
        /// What the EC said went wrong.
        status: EcStatus,
    },

    /// The EC returned fewer payload bytes than the declared response type
    /// needs, so there is no complete response to hand back.
    ShortResponse {
        /// The command that under-delivered.
        command: EcCmd,
        /// Bytes the response type requires.
        expected: usize,
        /// Bytes the EC actually wrote.
        got: usize,
    },

    /// A memory-map read ran past the end of the EC's mapped region.
    MemMapRange {
        /// Requested start offset.
        offset: u32,
        /// Requested length in bytes.
        len: usize,
    },

    /// A memory-map read returned fewer bytes than requested.
    ShortRead {
        /// Requested start offset.
        offset: u32,
        /// Bytes requested.
        expected: usize,
        /// Bytes actually read.
        got: usize,
    },
}

impl fmt::Display for EcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable(why) => write!(f, "cannot open /dev/cros_ec: {why}"),
            Self::Ioctl(e) => write!(f, "EC ioctl failed: {e}"),
            Self::Rejected {
                command,
                version,
                status,
            } => write!(f, "EC rejected {command:?} (v{version}): {status}"),
            Self::ShortResponse {
                command,
                expected,
                got,
            } => write!(
                f,
                "EC returned {got} bytes for {command:?}, expected {expected}"
            ),
            Self::MemMapRange { offset, len } => write!(
                f,
                "memory-map read of {len} bytes at {offset:#04x} runs past the \
                 end of the {} byte EC memory map",
                crate::memmap::EC_MEMMAP_SIZE
            ),
            Self::ShortRead {
                offset,
                expected,
                got,
            } => write!(
                f,
                "memory-map read at {offset:#04x} returned {got} bytes, expected {expected}"
            ),
        }
    }
}

impl std::error::Error for EcError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Ioctl(e) => Some(e),
            _ => None,
        }
    }
}

impl From<nix::Error> for EcError {
    fn from(e: nix::Error) -> Self {
        Self::Ioctl(e)
    }
}

//! Declaring and issuing EC host commands.

use std::mem::{MaybeUninit, offset_of, size_of};

use crate::device::{CrosEcCommandV2, xfer};
use crate::ec_cmd::EcCmd;
use crate::error::{EcError, EcStatus};

/// Payload storage sized and aligned for whichever of the request and response
/// types is larger — the two share the same bytes on the wire.
///
/// Only ever accessed through a raw pointer cast to `Req` or `Res`; the named
/// fields exist purely to drive the layout.
#[repr(C)]
#[allow(dead_code)]
union PayloadStorage<Req: Copy, Res: Copy> {
    req: Req,
    res: Res,
}

/// A command header plus its inline payload, laid out exactly as
/// `CROS_EC_DEV_IOCXCMD` expects.
#[repr(C)]
struct Envelope<Req: Copy, Res: Copy> {
    header: CrosEcCommandV2,
    payload: MaybeUninit<PayloadStorage<Req, Res>>,
}

impl<Req: Copy, Res: Copy> Envelope<Req, Res> {
    /// The kernel reads and writes the payload at a fixed offset — the end of
    /// the header. A request or response type wanting more than 4-byte
    /// alignment would get padded away from it and silently corrupt every
    /// transfer, so fail to compile instead.
    const LAYOUT_OK: () = {
        assert!(
            offset_of!(Self, payload) == size_of::<CrosEcCommandV2>(),
            "EC command payload must directly follow the header: the request or \
             response type is over-aligned (give it `#[repr(C, packed)]`)"
        );
        // Lets `call` narrow the payload sizes to the header's `u32` fields
        // without a fallible conversion.
        assert!(
            size_of::<Req>() <= u32::MAX as usize && size_of::<Res>() <= u32::MAX as usize,
            "EC command payload does not fit in the protocol's 32-bit size fields"
        );
    };
}

/// One host command the EC understands.
///
/// Implement this on a marker type — one per command *and version* — naming
/// the command number and the two payload types. `Request` is what gets sent,
/// `Response` is what comes back; use `()` for either when the command has
/// none. [`call`](EcCommand::call) then handles the framing: payload sizes are
/// derived from the types, and both the EC's result code and the response
/// length are checked before you get a value back.
///
/// ```no_run
/// use ec_core::{EcCmd, EcCommand};
///
/// #[repr(C, packed)]
/// #[derive(Clone, Copy)]
/// struct EcParamsTempSensorGetInfo {
///     id: u8,
/// }
///
/// #[repr(C, packed)]
/// #[derive(Clone, Copy)]
/// struct EcResponseTempSensorGetInfo {
///     sensor_name: [std::ffi::c_char; 32],
///     sensor_type: u8,
/// }
///
/// /// Read temperature sensor info.
/// struct GetTempSensorInfo;
///
/// impl EcCommand for GetTempSensorInfo {
///     type Request = EcParamsTempSensorGetInfo;
///     type Response = EcResponseTempSensorGetInfo;
///     const CMD: EcCmd = EcCmd::TempSensorGetInfo;
/// }
///
/// let info = GetTempSensorInfo::call(EcParamsTempSensorGetInfo { id: 0 })?;
/// # Ok::<(), ec_core::EcError>(())
/// ```
pub trait EcCommand {
    /// Parameters sent to the EC. `()` for commands that take none.
    type Request: Copy;

    /// Payload the EC writes back. `()` for commands that return none.
    type Response: Copy;

    /// The host command number.
    const CMD: EcCmd;

    /// Command version. Most commands only have version 0.
    const VERSION: u32 = 0;

    /// Send this command to the EC and return its response.
    ///
    /// # Errors
    ///
    /// Fails if `/dev/cros_ec` is unavailable, the ioctl fails, the EC reports
    /// a non-zero result, or the EC returns fewer bytes than `Response` needs.
    fn call(request: Self::Request) -> Result<Self::Response, EcError> {
        // Forces the layout assertion to be evaluated for this instantiation.
        () = Envelope::<Self::Request, Self::Response>::LAYOUT_OK;

        let out_len = size_of::<Self::Request>();
        let in_len = size_of::<Self::Response>();

        // Both bounded by `u32::MAX` per `LAYOUT_OK`.
        #[allow(clippy::cast_possible_truncation)]
        let (outsize, insize) = (out_len as u32, in_len as u32);

        let mut envelope = Envelope::<Self::Request, Self::Response> {
            header: CrosEcCommandV2 {
                version: Self::VERSION,
                command: Self::CMD as u32,
                outsize,
                insize,
                ..
            },
            // Zeroed rather than uninitialised so no stack garbage reaches the
            // EC through the padding between a short request and a long response.
            payload: MaybeUninit::zeroed(),
        };

        // SAFETY: the payload region is at least `size_of::<Request>()` bytes
        // (it is sized for the larger of request and response) and, per
        // `LAYOUT_OK`, correctly aligned for `Request`.
        unsafe {
            envelope
                .payload
                .as_mut_ptr()
                .cast::<Self::Request>()
                .write(request);
        }

        // SAFETY: `envelope.payload` follows the header in one allocation and
        // covers both `outsize` and `insize` bytes.
        let returned = unsafe { xfer(&raw mut envelope.header) }?;

        let status = EcStatus(envelope.header.result);
        if !status.is_success() {
            return Err(EcError::Rejected {
                command: Self::CMD,
                version: Self::VERSION,
                status,
            });
        }

        if returned < in_len {
            return Err(EcError::ShortResponse {
                command: Self::CMD,
                expected: in_len,
                got: returned,
            });
        }

        // SAFETY: the EC wrote at least `in_len` bytes into the payload, and
        // `Response: Copy` so reading it out leaves nothing to drop.
        Ok(unsafe { envelope.payload.as_ptr().cast::<Self::Response>().read() })
    }
}

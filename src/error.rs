use std::ffi::NulError;

use thiserror::Error;

use crate::{ServerVersion, stream::proto::control::peer::PacketSendError};

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum MoonlightError {
    // --- Moonlight ---
    // -- Session Configuration
    #[error("hdr not supported")]
    SettingHdrNotSupported,
    #[error("4k not supported")]
    Setting4kNotSupported,
    #[error("4k not supported: Your device must support HEVC or AV1 to stream at 4k")]
    Setting4kNotSupportedCodecMissing,
    #[error("4k not supported: Update GeForce Experience")]
    Setting4kNotSupportedUpdateGfe,
    // -- HTTP / HTTPS request and responses
    #[error("the host is offline")]
    HostOffline,
    #[error("the host returned an unsuccessful status code({code}): {reason}")]
    StatusCode { code: i32, reason: String },
    // -- Pairing
    #[error("another device is currently pairing with the server")]
    PairingFailedAlreadyInProgress,
    #[error("failed to pair because the pin was incorrect")]
    PairingFailedWrongPin,
    #[error("failed")]
    PairingFailed,
    #[error("this action requires pairing")]
    NotPaired,
    // -- Moonlight Shared
    #[error("the server version is not supported: {0}")]
    ServerVersionNotSupported(ServerVersion),
    #[error("failed to send packet: {0}")]
    PacketSend(#[from] PacketSendError),
    // -- Moonlight Common C
    #[error("couldn't aquire an instance")]
    InstanceAquire,
    #[error("a connection is already active")]
    ConnectionAlreadyExists,
    #[error("an error happened whilst sending an event")]
    EventSendError(i32),
    #[error("this call requires a GFE version which uses ENet")]
    ENetRequired,
    #[error("a string contained a nul byte which is not allowed in c strings")]
    StringNulError(#[from] NulError),
    #[error("couldn't establish a connection")]
    ConnectionFailed,
    // -- Moonlight Proto (custom rust impl)
    #[error("couldn't establish a connection")]
    ConnectionTimeout,
    // -- Crypto
    #[error("failed to decrypt data: {0}")]
    DecryptFailed(&'static str),
    // --- WebRTC ---
    #[error("session parse")]
    Session(#[from] sdp_types::ParserError),
    #[error("{context}: missing required attribute {attribute:?}")]
    MissingAttribute {
        context: &'static str,
        attribute: &'static str,
    },
    #[error("{context}: invalid attribute for {attribute:?}: got {got:?}, expected: {expected}")]
    InvalidAttribute {
        context: &'static str,
        attribute: &'static str,
        expected: &'static str,
        got: String,
    },
    #[error("invalid mode: {0}")]
    InvalidVideoMode(String),
    #[error("{context}: missing payload")]
    MissingPayload { context: &'static str },
    #[error("link header is missing the uri")]
    MissingUri,
    #[error("invalid link header")]
    InvalidLinkHeader,
    // --- Other ---
    #[error("other: {0}")]
    Other(#[from] Box<dyn std::error::Error + Send + Sync>),
}

pub(crate) fn parse_u32(
    context: &'static str,
    attribute: &'static str,
    value: &str,
) -> Result<u32, MoonlightError> {
    value.parse().map_err(|_| MoonlightError::InvalidAttribute {
        context,
        attribute,
        expected: "a valid positive number or 0 (u32)",
        got: value.to_string(),
    })
}

pub(crate) fn parse_number_as_bool(
    context: &'static str,
    attribute: &'static str,
    value: &str,
) -> Result<bool, MoonlightError> {
    match value {
        "0" => Ok(false),
        "1" => Ok(true),
        _ => Err(MoonlightError::InvalidAttribute {
            context,
            attribute,
            expected: "0 or 1",
            got: value.to_string(),
        }),
    }
}

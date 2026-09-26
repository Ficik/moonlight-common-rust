use std::{
    ffi::NulError,
    io,
    net::{Ipv4Addr, Ipv6Addr},
};

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
    #[error(
        "the https client doesn't have the required credentials, set them before making a request that requires authentication"
    )]
    Unauthenticated,
    #[error("couldn't establish a connection")]
    ConnectionTimeout,
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
    // -- Crypto
    #[error("failed to decrypt data: {0}")]
    DecryptFailed(&'static str),
    // --- Parsing ---
    #[error("session parse")]
    Session(#[from] sdp_types::ParserError),
    #[error("{context}: missing required attribute {attribute:?}")]
    MissingAttribute {
        context: &'static str,
        attribute: &'static str,
    },
    #[error("{context}: invalid attribute for {attribute:?}: got {got:?}, expected {expected}")]
    InvalidAttribute {
        context: &'static str,
        attribute: &'static str,
        expected: &'static str,
        got: String,
    },
    #[error("{context}: failed to parse value: got {got:?}, expected {expected}")]
    InvalidValue {
        context: &'static str,
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
    #[error("io: {0}")]
    Io(#[from] io::Error),
    #[error("other: {0}")]
    Other(#[from] Box<dyn std::error::Error + Send + Sync>),
}

fn parse_error(
    context: &'static str,
    attribute: impl Into<Option<&'static str>>,
    expected: &'static str,
    value: String,
) -> MoonlightError {
    if let Some(attribute) = attribute.into() {
        MoonlightError::InvalidAttribute {
            context,
            attribute,
            expected,
            got: value,
        }
    } else {
        MoonlightError::InvalidValue {
            context,
            expected,
            got: value,
        }
    }
}

pub(crate) fn parse_u32(
    context: &'static str,
    attribute: impl Into<Option<&'static str>>,
    value: &str,
) -> Result<u32, MoonlightError> {
    value.parse().map_err(|_| {
        parse_error(
            context,
            attribute,
            "a valid positive number or 0 (u32)",
            value.to_string(),
        )
    })
}
pub(crate) fn parse_u16(
    context: &'static str,
    attribute: impl Into<Option<&'static str>>,
    value: &str,
) -> Result<u16, MoonlightError> {
    value.parse().map_err(|_| {
        parse_error(
            context,
            attribute,
            "a valid positive number or 0 (u16)",
            value.to_string(),
        )
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

pub(crate) fn parse_ipv4(
    context: &'static str,
    attribute: impl Into<Option<&'static str>>,
    value: &str,
) -> Result<Ipv4Addr, MoonlightError> {
    value.parse().map_err(|_| {
        parse_error(
            context,
            attribute,
            "a valid ipv4 address",
            value.to_string(),
        )
    })
}

pub(crate) fn parse_ipv6(
    context: &'static str,
    attribute: impl Into<Option<&'static str>>,
    value: &str,
) -> Result<Ipv6Addr, MoonlightError> {
    value.parse().map_err(|_| {
        parse_error(
            context,
            attribute,
            "a valid ipv6 address",
            value.to_string(),
        )
    })
}

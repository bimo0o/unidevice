use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Invalid(String),
    Unsupported(String),
    NotFound,
    Ambiguous,
    Access,
    Disconnected,
    Timeout,
    ShortWrite,
    Protocol(String),
    Rejected(u8),
    Unverified,
    Io(String),
    Safety(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(s) => write!(f, "invalid input: {s}"),
            Self::Unsupported(s) => write!(f, "unsupported: {s}"),
            Self::NotFound => write!(f, "device not found; reconnect and run list again"),
            Self::Ambiguous => write!(f, "multiple HID collections; select --device from list"),
            Self::Access => write!(
                f,
                "cannot open HID collection; check permissions or close vendor software"
            ),
            Self::Disconnected => write!(
                f,
                "HID I/O failed: device disconnected, unavailable or access denied"
            ),
            Self::Timeout => write!(f, "device response timed out"),
            Self::ShortWrite => write!(f, "device accepted an incomplete report"),
            Self::Protocol(s) => write!(f, "invalid device response: {s}"),
            Self::Rejected(code) => write!(f, "device rejected request (status 0x{code:02X})"),
            Self::Unverified => write!(
                f,
                "hardware access is unverified; use --experimental with an explicit operation"
            ),
            Self::Io(s) => write!(f, "file error: {s}"),
            Self::Safety(s) => write!(f, "device protection blocked operation: {s}"),
        }
    }
}

impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;

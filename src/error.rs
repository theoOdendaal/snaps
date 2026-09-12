// FIXME: I need to drastically improve this module,
//
// as my errors are way too generic. I basically have
// to use println's to identify where the problem happended.
// This isn't java, I should easiy be able to identify the
// problem by looking at the error only.
// I think I need to read up a bit more about error handling.

use std::path::PathBuf;

#[derive(Debug)]
pub enum Error {

    /// Failed because an existing snapshot with the
    /// same timestamp already exists.
    ExistingSnapshot(u64),
    
    /// An error returned when output from
    /// duration_since or elapsed methods
    /// on SystemTime is not positive.
    SnapShotTime(std::time::SystemTimeError),

    Io { context: String, kind: std::io::ErrorKind },

    ParseInt(std::num::ParseIntError),

    FromStrError(String),

    UnknownTag(String),

    EmptyTag(String),

    InvalidPath { path: PathBuf},

}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ExistingSnapshot(err) => write!(
                f,
                "Unable to create new snapshot for {}, due to it already existing",
                err
            ),
            Self::SnapShotTime(err) => write!(f, "Snapshot time error: {err}"),
            Self::Io { context, kind } => write!(f, "I/O error {kind} from {context}"),
            Self::ParseInt(err) => write!(f, "parse int error: {err}"),
            Self::FromStrError(err) => write!(f, "{err}"),
            Self::UnknownTag(err) => write!(f, "Unknown tag: {err}"),
            Self::EmptyTag(err) => write!(f, "{err}"),
            Self::InvalidPath { path } => write!(f, "{}", path.to_string_lossy()),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ExistingSnapshot(_) => None,
            Self::SnapShotTime(err) => Some(err),
            Self::Io { context: _, kind: _ } => None,
            Self::ParseInt(err) => Some(err),
            Self::FromStrError(_) => None,
            Self::UnknownTag(_) => None,
            Self::EmptyTag(_) => None,
            Self::InvalidPath { path: _ } => None,
        }
    }
}

pub trait WithContext<'a, T> {
    fn with_context(self, context: &'a str) -> Result<T, Error>;
}

// Propogate std::io::Error with context.
impl<'a, T> WithContext<'a, T> for std::io::Result<T> {

    fn with_context(self, context: &'a str) -> Result<T, Error> {
        self.map_err(|source| Error::Io { context: context.into(), kind: source.kind() })
    }
}

impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Self::Io { context: "".into(), kind: value.kind() }
    }
}


impl From<std::time::SystemTimeError> for Error {
    fn from(value: std::time::SystemTimeError) -> Self {
        Self::SnapShotTime(value)
    }
}

impl From<std::num::ParseIntError> for Error {
    fn from(value: std::num::ParseIntError) -> Self {
        Self::ParseInt(value)
    }
}

#[derive(Debug)]
pub enum Error {
    SnapShotTime(std::time::SystemTimeError),
    Io(std::io::Error),
    ParseIntError(std::num::ParseIntError),
    FromStrError(String),
    InvalidSnapshot(u64),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SnapShotTime(err) => write!(f, "Snapshot time error: {}", err),
            Self::Io(err) => write!(f, "i/o error: {}", err),
            Self::ParseIntError(err) => write!(f, "parse int error: {}", err),
            Self::FromStrError(err) => write!(f, "{}", err),
            Self::InvalidSnapshot(err) => write!(f, "Invalid snapshot: {}", err),


        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::SnapShotTime(err) => Some(err),
            Self::Io(err) => Some(err),
            Self::ParseIntError(err) => Some(err),
            Self::FromStrError(_) => None,
            Self::InvalidSnapshot(_) => None,
        }
    }
}

impl From<std::time::SystemTimeError> for Error {
    fn from(value: std::time::SystemTimeError) -> Self {
        Self::SnapShotTime(value)
    }
}

impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<std::num::ParseIntError> for Error {
    fn from(value: std::num::ParseIntError) -> Self {
        Self::ParseIntError(value)
    }
}

use std::fmt::Formatter;
use std::fmt::Display;
use std::io::ErrorKind;
use std::sync::TryLockError;
use std::sync::PoisonError;

#[derive(Debug)]
pub enum Error {
    NotFound,
    NotPermitted,
    Busy,
    ParseError,
    TooLarge,
    Other(Box<dyn std::error::Error + Send + Sync>),
    Misc(String)
}

impl Error {
    pub fn other<E: std::error::Error + Send + Sync + 'static>(e: E) -> Self {
        Self::Other(Box::new(e))
    }
    
    pub fn misc<E: AsRef<str>>(e: E) -> Self {
        Self::Misc(e.as_ref().to_owned())
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(self, f)
    }
}

impl std::error::Error for Error {}

impl<E: 'static> From<TryLockError<E>> for Error {
    fn from(value: TryLockError<E>) -> Self {
        match value {
            TryLockError::WouldBlock => Self::Busy,
            TryLockError::Poisoned(e) => Self::misc("PoisonError")
        }
    }
}

impl<E: 'static> From<PoisonError<E>> for Error {
    fn from(value: PoisonError<E>) -> Self {
        Self::misc("PoisonError")
    }
}

impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        match value.kind() {
            ErrorKind::NotFound => Self::NotFound,
            ErrorKind::PermissionDenied => Self::NotPermitted,
            _ => Self::other(value)
        }
    }
}


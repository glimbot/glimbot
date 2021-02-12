use std::error::Error as StdErr;
use std::fmt;
use std::ops::Deref;
use std::fmt::Formatter;
use std::result::Result as StdRes;

pub trait LogErrorExt {
    fn log_error(&self);
}

pub struct Error {
    err: Box<dyn StdErr + Send>,
    user_error: bool
}

impl Error {
    pub fn from_err<T: StdErr + Send + Sized + 'static>(e: T, user_error: bool) -> Self {
        Self { err: Box::new(e), user_error }
    }

    pub const fn is_user_error(&self) -> bool {
        self.user_error
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let e = self.err.deref();
        write!(f, "{}", e)
    }
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let e = self.err.deref();
        write!(f, "{:?}", e)
    }
}

impl StdErr for Error {}

#[derive(Debug)]
pub struct UserError {
    info: String
}

impl UserError {
    pub fn new(info: impl Into<String>) -> Self {
        UserError { info: info.into() }
    }
}

impl fmt::Display for UserError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", &self.info)
    }
}

#[derive(Debug)]
pub struct SysError {
    info: String
}

impl SysError {
    pub fn new(info: impl Into<String>) -> Self {
        SysError { info: info.into() }
    }
}

impl fmt::Display for SysError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", &self.info)
    }
}

impl StdErr for SysError {}
impl StdErr for UserError {}

pub type Result<T> = StdRes<T, Error>;

impl<T> LogErrorExt for Result<T> {
    fn log_error(&self) {
        if let Err(e) = self {
            if e.is_user_error() {
                trace!("{}", e);
            } else {
                error!("{}", e);
            }
        }
    }
}

pub trait IntoBotErr<T>: Sized {
    fn into_user_err(self) -> Result<T>;
    fn into_sys_err(self) -> Result<T>;
}

impl <T, E> IntoBotErr<T> for StdRes<T, E> where E: StdErr + Send + Sized + 'static {
    fn into_user_err(self) -> Result<T> {
        self.map_err(|e| Error::from_err(e, true))
    }

    fn into_sys_err(self) -> Result<T> {
        self.map_err(|e| Error::from_err(e, false))
    }
}

/// Creates a wrapper around an error type that we can just assume isn't a user error
/// (should not be shown to user)
#[macro_export]
macro_rules! impl_std_from {
    ($($src:path),+) => {
        $(
        impl From<$src> for Error {
            fn from(s: $src) -> Self {
                Self::from_err(s, false)
            }
        }
        )+
    };
}

impl_std_from! {
    sled::Error,
    bincode::Error,
    SysError,
    std::io::Error,
    serenity::Error
}

#[macro_export]
macro_rules! impl_user_err_from {
    ($($src:path),+) => {
        $(
        impl From<$src> for $crate::error::Error {
            fn from(s: $src) -> Self {
                Self::from_err(s, true)
            }
        }
        )+
    };
}

impl_user_err_from! {
    UserError
}
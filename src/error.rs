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
        if !user_error {
            error!("{}", &e);
        }
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

pub trait SerenityErrExt: Sized {
    fn into_glim_err(self) -> Error;
}

impl From<serenity::Error> for Error {
    fn from(e: serenity::Error) -> Self {
        match e {
            serenity::Error::Model(me) => {
                match me {
                    serenity::model::ModelError::MessageTooLong(_) => {Self::from_err(me, false)}
                    me => Self::from_err(me, true)
                }
            }
            e => Self::from_err(e, false)
        }
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
    rmp_serde::decode::Error,
    rmp_serde::encode::Error,
    SysError,
    std::io::Error
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

#[macro_export]
macro_rules! impl_err {
    ($name:ident, $message:expr, $user_error:expr) => {
        #[derive(Debug)]
        pub struct $name;

        impl ::std::fmt::Display for $name {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                write!(f, "{}", $message)
            }
        }

        impl ::std::error::Error for $name {}

        impl From<$name> for $crate::error::Error {
            fn from(s: $name) -> Self {
                Self::from_err(s, $user_error)
            }
        }
    };
}

impl_err!(GuildNotInCache, "Couldn't find guild in cache.", false);
impl_err!(RoleNotInCache, "Couldn't find role in cache.", false);
impl_err!(InsufficientPermissions, "You do not have the permissions to run this command.", true);
impl_err!(DeputyConfused, "Your role is not high enough in the hierarchy to do that.", true);
use std::error::Error as StdErr;
use std::fmt;
use std::ops::Deref;
use std::fmt::Formatter;
use std::result::Result as StdRes;

pub trait BotError: StdErr {
    fn is_user_error(&self) -> bool;
}

pub trait LogErrorExt {
    fn log_error(&self);
}

pub enum Error {
    Std { src: Box<dyn StdErr>, user_error: bool },
    Bot(Box<dyn BotError>),
}

impl Error {
    pub fn from_std_err<T: StdErr + Sized + 'static>(e: T, user_err: bool) -> Self {
        Self::Std {
            src: Box::new(e),
            user_error: user_err
        }
    }

    pub fn from_bot_err<T: BotError + Sized + 'static>(e: T) -> Self {
        Self::Bot(Box::new(e))
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let e = self.deref();
        write!(f, "{}", e)
    }
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let e = self.deref();
        write!(f, "{:?}", e)
    }
}

impl StdErr for Error {}

impl StdErr for Box<dyn BotError> {}

impl BotError for Error {
    fn is_user_error(&self) -> bool {
        match self {
            Error::Std { user_error, .. } => {*user_error}
            Error::Bot(e) => { e.is_user_error() }
        }
    }
}

impl BotError for Box<dyn BotError> {
    fn is_user_error(&self) -> bool {
        self.as_ref().is_user_error()
    }
}

impl AsRef<dyn StdErr> for Error {
    fn as_ref(&self) -> &(dyn StdErr + 'static) {
        match self {
            Error::Std { src, .. } => {src.as_ref()}
            Error::Bot(e) => {
                e as &dyn StdErr
            }
        }
    }
}

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

// Creates a wrapper around an error type that we can just assume isn't a user error
// (should not be shown to user)
macro_rules! impl_std_from {
    ($($src:path),+) => {
        $(
        impl From<$src> for Error {
            fn from(s: $src) -> Self {
                Self::from_std_err(s, false)
            }
        }
        )+
    };
}

impl_std_from!{
    sled::Error
}

macro_rules! impl_bot_from {
    ($($src:path),+) => {
        $(
        impl From<$src> for Error {
            fn from(s: $src) -> Self {
                Self::from_bot_err(s, false)
            }
        }
        )+
    };
}
//! Defines the [`Error`] type for glimbot, wrapping most external error types and marking them
//! whether or not they should be displayed to the user. This is the preferred error type for glimbot
//! actions.

use std::borrow::Cow;
use std::error::Error as StdErr;
use std::fmt;
use std::fmt::Formatter;
use std::ops::Deref;
use std::result::Result as StdRes;

/// Extension trait for [`Result`] to enable easy logging of errors.
///
/// [`Result`]: std::error::Result
pub trait LogErrorExt {
    /// If the result contains a user error, logs it at trace level. Otherwise, logs it at error level.
    fn log_error(&self);
}

/// Wrapper type for errors in glimbot. Errors may be marked as being a user_error; this affects
/// the level at which they are logged locally and whether or not the user receives
/// the error's full output or just a generic "error occurred" message.
pub struct Error {
    /// The wrapped error.
    err: Box<dyn StdErr + Send>,
    /// Whether or not this error should be displayed directly to users.
    user_error: bool
}

impl Error {
    /// Converts a standard error type into an [`Error`].
    pub fn from_err<T: StdErr + Send + Sized + 'static>(e: T, user_error: bool) -> Self {
        if !user_error {
            error!("{}", &e);
        }
        Self { err: Box::new(e), user_error }
    }

    /// Returns true if this error should be displayed directly to users.
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

/// Simple wrapper for user errors. Deprecated in favor of specific errors from the [`impl_err`] macro.
#[derive(Debug)]
pub struct UserError {
    /// Info string to display to the user.
    info: String
}

impl UserError {
    /// Creates a user error from the given displayable.
    #[deprecated]
    pub fn new(info: impl fmt::Display) -> Self {
        UserError { info: info.to_string() }
    }
}

impl fmt::Display for UserError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", &self.info)
    }
}

/// Wrapper for system-level errors. Deprecated in favor of specific errors from the [`impl_err`] macro.
#[derive(Debug)]
pub struct SysError {
    /// Info string to display in the logs
    info: String
}

impl SysError {
    /// Creates a sys error from the given displayable.
    #[deprecated]
    pub fn new(info: impl fmt::Display) -> Self {
        SysError { info: info.to_string() }
    }
}

impl fmt::Display for SysError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", &self.info)
    }
}

impl StdErr for SysError {}
impl StdErr for UserError {}

/// Result type with [`Error`] as the error type.
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

/// Converts standard result types into bot error types.
pub trait IntoBotErr<T>: Sized {
    /// Converts a result into a user error.
    fn into_user_err(self) -> Result<T>;
    /// Converts a result into a system error.
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
    serde_json::Error,
    sqlx::Error,
    SysError,
    std::io::Error,
    dotenv::Error,
    tracing::subscriber::SetGlobalDefaultError,
    std::env::VarError,
    sqlx::migrate::MigrateError
}

/// Implements [`From<Error>`] for a type, with `user_error` set to true
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

/// Implements [`From<Error>`] for a type, with `user_error` set to false
#[macro_export]
macro_rules! impl_err {
    ($name:ident, $message:expr, $user_error:expr) => {
        #[doc = $message]
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
impl_err!(DeputyConfused, "Performing that action would confuse the deputy. See https://en.wikipedia.org/wiki/Confused_deputy_problem for an explanation.", true);

/// Extension trait for [`sqlx::Error`]
pub trait DatabaseError {
    /// Returns the name of the constraint violated if this was a constraint issue.
    fn constraint(&self) -> Option<&str>;
    /// Returns whether or not this error is a constraint violation.
    fn is_constraint(&self) -> bool;
    /// Returns whether or not this error is a `UNIQUE` constraint violation.
    fn is_unique(&self) -> bool;
    /// Returns whether or not this error is a `CHECK` constraint violation.
    fn is_check(&self) -> bool;
    /// Returns the several digit string representing what error occurred
    fn sqlstate(&self) -> Option<Cow<'_, str>>;
}

impl DatabaseError for sqlx::Error {
    fn constraint(&self) -> Option<&str> {
        match self {
            sqlx::Error::Database(d) => {
                d.constraint()
            }
            _ => {
                None
            }
        }
    }

    fn is_constraint(&self) -> bool {
        self.sqlstate().map_or(false, |c| c.starts_with("23"))
    }

    fn is_unique(&self) -> bool {
        self.sqlstate().map_or(false, |c| c.starts_with("23505"))
    }

    fn is_check(&self) -> bool {
        self.sqlstate().map_or(false, |c| c.starts_with("23515"))
    }

    fn sqlstate(&self) -> Option<Cow<'_, str>> {
        match self {
            sqlx::Error::Database(d) => {
                d.code()
            }
            _ => {
                None
            }
        }
    }
}
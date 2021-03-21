//! Contains misc utility extension traits and types.

use std::ffi::OsString;

use structopt::StructOpt;

use crate::error::{IntoBotErr, UserError};

pub mod constraints;
pub mod clock;

/// An extension trait to allow for extraction of the help string from command invocations,
/// as well as converting errors into Glimbot errors.
pub trait ClapExt: StructOpt + Sized {
    /// Extracts help text and converts errors into Glimbot errors. Additionally strips
    /// ANSI escapes from the help text.
    fn from_iter_with_help<I>(i: I) -> crate::error::Result<Self> where I: IntoIterator,
                                      I::Item: Into<OsString> + Clone {
        let opts = Self::from_iter_safe(i);
        match opts {
            Err(e) => {
                let b = e.to_string();
                let escaped = strip_ansi_escapes::strip(b).into_sys_err()?;
                let b = String::from_utf8_lossy(&escaped);
                #[allow(deprecated)]
                Err(UserError::new(b).into())
            },
            Ok(s) => Ok(s)
        }
    }
}

impl<T> ClapExt for T where T: StructOpt + Sized {}

/// Allows flipping `Option<Result<T, E>>` into `Result<Option<T>, E>`.
pub trait FlipResultExt<T, E>: Sized {
    /// Flips as specified in the type documentation.
    fn flip(self) -> Result<Option<T>, E>;
}

impl<T, E> FlipResultExt<T, E> for Option<Result<T, E>> {
    fn flip(self) -> Result<Option<T>, E> {
        self.map_or(Ok(None), |r| r.map(Some))
    }
}
//! Contains misc utility extension traits and types.

use std::ffi::OsString;

use structopt::StructOpt;

use crate::error::{IntoBotErr, UserError};
use noisy_float::types::R64;

pub mod constraints;
pub mod clock;
pub mod ordset;

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


pub trait CoalesceResultExt<T>: Sized {
    fn coalesce(self) -> T;
}

impl<T> CoalesceResultExt<T> for Result<T, T> {
    fn coalesce(self) -> T {
        match self {
            Ok(v) => {v}
            Err(v) => {v}
        }
    }
}

impl_err!(NeedNonNegativeFloat, "expected a non-negative real number", true);

pub fn parse_nonnegative_real(s: &str) -> crate::error::Result<R64> {
    let f = s.parse::<f64>()?;
    let r = R64::try_new(f)
        .ok_or(NeedNonNegativeFloat)?;

    if r < 0.0 {
        Err(NeedNonNegativeFloat.into())
    } else {
        Ok(r)
    }
}
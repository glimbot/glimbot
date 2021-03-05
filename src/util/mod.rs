use std::ffi::OsString;

use structopt::StructOpt;

use crate::error::{IntoBotErr, UserError};

pub mod constraints;

pub type HelpString = String;
pub trait ClapExt: StructOpt + Sized {
    fn from_iter_with_help<I>(i: I) -> crate::error::Result<Self> where I: IntoIterator,
                                      I::Item: Into<OsString> + Clone {
        let opts = Self::from_iter_safe(i);
        match opts {
            Err(e) => {
                let b = e.to_string();
                let escaped = strip_ansi_escapes::strip(b).into_sys_err()?;
                let b = String::from_utf8_lossy(&escaped);
                Err(UserError::new(b).into())
            },
            Ok(s) => Ok(s)
        }
    }
}

impl<T> ClapExt for T where T: StructOpt + Sized {}

pub trait FlipResultExt<T, E>: Sized {
    fn flip(self) -> Result<Option<T>, E>;
}

impl<T, E> FlipResultExt<T, E> for Option<Result<T, E>> {
    fn flip(self) -> Result<Option<T>, E> {
        self.map_or(Ok(None), |r| r.map(Some))
    }
}
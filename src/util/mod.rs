use structopt::StructOpt;
use std::ffi::OsString;
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
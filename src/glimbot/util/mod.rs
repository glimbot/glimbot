use std::error::Error;

pub mod rate_limit;

pub trait FromError {
    fn from_error(e: impl Error + 'static) -> Self;
}
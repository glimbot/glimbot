use std::borrow::Cow;
use clap::{App, ArgMatches};

pub fn string_from_cow(s: Cow<'static, [u8]>) -> String {
    String::from_utf8(s.into_owned()).unwrap()
}
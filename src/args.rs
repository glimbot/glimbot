//  Glimbot - A Discord anti-spam and administration bot.
//  Copyright (C) 2020 Nick Samson

//  This program is free software: you can redistribute it and/or modify
//  it under the terms of the GNU General Public License as published by
//  the Free Software Foundation, either version 3 of the License, or
//  (at your option) any later version.

//  This program is distributed in the hope that it will be useful,
//  but WITHOUT ANY WARRANTY; without even the implied warranty of
//  MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
//  GNU General Public License for more details.

//  You should have received a copy of the GNU General Public License
//  along with this program.  If not, see <https://www.gnu.org/licenses/>.

//! Contains functionality related to parsing of commands.
//! This module and its functionality are deprecated in favor of [modules][crate::modules]

use clap::{App, ArgMatches};

#[doc(hidden)]
#[derive(thiserror::Error, Debug)]
pub enum ParseError {
    #[error("{0}")]
    Clap(#[from] clap::Error),
    #[error("An error occurred while parsing the arguments string: {0}")]
    Splitter(#[from] shell_words::ParseError),
}

#[doc(hidden)]
pub type Result<T> = std::result::Result<T, ParseError>;

static DUMMY: [&'static str; 1] = ["dummy"];

#[doc(hidden)]
pub fn parse_app_matches<'a, 'b>(s: impl AsRef<str>, a: &App<'a, 'b>) -> Result<ArgMatches<'a>> {
    let s = s.as_ref();
    let parts = shell_words::split(s)?;
    let app = a.clone();
    let matches = app.get_matches_from_safe(
        DUMMY.iter().cloned()
            .chain(parts.iter().map(|s| s.as_str())))?;
    Ok(matches)
}
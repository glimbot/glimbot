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
use crate::error::BotError;

/// Errors related to parsing commands
#[derive(thiserror::Error, Debug)]
pub enum ParseError {
    /// There was a non-help error involving a [clap::Error]
    #[error("{0}")]
    Clap(clap::Error),
    /// There was an issue parsing the arguments string.
    #[error("An error occurred while parsing the arguments string: {0}")]
    Splitter(#[from] shell_words::ParseError),
    /// The help message ended up being displayed.
    #[error("{0}")]
    DisplayHelp(String)
}

impl ParseError {
    /// Returns whether or not this is a DisplayHelp
    pub fn is_help(&self) -> bool {
        matches!(self, ParseError::DisplayHelp(_))
    }

    /// Converts this into the contained help message if [ParseError::is_help] would have returned true.
    pub fn into_help(self) -> Option<String> {
        match self {
            ParseError::DisplayHelp(s) => Some(s),
            _ => None
        }
    }
}

impl BotError for ParseError {
    fn is_user_error(&self) -> bool {
        true
    }
}

impl From<ParseError> for crate::modules::commands::Error {
    fn from(e: ParseError) -> Self {
        crate::modules::commands::Error::RuntimeFailure(e.into())
    }
}

impl From<clap::Error> for ParseError {
    fn from(e: clap::Error) -> Self {
        match &e.kind {
            clap::ErrorKind::HelpDisplayed => ParseError::DisplayHelp(e.to_string()),
            _ => ParseError::Clap(e)
        }
    }
}

#[doc(hidden)]
pub type Result<T> = std::result::Result<T, ParseError>;

#[doc(hidden)]
pub fn parse_app_matches<'a, 'b>(name: impl AsRef<str>, s: impl AsRef<str>, a: &App<'a, 'b>) -> Result<ArgMatches<'a>> {
    let s = s.as_ref();
    let parts = shell_words::split(s)?;
    let app = a.clone();
    let matches = app.get_matches_from_safe(
        [name.as_ref()].iter().cloned()
            .chain(parts.iter().map(|s| s.as_str())))?;
    Ok(matches)
}
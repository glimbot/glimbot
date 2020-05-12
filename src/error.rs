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

//! Contains types related to error handling in Glimbot

use std::error::Error;
use std::fmt;

/// A trait common to all errors used in the bot.
pub trait BotError: Error {
    /// Returns true if this error should be reported to the user, false if it should *only* be logged
    /// on the server side.
    fn is_user_error(&self) -> bool;
}

/// BotError style wrapper around [anyhow::Error]
#[derive(Debug)]
pub struct AnyError(anyhow::Error);

impl AnyError {
    /// Creates a new AnyError around the given error.
    pub fn new<T: Error + Send + Sync + 'static>(e: T) -> Self {
        AnyError(anyhow::Error::new(e))
    }

    /// Creates a boxed error around the given error.
    pub fn boxed<T: Error + Send + Sync + 'static>(e: T) -> Box<dyn BotError> {
        let out = Self::new(e);
        out.into()
    }
}

impl BotError for AnyError {
    fn is_user_error(&self) -> bool {
        false
    }
}

impl Error for AnyError {}
impl fmt::Display for AnyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl <T: BotError + 'static> From<T> for Box<dyn BotError> {
    fn from(t: T) -> Self {
        let o: Box<dyn BotError> = Box::new(t);
        o
    }
}

/// Alias for results returning a [BotError] more easily
pub type BotResult<T> = Result<T, Box<dyn BotError>>;
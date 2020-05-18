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
use serenity::http::StatusCode;
use std::ops::Deref;

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

/// [BotError] wrapper around [serenity::Error]
#[derive(Debug)]
pub struct SerenityError(serenity::Error);

impl BotError for SerenityError {
    fn is_user_error(&self) -> bool {
        self.forbidden()
    }
}

impl From<serenity::Error> for SerenityError {
    fn from(e: serenity::Error) -> Self {
        Self::new(e)
    }
}

impl From<SerenityError> for crate::modules::commands::Error {
    fn from(e: SerenityError) -> Self {
        crate::modules::commands::Error::RuntimeFailure(e.into())
    }
}

impl From<SerenityError> for crate::modules::hook::Error {
    fn from(e: SerenityError) -> Self {
        crate::modules::hook::Error::Failed(e.into())
    }
}

impl SerenityError {
    /// Wraps the given error.
    pub fn new(e: serenity::Error) -> Self {
        SerenityError(e)
    }

    /// Returns true if this is an HTTP 403 error.
    pub fn forbidden(&self) -> bool {
        self.unsuccessful_request().map(|e| e.status_code == StatusCode::FORBIDDEN)
            .unwrap_or(false)
    }

    /// Returns `Some(e)` if the underlying error is an http error.
    pub fn http_error(&self) -> Option<&serenity::http::error::Error> {
        match &self.0 {
            serenity::Error::Http(e) => Some(e.deref()),
            _ => None
        }
    }

    /// Returns `Some(e)` if the underlying error is an unsuccessful http response.
    pub fn unsuccessful_request(&self) -> Option<&serenity::http::error::ErrorResponse> {
        self.http_error().and_then(|e| match e {
            serenity::http::error::Error::UnsuccessfulRequest(e) => Some(e),
            _ => None
        })
    }
}

impl Error for SerenityError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.0 as &dyn Error)
    }
}

impl std::fmt::Display for SerenityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.forbidden() {
            write!(f, "Glimbot is not permitted to do that.")
        } else {
            write!(f, "{}", self.0)
        }
    }
}
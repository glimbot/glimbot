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

/// A trait common to all errors used in the bot.
pub trait BotError: Error {
    /// Returns true if this error should be reported to the user, false if it should *only* be logged
    /// on the server side.
    fn is_user_error(&self) -> bool;
}

impl <T: BotError + 'static> From<T> for Box<dyn BotError> {
    fn from(t: T) -> Self {
        let o: Box<dyn BotError> = Box::new(t);
        o
    }
}

/// Alias for results returning a [BotError] more easily
pub type BotResult<T> = Result<T, Box<dyn BotError>>;
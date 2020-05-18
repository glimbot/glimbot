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

//! Contains functionality related to processing commands from users.

use crate::dispatch::Dispatch;
use serenity::model::id::UserId;
use serenity::client::Context;
use serenity::model::prelude::Message;
use std::borrow::Cow;
use crate::error::{BotError, SerenityError};

/// Error types for running commands based on user input.
#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// The user did not have permission to perform the specified action.
    #[error("User {0} is not authorized to perform that action.")]
    InsufficientUserPerms(UserId),
    /// Glimbot did not have sufficient permissions to perform that action.
    #[error("Glimbot is missing required permissions to perform that action.")]
    InsufficientBotPerms,
    /// The command failed for some other reason unrelated to permissions.
    #[error("{0}")]
    RuntimeFailure(Box<dyn BotError>),
}

impl BotError for Error {
    fn is_user_error(&self) -> bool {
        match self {
            Error::RuntimeFailure(e) => {e.is_user_error()},
            _ => true
        }
    }
}

impl From<Box<dyn BotError>> for Error {
    fn from(e: Box<dyn BotError>) -> Self {
        Error::RuntimeFailure(e)
    }
}

impl From<serenity::Error> for Error {
    fn from(e: serenity::Error) -> Self {
        SerenityError::from(e).into()
    }
}

/// Alias for result of running commands.
pub type Result<T> = std::result::Result<T, Error>;

/// The trait from which commands are derived. Each module can have one command, which may have
/// subcommands as appropriate.
pub trait Command: Send + Sync {
    /// The primary entry point for the command.
    fn invoke(&self, disp: &Dispatch, ctx: &Context, msg: &Message, args: Cow<str>) -> Result<()>;

    /// Returns a help string for the given command, invoked by the "help" module.
    fn help(&self) -> Cow<'static, str> {
        Cow::Borrowed("No help specified.")
    }
}
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

//! Contains types related to event hooks for modules.

use std::borrow::Cow;
use crate::dispatch::Dispatch;
use serenity::prelude::Context;
use serenity::model::prelude::Message;
use crate::error::BotError;
use crate::db::DatabaseError;

/// Errors that can result from the application of a hook.
#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// The user who triggered the event cannot perform this action, for some reason other than needing a role.
    #[error("The action is forbidden.")]
    Denied,
    /// Denies an action with a reason.
    #[error("The action is forbidden: {0}")]
    DeniedWithReason(Cow<'static, str>),
    /// The user needed one of the specified roles (given by name) to perform the action.
    #[error("You need one of these roles to perform that action: {0:?}")]
    NeedRole(Vec<String>),
    /// The command specified does not exist.
    #[error("Command not found.")]
    CommandNotFound(String),
    /// The action failed for some backend reason.
    #[error("Failed while processing event. {0:?}")]
    Failed(Box<dyn BotError>)
}

impl From<Box<dyn BotError>> for Error {
    fn from(e: Box<dyn BotError>) -> Self {
        Error::Failed(e)
    }
}

impl BotError for Error {
    fn is_user_error(&self) -> bool {
        match self {
            Error::Failed(e) => {e.is_user_error()},
            _ => true
        }
    }
}

impl From<crate::db::DatabaseError> for Error {
    fn from(e: DatabaseError) -> Self {
        Error::Failed(e.into())
    }
}


/// The hook results alias.
pub type Result<T> = std::result::Result<T, Error>;

/// A function that will be called on every command invocation.
/// Example function signature:
/// ```
/// fn length_hook<'a, 'b, 'c, 'd>(disp: &'a Dispatch, ctx: &'b Context, msg: &'c Message, name: Cow<'d, str>) -> super::hook::Result<Cow<'d, str>>
/// ```
pub type CommandHookFn = for <'a, 'b, 'c, 'd> fn(&'a Dispatch, &'b Context, &'c Message, Cow<'d, str>) -> Result<Cow<'d, str>>;
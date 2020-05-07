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

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("User {0} is not authorized to perform that action.")]
    InsufficientUserPerms(UserId),
    #[error("Glimbot is missing required permissions to perform that action.")]
    InsufficientBotPerms,
    #[error("{0}")]
    RuntimeFailure(#[from] anyhow::Error),
    #[error("An unspecified error occurred while performing the action.")]
    Other
}

pub type Result<T> = std::result::Result<T, Error>;

pub trait Command : Send + Sync {
    fn name(&self) -> &str;
    fn invoke(&self, disp: &Dispatch, args: String) -> Result<()>;
}
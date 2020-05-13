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

//! Contains generic hooks that should be run before every command.

use crate::modules::{Module, hook, config};
use crate::dispatch::Dispatch;
use serenity::prelude::Context;
use serenity::model::prelude::Message;
use std::borrow::Cow;
use once_cell::sync::Lazy;
use regex::Regex;

const MAX_COMMAND_INVOCATION_LENGTH: usize = 1500; // Max length for messages to be passed to commands.

/// Prohibits commands of more than [MAX_COMMAND_INVOCATION_LENGTH] from being processed.
/// This is helpful to avoid breakages from things like the ping module which will probably
/// add length to the args.
pub fn length_hook<'a, 'b, 'c, 'd>(_disp: &'a Dispatch, _ctx: &'b Context, msg: &'c Message, name: Cow<'d, str>) -> super::hook::Result<Cow<'d, str>> {
    if msg.content.len() > MAX_COMMAND_INVOCATION_LENGTH {
        let message = format!("Argument string was too long ({} bytes). Must be less than {} bytes (not characters).",
                              msg.content.len(),
                              MAX_COMMAND_INVOCATION_LENGTH);
        Err(hook::Error::DeniedWithReason(Cow::from(message)))
    } else {
        Ok(name)
    }
}

/// We want to reject commands coming in outside of a guild context, since we don't really have a way to track
/// configuration and other things at the individual level.
pub fn reject_dm_command_hook<'a, 'b, 'c, 'd>(_disp: &'a Dispatch, _ctx: &'b Context, msg: &'c Message, name: Cow<'d, str>) -> super::hook::Result<Cow<'d, str>> {
    if msg.guild_id.is_none() {
        Err(hook::Error::DeniedWithReason(Cow::from("Commands must be run from inside a guild.")))
    } else {
        Ok(name)
    }
}

fn validate_command_prefix(s: &str) -> bool {
    static COMMAND_PREFIX_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"[\p{Math Symbol}\p{Punctuation}]").unwrap());
    COMMAND_PREFIX_RE.is_match(s)
}

/// Returns a module with hook functionality not related to any particular command module.
pub fn base_hooks() -> Module {
    Module::with_name("base_hooks")
        .with_command_hook(reject_dm_command_hook)
        .with_command_hook(length_hook)
        .with_config_value(config::Value::new("command_prefix",
                                              "The single character before a command.",
                                              validate_command_prefix,
                                              Some("!")))
        .clear_dependencies()
}
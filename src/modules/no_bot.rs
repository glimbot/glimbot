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

//! Checks to see if we should respond to bot command invocations.

use crate::dispatch::Dispatch;
use serenity::prelude::Context;
use serenity::model::prelude::Message;
use std::borrow::{Cow};
use crate::db::cache::get_cached_connection;
use crate::modules::hook::Error::DeniedWithReason;
use crate::modules::{Module, config};
use crate::modules::config::valid_bool;

static NO_BOT_KEY: &'static str = "ignore_bots";
const DEFAULT_VALUE: bool = false;

/// This hook prevents bots from running commands.
fn no_bot_hook<'a, 'b, 'c, 'd>(disp: &'a Dispatch, _ctx: &'b Context, msg: &'c Message, name: Cow<'d, str>) -> super::hook::Result<Cow<'d, str>> {
    let conn = get_cached_connection(msg.guild_id.unwrap())?;
    let rl = conn.as_ref().borrow();
    let bots_allowed = disp.get_or_set_config(&rl, NO_BOT_KEY)?.parse::<bool>().unwrap();
    if bots_allowed || (!bots_allowed && !msg.author.bot){
        Ok(name)
    } else {
        Err(DeniedWithReason(Cow::from("Bots are not allowed to issue commands in this server.")))
    }
}

/// This module prevents bots from running commands optionally.
pub fn deny_bot_mod() -> Module {
    Module::with_name("deny_bot")
        .with_config_value(
            config::Value::new(NO_BOT_KEY,
                               "Whether or not bots are allowed to send Glimbot commands. Default is false.",
                               valid_bool,
                               Some(DEFAULT_VALUE.to_string())))
        .with_command_hook(no_bot_hook)
        .with_sensitivity(true)
}

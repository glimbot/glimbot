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

//! Lists out the help for all the commands

use std::borrow::Cow;

use serenity::model::channel::Message;
use serenity::prelude::Context;

use crate::dispatch::Dispatch;
use crate::modules::commands::Command;
use crate::modules::commands::Result;
use crate::modules::Module;

/// Command that tells an user information about installed commands
#[derive(Copy, Clone, Debug)]
pub struct Help;

impl Command for Help {
    fn invoke(&self, disp: &Dispatch, ctx: &Context, msg: &Message, _args: Cow<str>) -> Result<()> {
        trace!("Help wanted from user {:?}", msg.author.id);

        msg.author
            .id
            .create_dm_channel(&ctx.http)?
            .send_message(&ctx.http, |msg| {
                msg.embed(|embed| {
                    embed.fields(disp.modules().values().filter_map(|module| {
                        Some((module.name(), module.command_handler()?.help(), false))
                    }))
                })
            })?;

        Ok(())
    }
}

/// Creates a new help module.
pub fn help_module() -> Module {
    Module::with_name("help")
        .with_command(Help)
        .with_sensitivity(false)
}

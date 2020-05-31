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
use serenity::utils::MessageBuilder;

use crate::dispatch::Dispatch;
use crate::error::AnyError;
use crate::modules::Module;
use crate::modules::commands::Command;
use crate::modules::commands::Result;

/// Command that tells an user information about installed commands
#[derive(Copy, Clone, Debug)]
pub struct Help;

impl Command for Help {
    fn invoke(&self, disp: &Dispatch, ctx: &Context, msg: &Message, _args: Cow<str>) -> Result<()> {
        trace!("Help wanted from user {:?}", msg.author.id);

        let mut builder = MessageBuilder::new();
        for module in disp.modules().values() {
            builder.push_bold_line_safe(module.name());

            builder.push_codeblock_safe(
                &module.command_handler.as_ref().map_or(Cow::from("No command"), |cmd| cmd.help()), 
                None
            );
        }

        let content = builder.build();
        msg.channel_id.say(&ctx.http, content).map_err(AnyError::boxed)?;
        Ok(())
    }
}

/// Creates a new help module.
pub fn help_module() -> Module {
    Module::with_name("help")
        .with_command(Help)
        .with_sensitivity(false)
}
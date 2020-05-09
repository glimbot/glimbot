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

//! A simple ping command to reflect input back to the user.

use crate::modules::commands::Command;
use crate::dispatch::Dispatch;
use serenity::utils::MessageBuilder;
use serenity::prelude::Context;
use serenity::model::channel::Message;
use crate::modules::commands::Result;
use crate::modules::Module;
use std::borrow::Cow;

/// A command that reflects user input back to the user.
#[derive(Copy, Clone, Debug)]
pub struct Ping;

impl Command for Ping {
    fn invoke(&self, _disp: &Dispatch, ctx: &Context, msg: &Message, args: Cow<str>) -> Result<()> {
        trace!("Ping from user {:?}", msg.author.id);
        let message = if args.len() > 0 {
            MessageBuilder::new()
                .push_codeblock_safe(args, None)
                .build()
        } else {
            String::from("Pong!")
        };
        msg.channel_id.say(&ctx.http, message).map_err(anyhow::Error::new)?;
        Ok(())
    }
}

/// Creates a new ping module.
pub fn ping_module() -> Module {
    Module::with_name("ping")
        .with_command(Ping)
}
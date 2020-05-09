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

//! Contains the primary event handler for Glimbot.

use serenity::prelude::{EventHandler, Context};
use serenity::model::gateway::{Ready, Activity};
use crate::util::LogErrorExt;
use crate::db::cache::get_cached_connection;
use serenity::model::prelude::UserId;
use std::sync::atomic::AtomicU64;
use crate::modules::hook::CommandHookFn;
use std::collections::HashMap;
use crate::modules::{Module, hook};
use crate::modules::commands::Command;
use serenity::model::channel::Message;
use once_cell::unsync::Lazy;
use regex::Regex;
use crate::error::{BotResult};
use serenity::utils::MessageBuilder;
use std::borrow::Cow;

pub mod args;

/// The primary event handler for Glimbot. Contains references to transient state for the bot.
/// Non-transient data should live in the databases.
pub struct Dispatch {
    owner: AtomicU64,
    modules: HashMap<String, Module>,
    command_hooks: Vec<CommandHookFn>
}

// Thread local because Regex just uses an interior mutex if used in multiple threads, blegh.
thread_local! {
    static CMD_REGEX: Lazy<Regex> = Lazy::new(
        || Regex::new(r#"([\p{Math Symbol}\p{Punctuation}])(\w+)(?:\s*)(.*)"#).unwrap()
    );
}

impl EventHandler for Dispatch {
    fn message(&self, ctx: Context, new_message: Message) {
        let res = self.handle_message(&ctx, &new_message);

        if let Err(e) = res {
            let msg = if e.is_user_error() {
                info!("{}", &e);
                MessageBuilder::new()
                    .push_codeblock_safe(e, None)
                    .build()
            } else {
                error!("{}", &e);
                MessageBuilder::new()
                    .push_codeblock_safe("The command failed on the backend. Please contact the bot admin if this persists.", None)
                    .build()
            };

            new_message.channel_id.say(&ctx, msg).log_error();
        }
    }

    fn ready(&self, ctx: Context, data_about_bot: Ready) {
        ctx.set_activity(Activity::playing("Cultist Simulator"));
        let active_guilds = &data_about_bot.guilds;
        active_guilds.iter().for_each(
            |g| get_cached_connection(g.id()).log_error()
        );


        info!("Glimbot is up and running in at least {} servers.", active_guilds.len());
    }
}

impl Dispatch {
    /// Creates a dispatch with the given owner.
    pub fn new(owner: UserId) -> Self {
        Dispatch {
            owner: AtomicU64::new(*owner.as_u64()),
            command_hooks: Vec::new(),
            modules: HashMap::new()
        }
    }

    /// Handles an incoming new message.
    pub fn handle_message(&self, ctx: &Context, new_message: &Message) -> BotResult<()> {
        if new_message.is_own(&ctx) {
            debug!("Saw a message from myself.");
            return Ok(());
        }
        trace!("Saw a message from user {}", &new_message.author);

        let msg = &new_message.content;

        if CMD_REGEX.with(|r|r.is_match(msg)) {
            // It's a command (probably).
            trace!("It's a command, probably.");
            let m = CMD_REGEX.with(|r|r.captures(msg)).unwrap();
            let sym = m.get(1).unwrap();
            let req_sym = if let Some(g) = &new_message.guild_id {
                let conn = get_cached_connection(*g)?;
                let r = conn.borrow();
                r.command_prefix()?
            } else {
                '!'
            };

            if sym.as_str().chars().next().unwrap() != req_sym {
                return Ok(());
            }

            let command_group = m.get(2).unwrap();

            let command_name: Cow<str> = self.command_hooks.iter().try_fold(
                Cow::Borrowed(command_group.as_str()),
                |acc, next: &CommandHookFn| next(self, ctx, new_message, acc)
            )?;

            let cmd = self.resolve_command(&command_name)
                .ok_or_else(|| hook::Error::CommandNotFound(command_name.into_owned()))?;

            let args = m.get(3).unwrap().as_str().trim().to_string();
            cmd.invoke(self, ctx, new_message, Cow::Owned(args))?;
        }


        Ok(())
    }

    /// Adds a module to the dispatcher.
    pub fn with_module(mut self, m: Module) -> Self {
        self.command_hooks.extend(m.command_hooks().iter());
        self.modules.insert(m.name().to_owned(), m);
        self
    }

    /// Resolves the given name to a command, if it exists.
    pub fn resolve_command(&self, cmd: impl AsRef<str>) -> Option<&dyn Command> {
        self.modules
            .get(cmd.as_ref())
            .and_then(|x|x.command_handler())
            .map(|x|x.as_ref())
    }
}
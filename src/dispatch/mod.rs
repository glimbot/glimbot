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
use std::collections::{HashMap, HashSet};
use crate::modules::{Module, hook};
use crate::modules::commands::Command;
use serenity::model::channel::Message;
use once_cell::unsync::Lazy;
use regex::Regex;
use crate::error::{BotResult};
use serenity::utils::MessageBuilder;
use std::borrow::Cow;
use crate::db::GuildConn;
use crate::modules::config;
use crate::modules::config::Validator;

pub mod args;

/// The primary event handler for Glimbot. Contains references to transient state for the bot.
/// Non-transient data should live in the databases.
pub struct Dispatch {
    owner: AtomicU64,
    modules: HashMap<String, Module>,
    command_hooks: Vec<CommandHookFn>,
    config_validator: config::Validator
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
                debug!("{}", &e);
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
            |g| debug!("We're in guild {}", g.id())
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
            modules: HashMap::new(),
            config_validator: Validator::new()
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
        info!("Loading module {} with {} command, {} hooks, {} config values.", m.name(),
            if m.command_handler().is_some() {
                "a"
            } else {
                "no"
            },
            m.command_hooks().len(),
            m.config_values().len()
        );

        let deps = m.dependencies();
        trace!("Module {} has dependencies {:?}", m.name(), deps);
        if deps.iter().any(|s| !self.modules.contains_key(s)) {
            let keyset: HashSet<String> = self.modules.keys()
                .into_iter()
                .map(|x| x.clone())
                .collect();
            let diff = deps.difference(&keyset);
            let missing_deps: HashSet<String> = diff.map(String::clone).collect();
            panic!("Attempted to load module {}, which depends on {:?}, but all of {:?} were missing.",
                m.name(),
                deps,
                missing_deps
            )
        }

        self.command_hooks.extend(m.command_hooks().iter());
        m.config_values().iter().for_each(|v| {
            debug!("Added config key {}", v.name());
            self.config_validator.add_value(v.clone())
        });
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

    /// Sets the config value to the given value after validating it.
    pub fn set_config(&self, conn: &GuildConn, key: impl AsRef<str>, value: impl AsRef<str>) -> BotResult<()> {
        self.config_validator.validate(key.as_ref(), value.as_ref())?;
        conn.set_value(key, value)?;
        Ok(())
    }

    /// Gets the config value. Fails if the key doesn't exist.
    pub fn get_config(&self, conn: &GuildConn, key: impl AsRef<str>) -> BotResult<String> {
        self.config_validator.check_key(key.as_ref())?;
        let o = conn.get_value(key)?;
        Ok(o)
    }

    /// Gets the config value or sets the config value and *then* returns it if it doesn't already exist.
    pub fn get_or_set_config(&self, conn: &GuildConn, key: impl AsRef<str>) -> BotResult<String> {
        self.config_validator.check_key(key.as_ref())?;
        let default = self.config_validator.default_for(key.as_ref())?;
        let o = conn.get_or_else_set_value(
            key.as_ref(), || default.clone()
        )?;

        Ok(o)
    }

    /// Accessor for the config validator.
    pub fn config_validator(&self) -> &config::Validator {
        &self.config_validator
    }
}
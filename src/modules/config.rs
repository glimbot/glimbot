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

//! The config module manages configuration values for Glimbot

use std::collections::HashMap;
use std::str::FromStr;
use crate::error::{BotError, AnyError};
use clap::{App, SubCommand, Arg, ArgMatches, AppSettings};
use crate::modules::commands::Command as Cmd;
use serenity::prelude::Context;
use serenity::model::channel::Message;
use crate::dispatch::Dispatch;
use std::borrow::Cow;
use crate::args::parse_app_matches;
use once_cell::unsync::Lazy;
use crate::db::cache::get_cached_connection;
use serenity::utils::MessageBuilder;
use crate::modules::Module;

/// A validation function for config values. Should return true if the value would be valid input.
pub type ConfigValidatorFn = fn(&str) -> bool;

/// Alias for config validation failures.
pub type Result<T> = std::result::Result<T, Error>;

/// Error returned when validation of a config key fails.
#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// There is no key with that name.
    #[error("No such configuration key: {0}")]
    NoSuchKey(String),
    /// The given input is not valid for that key.
    #[error("Config value is invalid: {0}")]
    InvalidValue(&'static str),
    /// There is no default value for the given value.
    #[error("There is no default value for that.")]
    NoDefault,
}

impl From<Error> for super::commands::Error {
    fn from(e: Error) -> Self {
        super::commands::Error::RuntimeFailure(e.into())
    }
}

impl BotError for Error {
    fn is_user_error(&self) -> bool {
        true
    }
}

/// The validation registry for validating config options.
pub struct Validator {
    validators: HashMap<&'static str, Value>
}

impl Validator {
    /// Creates a new, empty validator.
    pub fn new() -> Self {
        Validator {
            validators: HashMap::new()
        }
    }

    /// Validates the config value
    pub fn validate(&self, config_name: impl AsRef<str>, value: impl AsRef<str>) -> Result<()> {
        let cval = self.validators.get(config_name.as_ref())
            .ok_or(Error::NoSuchKey(config_name.as_ref().to_string()))?;
        if cval.is_valid(value.as_ref()) {
            Ok(())
        } else {
            Err(Error::InvalidValue(cval.name))
        }
    }

    /// Checks to see if this is a valid config key.
    pub fn check_key(&self, key: impl AsRef<str>) -> Result<()> {
        if self.validators.contains_key(key.as_ref()) {
            Ok(())
        } else {
            Err(Error::NoSuchKey(key.as_ref().to_string()))
        }
    }

    /// Adds a new config value
    pub fn add_value(&mut self, v: Value) {
        self.validators.insert(v.name, v);
    }

    /// Retrieves the default value for the key
    pub fn default_for(&self, key: impl AsRef<str>) -> Result<&String> {
        self.validators.get(key.as_ref())
            .and_then(|v| v.default())
            .ok_or(Error::NoDefault)
    }

    /// Retrieves the help for the given key.
    pub fn help_for(&self, key: impl AsRef<str>) -> Result<&'static str> {
        self.check_key(key.as_ref())?;
        Ok(self.validators.get(key.as_ref()).unwrap().help)
    }
}

/// Represents a validatable config value for Glimbot.
#[derive(Clone)]
pub struct Value {
    name: &'static str,
    help: &'static str,
    validator: ConfigValidatorFn,
    default: Option<String>,
}

impl Value {
    /// Creates a new Value.
    pub fn new(name: &'static str, help: &'static str, validator: ConfigValidatorFn, default: Option<impl Into<String>>) -> Self {
        Value { name, help, validator, default: default.map(|x| x.into()) }
    }

    /// Returns the name of the config key.
    pub fn name(&self) -> &'static str {
        self.name
    }

    /// Returns the help string for the config key.
    pub fn help(&self) -> &'static str {
        self.help
    }

    /// Returns whether or not the given value is a valid config value
    pub fn is_valid(&self, s: &str) -> bool {
        (self.validator)(s)
    }

    /// Returns an optional default setting for this config value.
    pub fn default(&self) -> Option<&String> {
        self.default.as_ref()
    }
}

/// Helper function to validate boolean config values.
pub fn valid_bool(s: &str) -> bool {
    valid_parseable::<bool>(s)
}

/// Helper function to validate any parseable ([FromStr]) config values.
pub fn valid_parseable<T: FromStr>(s: &str) -> bool {
    s.parse::<T>().is_ok()
}

/// Function for creating validators for parseable types
pub fn fallible_validator<T: FromStr<Err=E>, E: std::error::Error>(s: String) -> std::result::Result<(), String> {
    s.parse::<T>().map_err(|e| format!("{}", e)).map(|_|())
}

/// The config command structure. Contains the parser for command arguments.
pub struct Command;

thread_local! {
static PARSER: Lazy<App<'static, 'static>> = Lazy::new (
    || {
        let key_arg = Arg::with_name("config-key")
            .required(true)
            .help("The name of the configuration value to change.")
            .takes_value(true)
            .value_name("CONFIG_KEY");
        App::new("config")
            .about("This command allows you to set and view configuration values available for Glimbot.")
            .subcommand(SubCommand::with_name("set")
                .arg(key_arg.clone())
                .arg(Arg::with_name("value")
                    .required(true)
                    .takes_value(true)
                    .help("The new value to set the config key to.")
                    .value_name("VALUE")
                )
                .about("Sets CONFIG_KEY to the given value.")
            )
            .subcommand(
                SubCommand::with_name("get")
                    .arg(key_arg.clone())
                    .about("Retrieves the value of CONFIG_KEY for this guild.")
            )
            .subcommand(
                SubCommand::with_name("help")
                    .arg(key_arg.clone())
                    .about("Displays help for the given config value.")
            )
            .setting(AppSettings::SubcommandRequiredElseHelp)
    }
);
}

impl Cmd for Command {
    fn invoke(&self, disp: &Dispatch, ctx: &Context, msg: &Message, args: Cow<str>) -> crate::modules::commands::Result<()> {
        let m: ArgMatches = PARSER.with(|p|
            parse_app_matches("config", args, &p)
        )?;

        let reply = match m.subcommand() {
            ("help", Some(subm)) => {
                let key = subm.value_of("config-key").unwrap();
                let help = disp.config_validator().help_for(key)?;
                format!("{}: {}", key, help)
            },
            ("get", Some(subm)) => {
                let key = subm.value_of("config-key").unwrap();
                let conn = get_cached_connection(msg.guild_id.unwrap())?;
                let rl = conn.as_ref().borrow();
                let val = if disp.config_validator().default_for(key).is_ok() {
                    disp.get_or_set_config(&rl, key)?
                } else {
                    disp.get_config(&rl, key)?
                };

                format!("{} is set to {}", key, val)
            },
            ("set", Some(subm)) => {
                let key = subm.value_of("config-key").unwrap();
                let val = subm.value_of("value").unwrap();
                let conn = get_cached_connection(msg.guild_id.unwrap())?;
                let rl = conn.as_ref().borrow();
                disp.set_config(&rl, key, val)?;

                format!("Set {} to {}", key, val)
            }
            _ => unreachable!()
        };

        msg.channel_id.say(ctx, MessageBuilder::new()
            .push_codeblock_safe(reply, None)
            .build()
        ).map_err(AnyError::boxed)?;
        Ok(())
    }

    fn help(&self) -> Cow<'static, str> {
        let c = PARSER.with(|p| (*p).clone());
        let e = c.get_matches_from_safe(["config", "help"].iter());
        match e {
            Err(clap::Error { message, .. }) => {
                Cow::Owned(message)
            }
            _ => unreachable!()
        }
    }
}

/// Creates a config module [Module]
pub fn config_mod() -> Module {
    Module::with_name("config")
        .with_command(Command)
        .with_sensitivity(true)
}
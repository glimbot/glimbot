//! Contains logic for implementing modular functionality for glimbot.
//! Modules can contain up to one filter and up to one command, and may specify `0..n` configuration values.

use std::cmp::Ordering;
use std::fmt;
use std::fmt::Formatter;
use std::sync::Arc;

use serenity::client::Context;
use serenity::model::channel::Message;

use crate::dispatch::{config, Dispatch};

pub mod status;
pub mod owner;
pub mod base_filter;
pub mod shutdown;
pub mod privilege;
pub mod conf;
pub mod roles;
pub mod moderation;
pub mod spam;
pub mod mock_raid;

pub const CHECKMARK_IN_GREEN_BOX: char = '✅';

/// The sensitivity for a command.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Sensitivity {
    /// Anyone should be able to run at any time
    Low,
    /// Anyone can run, but prone to spamming
    Medium,
    /// Sensitive commands related to managing users/spam
    High,
    /// Commands only the bot owner should be able to run, like `shutdown`.
    Owner,
}

// TODO: Make this less messy. Should be refactorable into an ordinal comparison.
impl PartialOrd for Sensitivity {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match (self, other) {
            (x, y) if x == y => Some(Ordering::Equal),
            (Self::Owner, _) | (_, Self::Owner) => None,
            (Self::High, o) => match o {
                Sensitivity::Low |
                Sensitivity::Medium => { Ordering::Greater }
                Sensitivity::High => { Ordering::Equal }
                _ => unreachable!()
            }.into(),
            (Self::Medium, o) => match o {
                Sensitivity::Low => Ordering::Greater,
                Sensitivity::Medium => Ordering::Equal,
                Sensitivity::High => Ordering::Less,
                _ => unreachable!()
            }.into(),
            (Self::Low, o) => match o {
                Sensitivity::Low => { Ordering::Equal }
                Sensitivity::Medium |
                Sensitivity::High => { Ordering::Less }
                _ => unreachable!()
            }.into()
        }
    }
}

impl fmt::Display for Sensitivity {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let s = match self {
            Sensitivity::Low => { "low" }
            Sensitivity::Medium => { "medium" }
            Sensitivity::High => { "high" }
            Sensitivity::Owner => { "owner" }
        };
        f.write_str(s)
    }
}

/// Information about a module, like its name and sensitivity.
pub struct ModInfo {
    /// The publicly displayed name of the module, as well as the name
    /// of any specified command.
    pub name: &'static str,
    /// The sensitivity of the module. See [`Sensitivity`].
    pub sensitivity: Sensitivity,
    /// Whether or not this module has a filter specified.
    pub does_filtering: bool,
    /// Whether or not this module has a command specified.
    pub command: bool,
    /// Any configuration values related to this module.
    pub config_values: Vec<Arc<dyn config::Validator>>,
    /// Whether or not this module has an on_tick hook.
    pub on_tick: bool,
    /// Whether or not this message has an on_message hook.
    pub on_message: bool,
}

impl ModInfo {
    /// Creates a module with the specified name.
    pub fn with_name(name: &'static str) -> Self {
        ModInfo {
            name,
            sensitivity: Sensitivity::Owner,
            does_filtering: false,
            command: false,
            config_values: Vec::new(),
            on_tick: false,
            on_message: false
        }
    }

    /// Specifies whether or not this module has a command.
    pub fn with_command(mut self, command: bool) -> Self {
        self.command = command;
        self
    }

    /// Specifies a config value for this module.
    pub fn with_config_value(mut self, v: impl config::Validator) -> Self {
        self.config_values.push(Arc::new(v));
        self
    }

    /// Specifies whether or not this module does message filtering.
    pub fn with_filter(mut self, does_filtering: bool) -> Self {
        self.does_filtering = does_filtering;
        self
    }

    /// Specifies a sensitivity for the module.
    pub fn with_sensitivity(mut self, s: Sensitivity) -> Self {
        self.sensitivity = s;
        self
    }

    /// Specifies whether or not this module has a hook that runs every tick.
    pub fn with_tick_hook(mut self, with_hook: bool) -> Self {
        self.on_tick = with_hook;
        self
    }

    /// Specifies whether or not this module has a hook that runs on every message.
    pub fn with_message_hook(mut self, with_hook: bool) -> Self {
        self.on_message = with_hook;
        self
    }
}

impl_err!(UnimplementedModule, "This module hasn't been finished yet.", true);

/// The trait all modules need to implement.
#[async_trait::async_trait]
pub trait Module: Sync + Send {
    /// Returns meta information about the module.
    fn info(&self) -> &ModInfo;

    /// Applies a filter to the command. The name of the invoked command is specified;
    /// it can be changed or left unchanged, and should be returned if it is okay for the command
    /// to be invoked.
    ///
    /// If the command should not be invoked, this command should return an error.
    async fn filter(&self, _dis: &Dispatch, _ctx: &Context, _orig: &Message, name: String) -> crate::error::Result<String> {
        Ok(name)
    }

    /// Processes a command.
    async fn process(&self, _dis: &Dispatch, _ctx: &Context, _orig: &Message, _command: Vec<String>) -> crate::error::Result<()> {
        Err(UnimplementedModule.into())
    }

    /// Hook to run some command at a regular interval.
    async fn on_tick(&self, _dis: &Dispatch, _ctx: &Context) -> crate::error::Result<()> {
        Err(UnimplementedModule.into())
    }

    /// Hook to run on all messages.
    async fn on_message(&self, _dis: &Dispatch, _ctx: &Context, _orig: &Message) -> crate::error::Result<()> {
        Err(UnimplementedModule.into())
    }
}


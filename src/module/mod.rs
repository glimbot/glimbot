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
pub(crate) mod moderation;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Sensitivity {
    Low,
    // Anyone should be able to run at any time
    Medium,
    // Anyone can run, but prone to spamming
    High,
    // Sensitive commands related to managing users/spam,
    Owner, // Only owner should be able to run
}

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
                Sensitivity::Low => {Ordering::Equal}
                Sensitivity::Medium |
                Sensitivity::High => {Ordering::Less}
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

pub struct ModInfo {
    pub name: &'static str,
    pub sensitivity: Sensitivity,
    pub does_filtering: bool,
    pub command: bool,
    pub config_values: Vec<Arc<dyn config::Validator>>,
}

impl ModInfo {
    pub fn with_name(name: &'static str) -> Self {
        ModInfo {
            name,
            sensitivity: Sensitivity::Owner,
            does_filtering: false,
            command: false,
            config_values: Vec::new(),
        }
    }

    pub fn with_command(mut self, command: bool) -> Self {
        self.command = command;
        self
    }

    pub fn with_config_value(mut self, v: impl config::Validator) -> Self {
        self.config_values.push(Arc::new(v));
        self
    }

    pub fn with_filter(mut self, does_filtering: bool) -> Self {
        self.does_filtering = does_filtering;
        self
    }

    pub fn with_sensitivity(mut self, s: Sensitivity) -> Self {
        self.sensitivity = s;
        self
    }
}

impl_err!(UnimplementedModule, "This module hasn't been finished yet.", true);

#[async_trait::async_trait]
pub trait Module: Sync + Send {
    fn info(&self) -> &ModInfo;

    async fn filter(&self, _dis: &Dispatch, _ctx: &Context, _orig: &Message, name: String) -> crate::error::Result<String> {
        Ok(name)
    }

    async fn process(&self, _dis: &Dispatch, _ctx: &Context, _orig: &Message, _command: Vec<String>) -> crate::error::Result<()> {
        Err(UnimplementedModule.into())
    }
}


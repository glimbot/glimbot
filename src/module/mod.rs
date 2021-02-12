use crate::dispatch::{Dispatch, config};
use serenity::client::Context;
use std::collections::HashSet;
use std::fmt;
use std::fmt::Formatter;
use serenity::model::channel::Message;
use std::sync::Arc;

pub mod status;
pub mod owner;
pub mod base_filter;
pub mod shutdown;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Sensitivity {
    Low, // Anyone should be able to run at any time
    Medium, // Anyone can run, but prone to spamming
    High, // Sensitive commands related to managing users/spam,
    Owner // Only owner should be able to run
}

impl fmt::Display for Sensitivity {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let s = match self {
            Sensitivity::Low => {"low"}
            Sensitivity::Medium => {"medium"}
            Sensitivity::High => {"high"}
            Sensitivity::Owner => {"owner"}
        };
        f.write_str(s)
    }
}

pub struct ModInfo {
    pub name: &'static str,
    pub sensitivity: Sensitivity,
    pub does_filtering: bool,
    pub command: bool,
    pub config_values: Vec<Arc<dyn config::Validator>>
}

impl ModInfo {
    pub fn with_name(name: &'static str) -> Self {
        ModInfo {
            name,
            sensitivity: Sensitivity::Owner,
            does_filtering: false,
            command: false,
            config_values: Vec::new()
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

#[async_trait::async_trait]
pub trait Module: Sync + Send {
    fn info(&self) -> &ModInfo;

    async fn filter(&self, _dis: &Dispatch, _ctx: &Context, _orig: &Message, name: String) -> crate::error::Result<String> {
        Ok(name)
    }

    async fn process(&self, _dis: &Dispatch, _ctx: &Context, _orig: &Message, _command: Vec<String>) -> crate::error::Result<()> {
        unimplemented!()
    }
}

use crate::dispatch::Dispatch;
use serenity::client::Context;
use std::collections::HashSet;
use std::fmt;
use std::fmt::Formatter;
use serenity::model::channel::Message;

pub mod status;
pub mod owner;

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

#[derive(Debug)]
pub struct ModInfo {
    pub name: &'static str,
    pub sensitivity: Sensitivity,
    pub does_filtering: bool,
    pub command: bool
}

#[async_trait::async_trait]
pub trait Module: Sync + Send {
    fn info(&self) -> &ModInfo;

    async fn filter(&self, _dis: &Dispatch, _ctx: &Context, _orig: &Message, command: Vec<String>) -> crate::error::Result<Vec<String>> {
        Ok(command)
    }

    async fn process(&self, _dis: &Dispatch, _ctx: &Context, _orig: &Message, _command: Vec<String>) -> crate::error::Result<()> {
        unimplemented!()
    }
}


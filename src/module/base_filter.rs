use once_cell::sync::Lazy;
use serenity::client::Context;
use serenity::model::channel::Message;

use crate::dispatch::{config, Dispatch};
use crate::error::UserError;
use crate::module::{ModInfo, Module, Sensitivity};

pub struct BaseFilter;

pub const MAX_COMMAND_LEN: usize = 1500;

#[async_trait::async_trait]
impl Module for BaseFilter {
    fn info(&self) -> &ModInfo {
        static INFO: Lazy<ModInfo> = Lazy::new(|| {
            ModInfo::with_name("base-filter")
                .with_filter(true)
                .with_sensitivity(Sensitivity::Low)
                .with_config_value(config::Value::<char>::with_default("command_prefix", "A single character which will precede commands.", '!'))
        });
        &INFO
    }

    async fn filter(&self, _dis: &Dispatch, _ctx: &Context, orig: &Message, name: String) -> crate::error::Result<String> {
        if orig.content.len() > MAX_COMMAND_LEN {
            return Err(UserError::new(format!("Command too long: must be no longer than {} UTF-8 code points", MAX_COMMAND_LEN)).into());
        }

        if orig.author.bot {
            return Err(UserError::new("Glimbot does not accept command strings from bots.").into());
        }

        Ok(name)
    }
}
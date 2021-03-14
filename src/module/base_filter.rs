//! Contains base filtering for glimbot, as well as the `command_prefix` config value.
//! Glimbot will not work at all without this module.

use once_cell::sync::Lazy;
use serenity::client::Context;
use serenity::model::channel::Message;

use crate::dispatch::{config, Dispatch};
use crate::module::{ModInfo, Module, Sensitivity};

/// Contains filtering for maximum command length and blocking bot commands.
pub struct BaseFilter;

/// The maximum number of UTF-8 code points which may be in a command message.
pub const MAX_COMMAND_LEN: usize = 1500;

impl_err!(NoBots, "Glimbot does not accept command strings from bots.", true);
impl_err!(CommandTooLong, "Command too long: must be no longer than 1500 UTF-8 code points", true);

#[async_trait::async_trait]
impl Module for BaseFilter {
    fn info(&self) -> &ModInfo {
        #[doc(hidden)]
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
            return Err(CommandTooLong.into());
        }

        if orig.author.bot {
            return Err(NoBots.into());
        }

        Ok(name)
    }
}
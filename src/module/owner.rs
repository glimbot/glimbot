//! Contains the owner filter, which ensures that commands with Sensitivity::Owner are only
//! run by the owner of the bot.





use once_cell::sync::Lazy;
use serenity::client::Context;
use serenity::model::channel::Message;

use crate::dispatch::Dispatch;
use crate::module::{ModInfo, Module, Sensitivity};

#[doc(hidden)]
static MOD_INFO: Lazy<ModInfo> = Lazy::new(|| {
    ModInfo::with_name("owner-check", "")
        .with_sensitivity(Sensitivity::Low)
        .with_filter(true)
});

/// Ensures that commands with owner sensitivity are run only by the owner of the bot.
pub struct OwnerFilter;

impl_err!(MustBeBotOwner, "You must be the bot owner to do the specified command.", true);


#[async_trait::async_trait]
impl Module for OwnerFilter {
    fn info(&self) -> &ModInfo {
        &MOD_INFO
    }

    async fn filter(&self, dis: &Dispatch, _ctx: &Context, orig: &Message, name: String) -> crate::error::Result<String> {
        let cmd = name.as_str();
        let mod_info = dis.command_module(cmd)?;
        if mod_info.info().sensitivity == Sensitivity::Owner {
            if orig.author.id == dis.owner() {
                trace!("Command invoked by owner.");
                Ok(name)
            } else {
                Err(MustBeBotOwner.into())
            }
        } else {
            trace!("Command not owner-only.");
            Ok(name)
        }
    }
}
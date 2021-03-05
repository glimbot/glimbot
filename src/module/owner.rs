use std::error::Error;
use std::fmt;
use std::fmt::Formatter;

use once_cell::sync::Lazy;
use serenity::client::Context;
use serenity::model::channel::Message;

use crate::dispatch::Dispatch;
use crate::module::{ModInfo, Module, Sensitivity};

static MOD_INFO: Lazy<ModInfo> = Lazy::new(|| {
    ModInfo::with_name("owner-check")
        .with_sensitivity(Sensitivity::Low)
        .with_filter(true)
});

pub struct OwnerFilter;

#[derive(Debug)]
pub struct MustBeBotOwner;

impl fmt::Display for MustBeBotOwner {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "You must be the bot owner to do the specified command.")
    }
}

impl Error for MustBeBotOwner {}

impl_user_err_from!(MustBeBotOwner);


#[async_trait::async_trait]
impl Module for OwnerFilter {
    fn info(&self) -> &ModInfo {
        &MOD_INFO
    }

    async fn filter(&self, dis: &Dispatch, ctx: &Context, orig: &Message, name: String) -> crate::error::Result<String> {
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
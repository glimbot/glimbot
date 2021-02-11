use crate::module::{ModInfo, Sensitivity, Module};
use once_cell::sync::Lazy;
use serenity::client::Context;
use crate::dispatch::Dispatch;
use std::fmt;
use std::fmt::Formatter;
use std::error::Error;
use serenity::model::channel::Message;

static MOD_INFO: Lazy<ModInfo> = Lazy::new(|| {
    ModInfo {
        name: "owner-check",
        sensitivity: Sensitivity::Low,
        does_filtering: true,
        command: false
    }
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

    async fn filter(&self, dis: &Dispatch, ctx: &Context, orig: &Message, command: Vec<String>) -> crate::error::Result<Vec<String>> {
        let cmd = command.first().unwrap();
        let mod_info = dis.command_module(cmd)?;
        if mod_info.info().sensitivity == Sensitivity::Owner {
            if orig.author.id == dis.owner() {
                trace!("Command invoked by owner.");
                Ok(command)
            } else {
                Err(MustBeBotOwner.into())
            }
        } else {
            trace!("Command not owner-only.");
            Ok(command)
        }
    }
}
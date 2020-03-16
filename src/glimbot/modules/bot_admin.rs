use crate::glimbot::{GlimDispatch, EventHandler};
use crate::glimbot::modules::command::{Commander, Arg};
use serenity::model::prelude::{GuildId, Message};
use serenity::prelude::Context;
use super::command::Result;
use std::sync::atomic::Ordering;
use crate::glimbot::util::say_codeblock;
use once_cell::sync::Lazy;
use std::collections::HashSet;
use std::iter::FromIterator;
use crate::glimbot::modules::command::CommanderError::InsufficientUserPerms;
use crate::glimbot::modules::{Module, ModuleBuilder};
use serenity::model::event::EventType::MessageCreate;
use serenity::model::Permissions;
use std::process::exit;
use diesel::connection::SimpleConnection;

static ADMIN_COMMANDS: Lazy<HashSet<String>> = Lazy::new(
    || HashSet::from_iter(vec!["shutdown".to_string()])
);

/// Assumes aliases have been resolved
/// If this is one of the admin aliases, ensures
fn bot_admin_permission_hook(disp: &GlimDispatch, g: GuildId, _ctx: &Context, msg: &Message, cmd: String) -> crate::glimbot::modules::command::Result<String> {
    if ADMIN_COMMANDS.contains(&cmd) {
        let rg = disp.owners.read();
        if !rg.contains(&msg.author.id) {
            info!("In guild {}, user {} attempted to use an owner command.", g, &msg.author.name);
            Err(InsufficientUserPerms(msg.author.id))
        } else {
            Ok(cmd)
        }
    } else {
        Ok(cmd)
    }
}

fn shutdown(disp: &GlimDispatch,
            _cmd: &Commander,
            _g: GuildId,
            ctx: &Context,
            msg: &Message,
            _args: &[Arg]) -> Result<()> {
    info!("Shutting down.");
    let wr_conn = disp.wr_conn().lock();
    let res = wr_conn.batch_execute("PRAGMA wal_checkpoint(FULL);");
    if let Err(e) = res {
        error!("Couldn't run checkpointing operation: {}", e);
    } else {
        info!("Ran checkpointing operation before shutdown.")
    }
    say_codeblock(ctx, msg.channel_id, "Shutting down.");
    exit(0);
}

pub fn bot_admin_module() -> Module {
    ModuleBuilder::new("bot_admin")
        .with_hook(MessageCreate, EventHandler::CommandHandler(bot_admin_permission_hook))
        .with_command(Commander::new(
            "shutdown",
            Some("Shuts down the Glimbot instance. Bot-owner-only."),
            Vec::<String>::new(),
            vec![],
            vec![],
            Permissions::SEND_MESSAGES,
            shutdown
        ))
        .build()
}
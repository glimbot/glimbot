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
use std::any::{Any, TypeId};
use std::sync::Arc;
use lock_api::RwLockUpgradableReadGuard;
use typemap::Key;
use parking_lot::RwLock;
use nom::lib::std::collections::HashMap;
use crate::glimbot::modules::command::parser::RawCmd;

static ADMIN_COMMANDS: Lazy<HashSet<String>> = Lazy::new(
    || HashSet::from_iter(vec!["shutdown".to_string(), "cmd_stats".to_string()])
);

struct AdminKey;

impl Key for AdminKey {
    type Value = Arc<AdminState>;
}

mod stats;

struct AdminState {
    command_stats: stats::GuildStats,
    guild_stats: RwLock<HashMap<GuildId, stats::GuildStats>>
}

impl AdminState {
    pub fn new() -> AdminState {
        AdminState {
            command_stats: stats::GuildStats::new(),
            guild_stats: RwLock::new(HashMap::new())
        }
    }
}

fn ensure_transient_state(ctx: &Context) -> Arc<AdminState> {
    let rg = ctx.data.upgradable_read();
    if !rg.contains::<AdminKey>() {
        let mut wg = RwLockUpgradableReadGuard::upgrade(rg);
        let out = Arc::new(AdminState::new());
        wg.insert::<AdminKey>(out.clone());
        out
    } else {
        rg.get::<AdminKey>().unwrap().clone()
    }
}

/// Assumes aliases have been resolved
/// If this is one of the admin aliases, ensures
fn bot_admin_permission_hook(disp: &GlimDispatch, g: GuildId, _ctx: &Context, msg: &Message, cmd: RawCmd) -> crate::glimbot::modules::command::Result<RawCmd> {
    if ADMIN_COMMANDS.contains(&cmd.command) {
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

fn bot_stat_hook(disp: &GlimDispatch, g: GuildId, ctx: &Context, _msg: &Message, cmd: RawCmd) -> crate::glimbot::modules::command::Result<RawCmd> {
    if !disp.command_map.contains_key(&cmd.command) {
        return Ok(cmd)
    }

    let st = ensure_transient_state(ctx);
    let usages = st.command_stats.add_usage(&cmd.command);
    trace!("Command {} used {} times", &cmd.command, usages);

    let rg = st.guild_stats.upgradable_read();
    if !rg.contains_key(&g) {
        let mut wg = RwLockUpgradableReadGuard::upgrade(rg);
        wg.insert(g, stats::GuildStats::new());
        let s = wg.get(&g).unwrap();
        let u = s.add_usage(&cmd.command);
        trace!("Guild {}, {}: {:?}", g, &cmd.command, u);
    } else {
        let s = rg.get(&g).unwrap();
        let u = s.add_usage(&cmd.command);
        trace!("Guild {}, {}: {:?}", g, &cmd.command, u);
    }

    Ok(cmd)
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

pub fn bot_stats_module() -> Module {
    ModuleBuilder::new("bot_stats")
        .with_hook(MessageCreate, EventHandler::CommandHandler(bot_stat_hook))
        .build()
}
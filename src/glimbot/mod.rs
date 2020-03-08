use std::collections::{HashSet, HashMap, BTreeMap};
use std::path::{Path, PathBuf};
use serenity::model::event::{EventType, Event, MessageUpdateEvent};
use crate::glimbot::modules::Module;
use crate::glimbot::guilds::{GuildContext, RwGuildPtr};
use serenity::model::prelude::{Message, GuildId};
use serenity::prelude::{Context, EventHandler as EHandler};
use thiserror::Error;
use std::rc::Rc;
use std::sync::Arc;
use once_cell::sync::Lazy;
use parking_lot::RwLock;
use std::result::Result as StdResult;
use std::error::Error as StdError;
use std::ops::Deref;
use std::io::Write;
use regex::Regex;
use crate::glimbot::util::FromError;
use log::{info, debug, error, trace};
use serenity::model::gateway::Ready;
use serenity::model::id::{ChannelId, MessageId};
use parking_lot::RwLockUpgradableReadGuard;
use serenity::http::CacheHttp;
use crate::glimbot::modules::command::CommanderError;
use crate::glimbot::modules::command::parser::RawCmd;

pub mod env;
pub mod config;
pub mod modules;
pub mod guilds;
pub mod util;


pub type EventHandlerFn = fn(&Context, &RwGuildPtr, &Event) -> bool;
pub type MessageHandlerFn = fn(&Context, &RwGuildPtr, &Message) -> bool;
pub type CommandHandlerFn = fn(&Context, &RwGuildPtr, &Message, String) -> modules::command::Result<String>;

#[derive(Clone)]
pub enum EventHandler {
    GenericHandler(EventHandlerFn),
    MessageHandler(MessageHandlerFn),
    CommandHandler(CommandHandlerFn),
}

#[derive(Error, Debug)]
pub enum EventError {
    #[error("An error occurred: {0}")]
    Other(#[from] Box<dyn StdError>),
}

#[derive(Error, Debug)]
pub enum InternalError {
    #[error("An error occurred: {0:?}")]
    Other(#[from] Box<dyn StdError>),
}

impl FromError for InternalError {
    fn from_error(e: impl StdError + 'static) -> Self {
        InternalError::Other(Box::from(e))
    }
}

pub type EventResult<T> = StdResult<T, EventError>;
pub type GlimResult<T> = StdResult<T, InternalError>;

pub struct GlimDispatch {
    working_directory: PathBuf,
    modules: HashMap<String, Module>,
    hooks: BTreeMap<EventType, Vec<EventHandler>>,
    guilds: RwLock<HashMap<GuildId, RwGuildPtr>>,
    command_map: HashMap<String, String>
}

static GUILD_PATH_RE: Lazy<Regex> = Lazy::new(
    || Regex::new(r#"^(\d+)_conf.yaml"#).unwrap()
);

fn file_is_guild(p: impl AsRef<Path>) -> bool {
    p.as_ref().file_name().map_or(
        false,
        |s| GUILD_PATH_RE.is_match(s.to_string_lossy().as_ref()),
    )
}

fn load_guild_config(p: impl AsRef<Path>) -> GlimResult<GuildContext> {
    let mut f = std::fs::OpenOptions::new()
        .read(true)
        .open(p)
        .map_err(InternalError::from_error)?;

    let o: GuildContext = serde_yaml::from_reader(f).map_err(InternalError::from_error)?;
    Ok(o)
}

impl GlimDispatch {
    pub fn new() -> Self {
        GlimDispatch {
            working_directory: PathBuf::from(".".to_string()),
            modules: HashMap::new(),
            hooks: BTreeMap::new(),
            guilds: RwLock::new(HashMap::new()),
            command_map: HashMap::new()
        }
    }

    pub fn with_module(mut self, module: Module) -> Self {
        module.hooks().iter()
            .for_each(|(ev, handler)| {
                let entry = self.hooks.entry(ev.clone()).or_default();
                entry.push(handler.clone())
            });
        module.commands().keys().for_each(|x| {self.command_map.insert(x.clone(), module.name().to_string());});
        info!("Loaded module {}", module.name());
        self.modules.insert(module.name().to_string(), module);
        self
    }

    pub fn load_guilds(&mut self) -> GlimResult<()> {
        let p = self.working_directory.clone();
        let v: Vec<_> = std::fs::read_dir(p).map_err(InternalError::from_error)?
            .map(|p| {
                if let Ok(d) = &p {
                    d.path()
                } else {
                    PathBuf::from("")
                }
            })
            .filter(|p| file_is_guild(p))
            .map(|p| load_guild_config(p))
            .collect();

        v.iter().filter(|e| e.is_err())
            .map(|e| e.as_ref().unwrap_err())
            .for_each(|e| error!("Couldn't load guild from {:?}", e));

        v.into_iter().filter(|r| r.is_ok())
            .map(|r| r.unwrap())
            .for_each(
                |g| {
                    self.guilds.write().insert(g.guild, RwGuildPtr::from(g));
                }
            );

        Ok(())
    }

    pub fn write_guilds(&mut self) -> GlimResult<()> {
        for g in self.guilds.read().values() {
            let rg = g.read();
            let dest_file = format!("{}_conf.yaml", rg.guild.to_string());
            let dest_path = self.working_directory.join(dest_file);
            let mut f = std::fs::OpenOptions::new()
                .write(true)
                .truncate(true)
                .open(dest_path)
                .map_err(InternalError::from_error)?;

            serde_yaml::to_writer(&f, rg.deref())
                .map_err(InternalError::from_error)?;
            f.flush().map_err(InternalError::from_error)?;
        }

        Ok(())
    }

    // Checks to see if we've met this guild before; if not it creates a default config for it.
    // In either case, returns a guild context associated with this id
    pub fn encounter_guild(&self, g: GuildId) -> RwGuildPtr {
        let read_gs = self.guilds.upgradable_read();
        let out = if !read_gs.contains_key(&g) {
            info!("Encountered new guild {}", g);
            let mut lock = RwLockUpgradableReadGuard::upgrade(read_gs);
            let out = RwGuildPtr::from(GuildContext::new(g));
            out.deref().read().commit_to_disk();
            lock.insert(g, out.clone());
            out
        } else {
            read_gs.get(&g).unwrap().clone()
        };

        out
    }
}

impl EHandler for GlimDispatch {
    fn message(&self, ctx: Context, new_message: Message) {
        if new_message.is_own(&ctx) {
            trace!("Saw a message from myself.");
            return;
        }
        if let Some(gid) = new_message.guild_id {
            let gc = self.encounter_guild(gid);
            if let Some(v) = self.hooks.get(&EventType::MessageCreate) {
                let mut stop = false;
                for hook in v {
                    match hook {
                        EventHandler::MessageHandler(m) => {
                            stop = m(&ctx, &gc, &new_message);
                        }
                        EventHandler::CommandHandler(_) => (),
                        _ => unreachable!()
                    };

                    if stop {
                        return
                    }
                };


            }

            let mut pref = gc.read().command_prefix.clone();
            if new_message.content.starts_with(&pref) {
                // This may be a command
                let cmd: modules::command::Result<String> = if let Some(v) = self.hooks.get(&EventType::MessageCreate) {
                    v.iter()
                        .filter(|e| match e {
                            EventHandler::CommandHandler(_) => { true }
                            _ => { false }
                        })
                        .try_fold(new_message.content.clone(), |s, h| {
                            if let EventHandler::CommandHandler(c) = h {
                                c(&ctx, &gc, &new_message, s)
                            } else {
                                unreachable!()
                            }
                        })
                    } else {
                        Ok(new_message.content.clone())
                    };

                let raw_cmd = cmd.and_then(
                    |s| modules::command::parser::parse_command(s)
                );

                match raw_cmd.and_then(|r| {
                    let module = self.command_map.get(&r.command);
                    if let Some(name) = module {
                        let m = self.modules.get(name).unwrap();
                        let c = m.commands().get(&r.command).unwrap();
                        c.invoke(&gc, &ctx, &new_message, &r.args)
                    } else {
                        debug!("Got invalid command in channel {}: {}", new_message.channel_id, &r.command);
                        new_message.channel_id.say(&ctx, "No such command.")
                            .map(|x| {})
                            .map_err(|_| CommanderError::Silent)
                    }
                }) {
                    Err(CommanderError::Silent) => {},
                    Err(e) => {
                        if let Err(err) = new_message.channel_id.say(&ctx, e) {
                            debug!("Command failed: {}", err);
                        }
                    }
                    Ok(_) => {}
                }
            }
        }
    }

    fn message_delete(&self, ctx: Context, channel_id: ChannelId, deleted_message_id: MessageId) {}

    fn message_delete_bulk(&self, ctx: Context, channel_id: ChannelId, multiple_deleted_messages_ids: Vec<MessageId>) {}

    fn ready(&self, ctx: Context, data_about_bot: Ready) {
        use serenity::model::gateway::Activity;
        info!("Connected to Discord!");

        data_about_bot.guilds.iter().for_each(
            |g| {
                self.encounter_guild(g.id());
            }
        );
        ctx.set_activity(Activity::playing("Cultist Simulator"));
    }
}
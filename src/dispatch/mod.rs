//! Contains the code related to dispatching glimbot actions, reacting to messages, etc.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fmt;
use std::fmt::Formatter;
use std::sync::{Arc, Weak};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use futures::stream;
use futures::stream::StreamExt;
use futures::TryStreamExt;
use linked_hash_map::LinkedHashMap;
use once_cell::sync::{OnceCell, Lazy};
use rand::seq::SliceRandom;
use rand::thread_rng;
use serenity::client::{Context, EventHandler};
use serenity::client::bridge::gateway::ShardManager;
use serenity::model::channel::Message;
use serenity::model::gateway::{Activity, Ready};
use serenity::model::id::{GuildId, UserId};
use serenity::prelude::TypeMapKey;
use serenity::utils::MessageBuilder;
use sqlx::PgPool;
use tokio::sync::{Mutex, watch};
use tracing::Instrument;

use crate::db::{DbContext, ConfigCache};
use crate::db::timed::TimedEvents;
use crate::dispatch::config::ValueType;
use crate::error::{LogErrorExt, SysError, UserError};
use crate::module::Module;
use crate::db::cache::TimedCache;
use crate::util::ordset::OrdSet;
use crate::dispatch::message_info::MsgInfo;
use std::num::NonZeroUsize;

pub mod config;
pub mod message_info;

pub const PER_GUILD_MESSAGE_CACHE_SIZE: usize = 4096;

/// The primary dispatch state holder. Contains information on the various modules
/// and filters installed in Glimbot.
pub struct Dispatch {
    /// The bot owner.
    owner: UserId,
    /// Filters which are applied to each message.
    filters: Vec<Arc<dyn Module>>,
    /// Modules containing some combination of commands and filters.
    modules: LinkedHashMap<&'static str, Arc<dyn Module>>,
    /// Modules containing message hooks.
    message_hooks: Vec<Arc<dyn Module>>,
    /// Modules containing tick-based hooks
    tick_hooks: Vec<Arc<dyn Module>>,
    /// Config value validators for the configuration values set in each guild.
    config_values: BTreeMap<&'static str, Arc<dyn config::Validator>>,
    /// Database connection pool.
    pool: PgPool,
    /// The background service, initialized on first start.
    background_service: OnceCell<Arc<BackgroundService>>,
    config_cache: ConfigCache,
    msg_cache: TimedCache<GuildId, OrdSet<MsgInfo>>,
    bot_id_channels: (watch::Sender<Option<UserId>>, watch::Receiver<Option<UserId>>),
    bot_id_local: thread_local::ThreadLocal<Mutex<watch::Receiver<Option<UserId>>>>
}

impl Dispatch {
    pub async fn bot(&self) -> UserId {
        let mut g = self.bot_id_local
            .get_or(|| Mutex::new(self.bot_id_channels.1.clone()))
            .lock()
            .await;

        if g.borrow().is_none() {
            g.changed().await.expect("Receiver was unable to get the UserId");
        }

        let v = g.borrow().as_ref().cloned().expect("False wake up");
        v
    }
}

impl Dispatch {
    pub fn config_cache(&self) -> &ConfigCache {
        &self.config_cache
    }
}

impl Dispatch {
    /// Gets a reference to the DB pool.
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}

impl Dispatch {
    /// Retrieves a reference to the map mapping config values to
    pub fn config_values(&self) -> &BTreeMap<&'static str, Arc<dyn config::Validator>> {
        &self.config_values
    }
}

/// TypeId key for accessing the shard manager.
pub struct ShardManKey;

impl TypeMapKey for ShardManKey {
    type Value = Arc<Mutex<ShardManager>>;
}

impl Dispatch {
    /// Get the owner of this instance of Glimbot.
    pub fn owner(&self) -> UserId {
        self.owner
    }
    /// Convenience function for constructing a DbContext with the pool in this Dispatch.
    pub fn db(&self, gid: GuildId) -> DbContext {
        DbContext::new(self, gid)
    }
}

/// Error returned if a specified command doesn't exist.
#[derive(Debug)]
pub struct NoSuchCommand {
    #[doc(hidden)]
    cmd: Cow<'static, str>
}

impl NoSuchCommand {
    /// Creates a new NoSuchCommand.
    pub fn new(cmd: impl Into<Cow<'static, str>>) -> Self {
        NoSuchCommand { cmd: cmd.into() }
    }
}

impl fmt::Display for NoSuchCommand {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "No such command: {}", &self.cmd)
    }
}

impl std::error::Error for NoSuchCommand {}
impl_user_err_from!(NoSuchCommand);
impl_err!(NoDMs, "Glimbot is not designed to respond to DMs.", true);
impl_err!(ExpectedString, "Expected at least once string to appear in the command.", false);


impl Dispatch {
    /// Creates an empty dispatch with the given pool and owner.
    pub fn new(owner: UserId, pool: PgPool) -> Self {
        Self {
            owner,
            filters: Vec::new(),
            modules: Default::default(),
            message_hooks: vec![],
            tick_hooks: vec![],
            config_values: Default::default(),
            background_service: Default::default(),
            pool,
            config_cache: ConfigCache::default(),
            msg_cache: TimedCache::new(chrono::Duration::days(7).to_std().unwrap()),
            bot_id_channels: watch::channel(None),
            bot_id_local: Default::default()
        }
    }

    /// Adds a module to this dispatch instance.
    #[instrument(level = "info", skip(self, module), fields(m = % module.info().name))]
    pub fn add_module<T: Module + 'static>(&mut self, module: T) {
        let a = Arc::new(module);
        let inf = a.info();

        info!("with sensitivity: {}", inf.sensitivity);

        if inf.does_filtering {
            info!("does filtering");
            self.filters.push(a.clone());
        }

        if inf.on_message {
            info!("has on message hook");
            self.message_hooks.push(a.clone());
        }

        if inf.on_tick {
            info!("has on tick hook");
            self.tick_hooks.push(a.clone());
        }

        for v in &inf.config_values {
            info!("adds config value {}", v.name());
            self.config_values.insert(v.name(), v.clone());
            self.config_cache.add_key(v.name());
        }

        self.modules.insert(inf.name, a);
    }

    /// Retrieves a module by name.
    pub fn module(&self, name: &str) -> Option<&dyn Module> {
        self.modules.get(name).map(|r| r.as_ref())
    }

    /// Retrieves a module, returning an error if the specified module isn't a command module.
    pub fn command_module(&self, cmd: &str) -> Result<&dyn Module, NoSuchCommand> {
        self.module(cmd)
            .filter(|s| s.info().command)
            .ok_or_else(|| NoSuchCommand::new(cmd.to_string()))
    }

    /// Retrieves a validator reference by name.
    pub fn config_value(&self, name: &str) -> crate::error::Result<&dyn config::Validator> {
        self.config_values.get(name).map(|o| o.as_ref())
            .ok_or_else(|| #[allow(deprecated)] UserError::new(format!("No such config value: {}", name)).into())
    }

    /// Retrieves a validator reference by name, downcasting it to a specified type.
    pub fn config_value_t<T: ValueType>(&self, name: &str) -> crate::error::Result<&config::Value<T>>
        where T::Err: std::error::Error + Send + Sized + 'static {
        let v = self.config_value(name)?;
        let out = v.as_any().downcast_ref()
            .ok_or_else(|| #[allow(deprecated)] SysError::new(format!("Incorrect type downcast for config value {}", name)))?;
        Ok(out)
    }

    /// The primary entry point for glimbot message handling. Messages that start with a command prefix are interpreted
    /// as commands and have filters and such applied to them.
    pub async fn handle_message(&self, ctx: &Context, new_message: &Message) -> crate::error::Result<()> {
        let contents = &new_message.content;
        // This allows us to assume we're in a guild everywhere down the line.
        let guild = if let Some(id) = new_message.guild_id {
            id
        } else {
            return Err(NoDMs.into());
        };
        tracing::Span::current().record("g", &guild.0);
        if new_message.author.id == ctx.cache.current_user_id().await {
            trace!("Saw message from self. Ignoring.");
            return Ok(());
        }

        self.msg_cache.get_or_insert_sync(&guild, || {
            OrdSet::new(NonZeroUsize::new(PER_GUILD_MESSAGE_CACHE_SIZE))
        })
            .insert(new_message.into());

        stream::iter(self.message_hooks.iter())
            .map(Ok)
            .try_for_each(|m| m.on_message(self, ctx, new_message).instrument(debug_span!("applying msg hook", h=%m.info().name)))
            .await?;

        let first_bit = if let Some(c) = contents.chars().next() {
            c
        } else {
            trace!("Saw empty message or embed.");
            return Ok(());
        };


        let db = DbContext::new(self, guild);

        let command_char = self.config_value_t::<char>("command_prefix")?
            .get_or_default(&db)
            .await?;

        if first_bit != *command_char {
            trace!("Ignoring non-command message");
            return Ok(());
        }

        let cmd_raw = &contents[first_bit.len_utf8()..];
        let cmd_name = cmd_raw.split_whitespace()
            .next()
            .ok_or(ExpectedString)?;


        let cmd = stream::iter(self.filters.iter())
            .map(Result::Ok)
            .try_fold(cmd_name.to_string(), |acc, f: &Arc<dyn Module>| {
                f.filter(self, ctx, new_message, acc)
                    .instrument(debug_span!("applying filter", f=%f.info().name))
            }).await?;

        let mut command = if let Some(c) = shlex::split(cmd_raw) {
            c
        } else {
            #[allow(deprecated)]
                return Err(UserError::new(format!("Invalid command string: {}", &contents)).into());
        };
        command[0] = cmd;
        let name = cmd_name;
        let cmd_mod = self.command_module(name)?;
        cmd_mod.process(self, ctx, &new_message, command)
            .instrument(info_span!("running command", c=%cmd_mod.info().name))
            .await?;

        Ok(())
    }
}

#[async_trait::async_trait]
impl EventHandler for Dispatch {
    #[instrument(level = "info", skip(self, ctx, new_message), fields(g, u = % new_message.author.id, m = % new_message.id))]
    async fn message(&self, ctx: Context, new_message: Message) {
        let start = Instant::now();
        let res = self.handle_message(&ctx, &new_message).await;

        res.log_error();
        if let Err(e) = res {
            let mb = if e.is_user_error() {
                MessageBuilder::new()
                    .push_codeblock_safe(format!("{}", e), None)
                    .build()
            } else {
                MessageBuilder::new()
                    .push_codeblock_safe("An internal error occurred. If this continues, please contact the bot owner.", None)
                    .build()
            };

            if let Err(e) = new_message.reply(&ctx, mb).await {
                error!("Failed while sending error message: {}", e);
            }
        }

        let elapsed = start.elapsed();
        debug!("Processing took {:?}", elapsed);
    }

    async fn ready(&self, ctx: Context, rdy: Ready) {
        self.bot_id_channels.0.send(Some(rdy.user.id)).expect("All receivers dropped?");
        info!("up and running in {} guilds.", rdy.guilds.len());
        ctx.set_activity(Activity::playing("Cultist Simulator")).await;
    }
}

/// Thin wrapper around Dispatch to allow sharing it with the background service.
#[derive(Shrinkwrap, Clone)]
pub struct ArcDispatch(Arc<Dispatch>);

impl From<Dispatch> for ArcDispatch {
    fn from(d: Dispatch) -> Self {
        ArcDispatch(Arc::new(d))
    }
}

/// Represents the background service. It's self cancelling; when Dispatch is dropped,
/// this service will stop itself after the next tick.
struct BackgroundService {
    /// Reference to the original dispatch. We use weak to avoid a reference cycle.
    /// Also makes the background service self cancelling.
    dispatch: Weak<Dispatch>,
    /// Context for interacting with Discord.
    ctx: Context,
    /// Set on first start.
    started: AtomicBool,
}

impl BackgroundService {
    /// Starts the background service if it hasn't already started.
    pub async fn start(&self) {
        // fetch_or returns the previously stored value; if it's false, we
        // weren't the first to try starting the service.
        if self.started.fetch_or(true, Ordering::AcqRel) {
            return;
        }

        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(15));
        interval.tick().await; // Avoid waiting while we're holding the pointer to Dispatch.

        while let Some(d) = self.dispatch.upgrade() {
            self.process_events(&d).await.log_error();
            std::mem::drop(d); // Manually drop to avoid holding while we wait.
            interval.tick().await;
        }
    }

    /// Processes timed events from the database.
    #[instrument(level = "info", skip(self, dis))]
    pub async fn process_events(&self, dis: &Dispatch) -> crate::error::Result<()> {
        let mut batch = TimedEvents::get_actions_before(dis.pool(),
                                                        chrono::DateTime::from(chrono::Local::now()),
        ).await?;

        // Avoid a long sequence of the same guild from bulk actions
        batch.shuffle(&mut thread_rng());

        if !batch.is_empty() {
            debug!("got {} events", batch.len());
        }

        for a in batch {
            let r = a.act(dis, &self.ctx).await;
            r.log_error();
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl EventHandler for ArcDispatch {
    #[instrument(level = "info", skip(self, ctx, _guilds), fields(shard = % ctx.shard_id))]
    async fn cache_ready(&self, ctx: Context, _guilds: Vec<GuildId>) {
        let service = self.0.background_service.get_or_init(|| {
            BackgroundService {
                dispatch: Arc::downgrade(self.as_ref()),
                ctx,
                started: Default::default(),
            }.into()
        });

        let s = service.clone();
        tokio::task::spawn(async move {
            s.start().await
        });
    }

    async fn message(&self, ctx: Context, new_message: Message) {
        self.0.message(ctx, new_message).await
    }

    async fn ready(&self, ctx: Context, rdy: Ready) {
        self.0.ready(ctx, rdy).await
    }
}
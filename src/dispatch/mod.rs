use std::borrow::Cow;
use std::fmt;
use std::fmt::Formatter;
use std::sync::Arc;
use std::time::Instant;

use futures::stream;
use futures::stream::StreamExt;
use futures::TryStreamExt;
use linked_hash_map::LinkedHashMap;
use serenity::client::{Context, EventHandler};
use serenity::client::bridge::gateway::ShardManager;
use serenity::model::channel::Message;
use serenity::model::gateway::{Activity, Ready};
use serenity::model::id::{GuildId, UserId};
use serenity::prelude::TypeMapKey;
use serenity::utils::MessageBuilder;
use sqlx::PgPool;
use tokio::sync::Mutex;
use tracing::Instrument;

use crate::db::DbContext;
use crate::dispatch::config::ValueType;
use crate::error::{LogErrorExt, SysError, UserError};
use crate::module::Module;

pub mod config;

pub struct Dispatch {
    owner: UserId,
    filters: Vec<Arc<dyn Module>>,
    modules: LinkedHashMap<&'static str, Arc<dyn Module>>,
    config_values: LinkedHashMap<&'static str, Arc<dyn config::Validator>>,
    pool: PgPool
}

impl Dispatch {
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}

impl Dispatch {
    pub fn config_values(&self) -> &LinkedHashMap<&'static str, Arc<dyn config::Validator>> {
        &self.config_values
    }
}

pub struct ShardManKey;

impl TypeMapKey for ShardManKey {
    type Value = Arc<Mutex<ShardManager>>;
}

impl Dispatch {
    pub fn owner(&self) -> UserId {
        self.owner
    }
    pub fn db(&self, gid: GuildId) -> DbContext {
        DbContext::new(self.pool(), gid)
    }
}

#[derive(Debug)]
pub struct NoSuchCommand {
    cmd: Cow<'static, str>
}

impl NoSuchCommand {
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

impl Dispatch {
    pub fn new(owner: UserId, pool: PgPool) -> Self {
        Self {
            owner,
            filters: Vec::new(),
            modules: Default::default(),
            config_values: Default::default(),
            pool
        }
    }

    pub fn add_module<T: Module + 'static>(&mut self, module: T) {
        let a = Arc::new(module);
        let inf = a.info();

        info!("Adding module {} with sensitivity {}, {} command, {} filtering",
              &inf.name,
              inf.sensitivity,
              if inf.command { "is a" } else { "is not a" },
              if inf.does_filtering { "with" } else { "without" }
        );

        if inf.does_filtering {
            self.filters.push(a.clone());
        }

        for v in &inf.config_values {
            info!("Module {} adds config value {}", &inf.name, v.name());
            self.config_values.insert(v.name(), v.clone());
        }

        self.modules.insert(inf.name, a);
    }

    pub fn module(&self, name: &str) -> Option<&dyn Module> {
        self.modules.get(name).map(|r| r.as_ref())
    }

    pub fn command_module(&self, cmd: &str) -> Result<&dyn Module, NoSuchCommand> {
        self.module(cmd)
            .filter(|s| s.info().command)
            .ok_or_else(|| NoSuchCommand::new(cmd.to_string()))
    }

    pub fn config_value(&self, name: &str) -> crate::error::Result<&dyn config::Validator> {
        self.config_values.get(name).map(|o| o.as_ref())
            .ok_or_else(|| UserError::new(format!("No such config value: {}", name)).into())
    }

    pub fn config_value_t<T: ValueType>(&self, name: &str) -> crate::error::Result<&config::Value<T>>
        where T::Err: std::error::Error + Send + Sized + 'static {
        let v = self.config_value(name)?;
        let out = v.as_any().downcast_ref()
            .ok_or_else(|| SysError::new(format!("Incorrect type downcast for config value {}", name)))?;
        Ok(out)
    }

    pub async fn handle_message(&self, ctx: &Context, new_message: &Message) -> crate::error::Result<()> {
        let contents = &new_message.content;
        let guild = if let Some(id) = new_message.guild_id {
            id
        } else {
            return Err(UserError::new("Glimbot is not designed to respond to DMs.").into());
        };
        tracing::Span::current().record("g", &guild.0);
        if new_message.author.id == ctx.cache.current_user_id().await {
            trace!("Saw message from self. Ignoring.");
            return Ok(());
        }
        let first_bit = if let Some(c) = contents.chars().next() {
            c
        } else {
            trace!("Saw empty message or embed.");
            return Ok(());
        };


        let db = DbContext::new(self.pool(), guild);

        let command_char = self.config_value_t::<char>("command_prefix")?
            .get_or_default(&db)
            .await?;

        if first_bit != command_char {
            trace!("Ignoring non-command message");
            return Ok(());
        }

        let cmd_raw = &contents[first_bit.len_utf8()..];
        let cmd_name = cmd_raw.split_whitespace()
            .next()
            .ok_or_else(|| SysError::new("Expected at least one string to appear in the command."))?;


        let cmd = stream::iter(self.filters.iter())
            .map(Result::Ok)
            .try_fold(cmd_name.to_string(), |acc, f: &Arc<dyn Module>| {
                f.filter(self, ctx, new_message, acc)
                    .instrument(debug_span!("applying filter", f=%f.info().name))
            }).await?;

        let mut command = if let Some(c) = shlex::split(cmd_raw) {
            c
        } else {
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
        debug!("Processing took {} ms", elapsed.as_millis());
    }

    async fn ready(&self, ctx: Context, rdy: Ready) {
        info!("up and running in {} guilds.", rdy.guilds.len());
        ctx.set_activity(Activity::playing("Cultist Simulator")).await;
    }
}


#[derive(Shrinkwrap, Clone)]
pub struct ArcDispatch(Arc<Dispatch>);

impl From<Dispatch> for ArcDispatch {
    fn from(d: Dispatch) -> Self {
        ArcDispatch(Arc::new(d))
    }
}

struct BackgroundService {
    dispatch: ArcDispatch,
    ctx: Context,
}

#[async_trait::async_trait]
impl EventHandler for ArcDispatch {
    #[instrument(level="info", skip(self, _ctx, _guilds), fields(shard=%_ctx.shard_id))]
    async fn cache_ready(&self, _ctx: Context, _guilds: Vec<GuildId>) {
        info!("Starting background work service.");

    }

    async fn message(&self, ctx: Context, new_message: Message) {
        self.0.message(ctx, new_message).await
    }

    async fn ready(&self, ctx: Context, rdy: Ready) {
        self.0.ready(ctx, rdy).await
    }
}
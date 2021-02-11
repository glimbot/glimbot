use serenity::model::id::UserId;
use once_cell::sync::OnceCell;
use tokio::sync::RwLock;
use serenity::client::{Context, EventHandler};
use serenity::model::gateway::{Ready, Activity};
use serenity::model::channel::Message;
use std::sync::Arc;
use crate::module::Module;
use linked_hash_map::LinkedHashMap;
use std::fmt;
use std::fmt::Formatter;
use std::borrow::Cow;
use crate::db::DbContext;
use crate::error::{LogErrorExt, UserError};
use serenity::utils::MessageBuilder;
use tracing::Instrument;
use futures::TryStreamExt;
use futures::stream;
use futures::stream::StreamExt;

pub struct Dispatch {
    owner: UserId,
    filters: Vec<Arc<dyn Module>>,
    modules: LinkedHashMap<&'static str, Arc<dyn Module>>,
}

impl Dispatch {
    pub fn owner(&self) -> UserId {
        self.owner
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
    pub fn new(owner: UserId) -> Self {
        Self {
            owner,
            filters: Vec::new(),
            modules: Default::default(),
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

    pub async fn handle_message(&self, ctx: &Context, new_message: &Message) -> crate::error::Result<()> {
        let contents = &new_message.content;
        let guild = if let Some(id) = new_message.guild_id {
            id
        } else {
            return Err(UserError::new("Glimbot is not designed to respond to DMs.").into());
        };
        tracing::Span::current().record("g", &guild.0);
        let first_bit = if let Some(c) = contents.chars().next() {
            c
        } else {
            debug!("Somehow saw empty message.");
            return Ok(());
        };


        let db = DbContext::new(guild)
            .await?;

        let command_char = db.get_or_insert("command_prefix", '!')
            .await?;

        if first_bit != command_char {
            trace!("Ignoring non-command message");
            return Ok(());
        }

        let cmd_raw = &contents[first_bit.len_utf8()..];
        let command = if let Some(c) = shlex::split(cmd_raw) {
            c
        } else {
            return Err(UserError::new(format!("Invalid command string: {}", &contents)).into());
        };

        let cmd = stream::iter(self.filters.iter())
            .map(Result::Ok)
            .try_fold(command, |acc, f: &Arc<dyn Module>| {
                f.filter(self, ctx, new_message, acc)
                    .instrument(debug_span!("applying filter", f=%f.info().name))
            }).await?;

        let name = &cmd[0];
        let cmd_mod = self.command_module(name)?;
        cmd_mod.process(self, ctx, &new_message, cmd)
            .instrument(info_span!("running command", c=%cmd_mod.info().name))
            .await?;

        Ok(())
    }
}

#[async_trait::async_trait]
impl EventHandler for Dispatch {
    #[instrument(level = "info", skip(self, ctx, new_message), fields(g, u = % new_message.author.id, m = % new_message.id))]
    async fn message(&self, ctx: Context, new_message: Message) {
        let res = self.handle_message(&ctx, &new_message).await;

        res.log_error();
        if let Err(e) = res {
            if e.is_user_error() {
                let mb = MessageBuilder::new()
                    .push_codeblock_safe(format!("{}", e), None)
                    .build();

                if let Err(e) = new_message.reply(&ctx, mb).await {
                    error!("Failed while sending error message: {}", e);
                }
            }
        }
    }

    async fn ready(&self, ctx: Context, rdy: Ready) {
        info!("up and running in {} guilds.", rdy.guilds.len());
        ctx.set_activity(Activity::playing("Cultist Simulator")).await;
    }
}

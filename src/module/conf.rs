use crate::module::{Module, ModInfo, Sensitivity};
use serenity::client::Context;
use serenity::model::channel::Message;
use crate::dispatch::Dispatch;
use structopt::StructOpt;
use once_cell::sync::Lazy;
use crate::error::IntoBotErr;
use crate::util::ClapExt;
use itertools::{Either, Itertools};
use crate::db::DbContext;
use std::sync::Arc;
use serenity::utils::{MessageBuilder, content_safe, ContentSafeOptions};

pub struct ConfigModule;

/// Command to set bot config values for this guild.
#[derive(Debug, StructOpt)]
#[structopt(name = "config", no_version)]
enum ConfigOpt {
    /// Sets a bot config value
    Set {
        /// The name of the config value to set
        key: String,
        /// The value to set it to
        value: String,
    },
    /// Shows a bot config value
    Show {
        /// The name of the config value to show
        key: String,
    },
    /// Lists the available config values to be set.
    List,
    /// Shows info for config key
    Info {
        /// The name of the config value to show
        key: String,
    },
}

#[async_trait::async_trait]
impl Module for ConfigModule {
    fn info(&self) -> &ModInfo {
        static INFO: Lazy<ModInfo> = Lazy::new(|| {
            ModInfo::with_name("config")
                .with_command(true)
                .with_sensitivity(Sensitivity::High)
        });
        &INFO
    }

    async fn process(&self, dis: &Dispatch, ctx: &Context, orig: &Message, command: Vec<String>) -> crate::error::Result<()> {
        let opts = ConfigOpt::from_iter_with_help(command)?;
        let gid = orig.guild_id.unwrap();
        let message = match opts {
            ConfigOpt::Set { key, value } => {
                let config_val = dis.config_value(&key)?;
                let new_val = config_val
                    .validate(ctx, orig.guild_id.unwrap(), &value)
                    .await?;
                let ctx = DbContext::new(dis.pool(), gid);
                ctx.insert(&key, new_val).await?;
                format!("Set {} to specified value.", &key)
            }
            ConfigOpt::Show { key } => {
                let config_val = dis.config_value(&key)?;
                let db = DbContext::new(dis.pool(), gid);
                let val: Option<serde_json::Value> = db.get(key).await?;

                match val {
                    None => { "<unset>".to_string() }
                    Some(v) => {
                        config_val.display_value(v)?
                    }
                }
            }
            ConfigOpt::List => {
                dis.config_values().keys().join(", ")
            }
            ConfigOpt::Info { key } => {
                let config_val = dis.config_value(&key)?;
                format!("{}: {}", key, config_val.help())
            }
        };

        let message = content_safe(ctx,
                                   message,
                                   &ContentSafeOptions::default()
                                       .display_as_member_from(gid)).await;
        let message = MessageBuilder::new()
            .push_codeblock_safe(message, None)
            .build();
        orig.reply(ctx, message).await?;
        Ok(())
    }
}
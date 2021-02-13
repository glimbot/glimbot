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
use sled::IVec;
use serenity::utils::MessageBuilder;

pub struct ConfigModule;

/// Command to set bot config values for this guild.
#[derive(Debug, StructOpt)]
#[structopt(name = "config", no_version, setting = clap::AppSettings::ColorNever)]
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
        let message = match opts {
            ConfigOpt::Set { key, value } => {
                let config_val = dis.config_value(&key)?;
                let new_val = config_val
                    .validate(ctx, orig.guild_id.unwrap(), &value)
                    .await?;
                let ctx = DbContext::new(orig.guild_id.unwrap())
                    .await?;
                let akey = Arc::new(key);
                let bkey = akey.clone();
                ctx.do_async(move |c| {
                    c.tree().insert(bkey.as_str(), new_val)?;
                    c.tree().flush()?;
                    Ok::<_, crate::error::Error>(())
                }).await?;
                format!("Set {} to specified value.", akey)
            }
            ConfigOpt::Show { key } => {
                let config_val = dis.config_value(&key)?;
                let db = DbContext::new(orig.guild_id.unwrap())
                    .await?;
                let akey = Arc::new(key);
                let bkey = akey.clone();
                let val = db.do_async(move |c| {
                    c.tree().get(bkey.as_str())
                }).await?;

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

        let message = MessageBuilder::new()
            .push_codeblock_safe(message, None)
            .build();

        orig.reply(ctx, message).await?;
        Ok(())
    }
}
use crate::module::{Module, ModInfo, Sensitivity};
use serenity::model::prelude::RoleId;
use crate::dispatch::config::FromStrWithCtx;
use serenity::client::Context;
use serenity::model::id::GuildId;
use once_cell::sync::Lazy;
use serenity::model::channel::Message;
use crate::dispatch::Dispatch;
use structopt::StructOpt;
use crate::util::ClapExt;
use crate::db::DbContext;
use std::collections::HashSet;
use serde::Serialize;
use crate::module::privilege::VerifiedRole;
use itertools::Itertools;
use futures::StreamExt;
use crate::error::{UserError, SysError};
use serenity::utils::MessageBuilder;

pub struct RoleModule;

pub const JOINABLE_ROLES_KEY: &'static str = "joinable_roles";

/// Command to join joinable roles. Use list-joinable to join a role.
#[derive(StructOpt)]
#[structopt(name = "role", no_version)]
pub enum RoleOpt {
    /// Joins a joinable role.
    Join {
        role: String
    },
    /// Leaves a joinable role.
    Leave {
        role: String
    },
    /// Lists all joinable roles.
    ListJoinable
}

type JoinableRoles = HashSet<VerifiedRole>;

#[async_trait::async_trait]
impl Module for RoleModule {
    fn info(&self) -> &ModInfo {
        static INFO: Lazy<ModInfo> = Lazy::new(|| ModInfo::with_name("role")
            .with_sensitivity(Sensitivity::Low)
            .with_filter(false)
            .with_command(true));
        &INFO
    }

    async fn process(&self, dis: &Dispatch, ctx: &Context, orig: &Message, command: Vec<String>) -> crate::error::Result<()> {
        let role_opts = RoleOpt::from_iter_with_help(command)?;
        let gid = orig.guild_id.unwrap();
        let db = DbContext::new(gid).await?;

        let message = match &role_opts {
            RoleOpt::Join { .. } |
            RoleOpt::Leave { .. } => {
                let role = match &role_opts {
                    RoleOpt::Join { role } => role,
                    RoleOpt::Leave { role } => role,
                    _ => unreachable!()
                };

                let vrole = VerifiedRole::from_str_with_ctx(role, ctx, gid)
                    .await?;

                let roles = db
                    .get_or_insert(JOINABLE_ROLES_KEY, JoinableRoles::new())
                    .await?;

                if !roles.contains(&vrole) {
                    return Err(UserError::new(format!("{} is not user-joinable.", vrole.to_role_name_or_id(ctx, gid).await)).into());
                }

                let guild = gid.to_guild_cached(ctx)
                    .await
                    .ok_or_else(|| SysError::new("No such guild."))?;
                let mut mem = guild.member(ctx, orig.author.id)
                    .await?;

                match &role_opts {
                    RoleOpt::Join {..} => {
                        mem.add_role(ctx, vrole.into_inner()).await?;
                        "Added role.".to_string()
                    },
                    _ => {
                        mem.remove_role(ctx, vrole.into_inner()).await?;
                        "Removed role.".to_string()
                    }
                }
            },
            RoleOpt::ListJoinable => {
                let roles = db
                    .get_or_insert(JOINABLE_ROLES_KEY, JoinableRoles::new())
                    .await?;

                if roles.is_empty() {
                    "No joinable roles.".to_string()
                } else {
                    let joinable_roles = tokio_stream::iter(roles)
                        .then(|r| async move {
                            r.to_role_name(ctx, gid)
                                .await
                                .unwrap_or_else(|_| r.to_string())
                        })
                        .collect::<Vec<_>>()
                        .await
                        .join(", ");
                    joinable_roles
                }
            }
        };

        let msg = MessageBuilder::new()
            .push_codeblock_safe(message, None)
            .build();
        orig.reply(ctx, msg).await?;
        Ok(())
    }
}
use crate::module::{Module, ModInfo, Sensitivity};
use serenity::model::prelude::RoleId;
use crate::dispatch::config::{FromStrWithCtx, RoleExt, VerifiedUser, NoSuchUser, Value};
use serenity::client::Context;
use serenity::model::id::GuildId;
use once_cell::sync::Lazy;
use serenity::model::channel::Message;
use crate::dispatch::Dispatch;
use structopt::StructOpt;
use crate::util::ClapExt;
use crate::db::{DbContext, NamespacedDbContext};
use std::collections::HashSet;
use serde::Serialize;
use crate::dispatch::config::VerifiedRole;
use itertools::Itertools;
use futures::StreamExt;
use crate::error::{UserError, SysError, GuildNotInCache, RoleNotInCache, InsufficientPermissions};
use serenity::utils::MessageBuilder;
use sled::IVec;
use std::str::FromStr;
use std::fmt;
use crate::module::privilege::{PRIV_NAME, ensure_authorized_for_role};

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
    ListJoinable,
}

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
        let db = NamespacedDbContext::new(gid, JOINABLE_ROLES_KEY)
            .await?;

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

                let full_role = vrole.into_inner().to_role_cached(ctx)
                    .await
                    .ok_or(RoleNotInCache)?;

                let auth_mem = orig.member(ctx)
                    .await?;

                ensure_authorized_for_role(ctx, &auth_mem, &full_role).await?;

                let is_joinable = db.contains_key(vrole).await?;

                if !is_joinable {
                    return Err(UserError::new(format!("{} is not user-addable/removable.", vrole.to_role_name_or_id(ctx, gid).await)).into());
                }

                let guild = gid.to_guild_cached(ctx)
                    .await
                    .ok_or_else(|| SysError::new("No such guild."))?;
                let mut mem = guild.member(ctx, orig.author.id)
                    .await?;

                match &role_opts {
                    RoleOpt::Join { .. } => {
                        mem.add_role(ctx, vrole.into_inner()).await?;
                        "Added role.".to_string()
                    }
                    _ => {
                        mem.remove_role(ctx, vrole.into_inner()).await?;
                        "Removed role.".to_string()
                    }
                }
            }
            RoleOpt::ListJoinable => {
                let roles: Result<Vec<RoleId>, crate::error::Error> = db.do_async(|c| {
                    c.tree().iter()
                        .keys()
                        .map_ok(|v| {
                            let mut bytes = [0u8; 8];
                            bytes.copy_from_slice(v.as_ref());
                            RoleId::from(u64::from_be_bytes(bytes))
                        })
                        .try_collect()
                        .map_err(Into::into)
                }).await;
                let roles: Vec<_> = futures::stream::iter(roles?
                    .into_iter())
                    .then(|r| async move { r.to_role_name_or_id(ctx, gid).await })
                    .collect()
                    .await;

                if roles.is_empty() {
                    "No joinable roles.".to_string()
                } else {
                    roles.join(", ")
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

pub struct ModRoleModule;

#[derive(Debug, StructOpt)]
#[structopt(no_version)]
enum UserAction {
    /// Adds a role to a user
    Assign,
    /// Removes a role from a user
    Unassign,
}

#[derive(Debug, StructOpt)]
#[structopt(no_version)]
/// What to do with a role.
enum Action {
    /// Makes a role joinable
    AddJoinable,
    /// Removes a role from the joinable list.
    DelJoinable,
    /// Assign or unassign a role to a user.
    User {
        /// The user to assign/unassign a role to.
        user: String,
        #[structopt(subcommand)]
        /// Assign or unassign.
        action: UserAction,
    },
}

#[derive(StructOpt)]
#[structopt(name = "mod-role", no_version)]
/// Command to manage roles that users can join on their own.
struct ModRoleOpt {
    #[structopt(subcommand)]
    /// What to do with a role.
    action: Action,
    /// The role on which an action will be performed.
    role: String,
}

impl ModRoleOpt {
    pub fn extract_role(&self) -> &str {
        self.role.as_str()
    }
}

#[async_trait::async_trait]
impl Module for ModRoleModule {
    fn info(&self) -> &ModInfo {
        static INFO: Lazy<ModInfo> = Lazy::new(|| {
            ModInfo::with_name("mod-role")
                .with_command(true)
                .with_sensitivity(Sensitivity::High)
        });
        &INFO
    }

    async fn process(&self, dis: &Dispatch, ctx: &Context, orig: &Message, command: Vec<String>) -> crate::error::Result<()> {
        let opts = ModRoleOpt::from_iter_with_help(command)?;
        let gid = orig.guild_id.unwrap();
        let role = VerifiedRole::from_str_with_ctx(opts.extract_role(), ctx, gid)
            .await?;

        let full_role = role.into_inner().to_role_cached(ctx)
            .await
            .ok_or(RoleNotInCache)?;

        let auth_mem = orig.member(ctx)
            .await?;

        ensure_authorized_for_role(ctx, &auth_mem, &full_role).await?;

        let db = NamespacedDbContext::new(gid, JOINABLE_ROLES_KEY)
            .await?;

        let message = match opts.action {
            Action::AddJoinable => {
                db.insert(role, ()).await?;
                "Set role to joinable."
            }
            Action::DelJoinable => {
                let prev_val: Option<()> = db.remove(role).await?;
                match prev_val {
                    None => { "Role was already not joinable." }
                    Some(_) => { "Role is no longer joinable." }
                }
            }
            Action::User { user, action } => {
                let user = VerifiedUser::from_str_with_ctx(&user, ctx, gid)
                    .await?;
                let mut member = gid.to_guild_cached(ctx)
                    .await
                    .ok_or(GuildNotInCache)?
                    .member(ctx, user.into_inner())
                    .await
                    .map_err(|_| NoSuchUser)?;

                match action {
                    UserAction::Assign => {
                        member.add_role(ctx, role.into_inner())
                            .await?;
                        "Added role to user."
                    }
                    UserAction::Unassign => {
                        member.remove_role(ctx, role.into_inner())
                            .await?;
                        "Removed role from user if they had it."
                    }
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
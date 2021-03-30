//! Contains logic related to joining/assigning/leaving/unassigning roles.

use std::borrow::Borrow;

use futures::StreamExt;
use once_cell::sync::Lazy;
use serenity::client::Context;
use serenity::model::channel::Message;
use serenity::model::prelude::RoleId;
use serenity::utils::MessageBuilder;
use shrinkwraprs::Shrinkwrap;
use structopt::StructOpt;

use crate::db::DbContext;
use crate::dispatch::config::{FromStrWithCtx, NoSuchUser, RoleExt, VerifiedUser};
use crate::dispatch::config::VerifiedRole;
use crate::dispatch::Dispatch;
use crate::error::{DatabaseError, GuildNotInCache, RoleNotInCache};
use crate::module::{ModInfo, Module, Sensitivity};
use crate::module::privilege::ensure_authorized_for_role;
use crate::util::{ClapExt, CoalesceResultExt};

/// Adds `role` and `mod_role` command.
pub struct RoleModule;

/// Command to join joinable roles. Use list-joinable to join a role.
#[derive(StructOpt)]
#[structopt(name = "role", no_version)]
pub enum RoleOpt {
    /// Joins a joinable role.
    Join {
        /// The role to join.
        role: String
    },
    /// Leaves a joinable role.
    Leave {
        /// The role to leave.
        role: String
    },
    /// Lists all joinable roles.
    ListJoinable,
}

/// Wrapper around DbContext to retrieve/set joinable roles.
#[derive(Shrinkwrap)]
pub struct JoinableRoles<'pool> {
    #[doc(hidden)]
    ctx: DbContext<'pool>
}

impl_err!(TooManyRoles, "Can't add more roles to joinable; you have too many!", true);
impl_err!(AlreadyJoinable, "This role is already joinable.", true);

impl<'pool> JoinableRoles<'pool> {
    /// Creates a wrapper around the database context.
    pub fn new(ctx: impl Borrow<DbContext<'pool>>) -> Self {
        JoinableRoles { ctx: ctx.borrow().clone() }
    }

    /// Inserts a new joinable role into the database.
    /// This will error if the guild has too many roles or if the role is already joinable.
    pub async fn add_joinable_role(&self, role: VerifiedRole) -> crate::error::Result<()> {
        let res: Result<_, sqlx::Error> = sqlx::query!(
            "INSERT INTO joinable_roles (guild, role) VALUES ($1, $2);",
            self.ctx.guild_as_i64(),
            role.to_i64()
        )
            .execute(self.ctx.conn())
            .await;

        if let Err(e) = res {
            if e.is_check() {
                Err(TooManyRoles.into())
            } else if e.is_unique() {
                Err(AlreadyJoinable.into())
            } else {
                Err(e.into())
            }
        } else {
            Ok(())
        }
    }

    /// Removes a role from the joinable list.
    pub async fn del_joinable_role(&self, role: VerifiedRole) -> crate::error::Result<()> {
        sqlx::query!(
            "DELETE FROM joinable_roles WHERE guild = $1 AND role = $2;",
            self.ctx.guild_as_i64(),
            role.to_i64()
        ).execute(self.ctx.conn())
            .await?;

        Ok(())
    }

    /// Returns whether or not the role is in the joinable roles list.
    pub async fn is_joinable(&self, role: VerifiedRole) -> crate::error::Result<bool> {
        Ok(sqlx::query_scalar!(
                    "SELECT COUNT(*) AS matching FROM joinable_roles WHERE guild = $1 AND role = $2;",
                    self.ctx.guild_as_i64(),
                    role.to_i64()
                ).fetch_one(self.ctx.conn())
            .await?
            .unwrap_or_default() > 0)
    }

    /// Retrieves the list of joinable roles. Keeping this query sane is why
    /// we limit the number of joinable roles.
    pub async fn joinable_roles(&self) -> crate::error::Result<Vec<RoleId>> {
        let s: Vec<i64> = sqlx::query_scalar!(
            "SELECT role FROM joinable_roles WHERE guild = $1 ORDER BY role ASC;",
            self.ctx.guild_as_i64()
        ).fetch_all(self.ctx.conn())
            .await?;
        let mut out = Vec::with_capacity(s.len());
        out.extend(
            s.into_iter()
                .map(|r| RoleId::from(r as u64))
        );
        Ok(out)
    }
}

impl_err!(RoleNotSelfAssignable, "Role is not self-assignable/removable.", true);

#[async_trait::async_trait]
impl Module for RoleModule {
    fn info(&self) -> &ModInfo {
        #[doc(hidden)]
        static INFO: Lazy<ModInfo> = Lazy::new(|| ModInfo::with_name("role", "allows users to self-manage roles.")
            .with_sensitivity(Sensitivity::Low)
            .with_filter(false)
            .with_command(true));
        &INFO
    }

    async fn process(&self, dis: &Dispatch, ctx: &Context, orig: &Message, command: Vec<String>) -> crate::error::Result<()> {
        let role_opts = RoleOpt::from_iter_with_help(command)?;
        let gid = orig.guild_id.unwrap();

        let db = DbContext::new(dis, gid);
        let join = JoinableRoles::new(db);

        match &role_opts {
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

                let is_joinable = join.is_joinable(vrole).await?;

                if !is_joinable {
                    return Err(RoleNotSelfAssignable.into());
                }

                let guild = gid.to_guild_cached(ctx)
                    .await
                    .ok_or(GuildNotInCache)?;
                let mut mem = guild.member(ctx, orig.author.id)
                    .await?;

                match &role_opts {
                    RoleOpt::Join { .. } => {
                        mem.add_role(ctx, vrole.into_inner()).await?;
                    }
                    _ => {
                        mem.remove_role(ctx, vrole.into_inner()).await?;
                    }
                }
            }
            RoleOpt::ListJoinable => {
                let roles = join.joinable_roles().await?;
                let roles: Vec<_> = futures::stream::iter(roles
                    .into_iter())
                    .then(|r| async move { r.to_role_name_or_id(ctx, gid).await })
                    .collect()
                    .await;

                let message = if roles.is_empty() {
                    "No joinable roles.".to_string()
                } else {
                    roles.join(", ")
                };

                let msg = MessageBuilder::new()
                    .push_codeblock_safe(message, None)
                    .build();
                orig.reply(ctx, msg).await?;
                return Ok(());
            }
        };

        orig.react(ctx, '✅').await?;
        Ok(())
    }
}

/// Represents the `mod-role` command.
pub struct ModRoleModule;

/// Represents whether a user should be assigned or unassigned a role.
#[derive(Debug, StructOpt)]
#[structopt(no_version)]
enum UserAction {
    /// Adds a role to a user
    Assign,
    /// Removes a role from a user
    Unassign,
}

#[derive(StructOpt)]
#[structopt(name = "mod-role", no_version)]
/// Command to manage roles that users can join on their own.
enum ModRoleOpt {
    /// Makes a role joinable.
    AddJoinable {
        /// The role to make joinable.
        role: String,
    },
    /// Removes a role from the joinable list.
    DelJoinable {
        /// The role to remove from being joinable.
        role: String,
    },
    /// Assign a role to a user.
    Assign {
        /// The role on which an action will be performed.
        role: String,
        /// The user to assign/unassign a role to.
        user: String,
    },
    /// Unassign a role to a user.
    Unassign {
        /// The role on which an action will be performed.
        role: String,
        /// The user to assign/unassign a role to.
        user: String,
    },
}

impl ModRoleOpt {
    /// Extracts the role string from the arguments
    pub fn extract_role(&self) -> &str {
        match self {
            ModRoleOpt::AddJoinable { role, .. } => { role.as_str() }
            ModRoleOpt::DelJoinable { role, .. } => { role.as_str() }
            ModRoleOpt::Assign { role, .. } => { role.as_str() }
            ModRoleOpt::Unassign { role, .. } => { role.as_str() }
        }
    }

    /// Extracts the user string from the arguments
    pub fn extract_user(&self) -> Option<&str> {
        match self {
            ModRoleOpt::AddJoinable { .. } |
            ModRoleOpt::DelJoinable { .. } => { None }
            ModRoleOpt::Assign { user, .. } => { Some(user.as_ref()) }
            ModRoleOpt::Unassign { user, .. } => { Some(user.as_ref()) }
        }
    }

    /// Returns true if this is an assign variant.
    pub fn is_assign(&self) -> bool {
        match self {
            ModRoleOpt::Assign { .. } => { true }
            _ => false
        }
    }
}

#[async_trait::async_trait]
impl Module for ModRoleModule {
    fn info(&self) -> &ModInfo {
        #[doc(hidden)]
        static INFO: Lazy<ModInfo> = Lazy::new(|| {
            ModInfo::with_name("mod-role", "allows moderators to assign/unassign roles, and to make/unmake roles assignable.")
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

        let db = DbContext::new(dis, gid);
        let join = JoinableRoles::new(db);
        let user = futures::stream::iter(opts.extract_user())
            .then(|s| VerifiedUser::from_str_with_ctx(s, ctx, gid))
            .next()
            .await
            .transpose()?;

        match opts {
            ModRoleOpt::AddJoinable { .. } => {
                join.add_joinable_role(role).await?;
                "Set role to joinable."
            }
            ModRoleOpt::DelJoinable { .. } => {
                join.del_joinable_role(role).await?;
                "Role is/was no longer joinable."
            }
            _ => {
                let user = user.unwrap();
                let mut member = gid.to_guild_cached(ctx)
                    .await
                    .ok_or(GuildNotInCache)?
                    .member(ctx, user.into_inner())
                    .await
                    .map_err(|_| NoSuchUser)?;

                if opts.is_assign() {
                    member.add_role(ctx, role.into_inner())
                        .await?;
                    "Added role to user."
                } else {
                    member.remove_role(ctx, role.into_inner())
                        .await?;
                    "Removed role from user if they had it."
                }
            }
        };

        orig.react(ctx, '✅').await?;
        Ok(())
    }
}
use std::borrow::Borrow;

use futures::StreamExt;
use once_cell::sync::Lazy;
use serenity::client::Context;
use serenity::model::channel::Message;
use serenity::model::prelude::RoleId;
use shrinkwraprs::Shrinkwrap;
use structopt::StructOpt;

use crate::db::DbContext;
use crate::dispatch::config::{FromStrWithCtx, NoSuchUser, RoleExt, VerifiedUser};
use crate::dispatch::config::VerifiedRole;
use crate::dispatch::Dispatch;
use crate::error::{DatabaseError, GuildNotInCache, RoleNotInCache, UserError};
use crate::module::{ModInfo, Module, Sensitivity};
use crate::module::privilege::ensure_authorized_for_role;
use crate::util::ClapExt;

pub struct RoleModule;

pub const JOINABLE_ROLES_KEY: &str = "joinable_roles";

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

#[derive(Shrinkwrap)]
pub struct JoinableRoles<'pool> {
    ctx: DbContext<'pool>
}

impl_err!(TooManyRoles, "Can't add more roles to joinable; you have too many!", true);
impl_err!(AlreadyJoinable, "This role is already joinable.", true);

impl<'pool> JoinableRoles<'pool> {
    pub fn new(ctx: impl Borrow<DbContext<'pool>>) -> Self {
        JoinableRoles { ctx: ctx.borrow().clone() }
    }

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

    pub async fn del_joinable_role(&self, role: VerifiedRole) -> crate::error::Result<()> {
        sqlx::query!(
            "DELETE FROM joinable_roles WHERE guild = $1 AND role = $2;",
            self.ctx.guild_as_i64(),
            role.to_i64()
        ).execute(self.ctx.conn())
            .await?;

        Ok(())
    }

    pub async fn is_joinable(&self, role: VerifiedRole) -> crate::error::Result<bool> {
        #[derive(Debug)]
        struct Row {
            matching: Option<i64>
        }

        Ok(sqlx::query_as!(
                    Row,
                    "SELECT COUNT(*) AS matching FROM joinable_roles WHERE guild = $1 AND role = $2;",
                    self.ctx.guild_as_i64(),
                    role.to_i64()
                ).fetch_one(self.ctx.conn())
            .await?
            .matching
            .unwrap_or_default() > 0)
    }

    pub async fn joinable_roles(&self) -> crate::error::Result<Vec<RoleId>> {
        #[derive(Debug)]
        struct Row {
            role: i64
        }
        let s: Vec<Row> = sqlx::query_as!(
            Row,
            "SELECT role FROM joinable_roles WHERE guild = $1 ORDER BY role ASC;",
            self.ctx.guild_as_i64()
        ).fetch_all(self.ctx.conn())
            .await?;
        let mut out = Vec::with_capacity(s.len());
        out.extend(
            s.into_iter()
                .map(|r| RoleId::from(r.role as u64))
        );
        Ok(out)
    }
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

        let db = DbContext::new(dis.pool(), gid);
        let join = JoinableRoles::new(db);

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

                let is_joinable = join.is_joinable(vrole).await?;

                if !is_joinable {
                    return Err(UserError::new(format!("{} is not user-addable/removable.", vrole.to_role_name_or_id(ctx, gid).await)).into());
                }

                let guild = gid.to_guild_cached(ctx)
                    .await
                    .ok_or(GuildNotInCache)?;
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
                let roles = join.joinable_roles().await?;
                let roles: Vec<_> = futures::stream::iter(roles
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

        orig.react(ctx, '✅').await?;
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

        let db = DbContext::new(dis.pool(), gid);
        let join = JoinableRoles::new(db);

        let message = match opts.action {
            Action::AddJoinable => {
                join.add_joinable_role(role).await?;
                "Set role to joinable."
            }
            Action::DelJoinable => {
                join.del_joinable_role(role).await?;
                "Role is/was no longer joinable."
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

        orig.react(ctx, '✅').await?;
        Ok(())
    }
}
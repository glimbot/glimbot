use std::fmt;
use std::fmt::Formatter;
use std::ops::Deref;
use std::str::FromStr;

use once_cell::sync::Lazy;
use serenity::client::Context;
use serenity::model::channel::Message;
use serenity::model::id::{GuildId, RoleId};
use crate::db::DbContext;
use crate::dispatch::{config, Dispatch};
use crate::dispatch::config::{FromStrWithCtx, VerifiedRole};
use crate::error::{IntoBotErr, SysError, UserError, GuildNotInCache, RoleNotInCache, InsufficientPermissions, DeputyConfused};
use crate::module::{ModInfo, Module, Sensitivity};
use serenity::model::guild::{Member, Role};

pub struct PrivilegeFilter;

pub const PRIV_NAME: &'static str = "privileged_role";

#[async_trait::async_trait]
impl Module for PrivilegeFilter {
    fn info(&self) -> &ModInfo {
        static INFO: Lazy<ModInfo> = Lazy::new(|| {
            ModInfo::with_name("privilege-check")
                .with_filter(true)
                .with_sensitivity(Sensitivity::High)
                .with_config_value(config::Value::<VerifiedRole>::new(PRIV_NAME, "A role which may run commands requiring elevated privilege."))
        });
        &INFO
    }

    async fn filter(&self, dis: &Dispatch, ctx: &Context, orig: &Message, name: String) -> crate::error::Result<String> {
        let cmd = dis.command_module(&name)?;
        if cmd.info().sensitivity < Sensitivity::High {
            trace!("Not a sensitive command.");
            return Ok(name);
        }

        // Either an owner command or a high command. Owner commands are handled by a different module.
        let guild_owner = orig
            .guild_field(ctx, |g| g.owner_id)
            .await
            .ok_or_else(|| SysError::new("Couldn't retrieve guild info."))?;

        if orig.author.id == guild_owner {
            debug!("Guild owner ran command.");
            return Ok(name);
        }

        // Gotta hit the DB
        let v = dis.config_value_t::<VerifiedRole>(PRIV_NAME)?;
        let db = DbContext::new(dis.pool(), orig.guild_id.unwrap());
        let mod_role = v.get(&db).await?
            .ok_or_else(|| UserError::new("Need to set a moderator role -- see privileged_role config option."))?;

        if orig.author.has_role(ctx, orig.guild_id.unwrap(), mod_role.into_inner()).await? {
            trace!("Mod ran command.");
            Ok(name)
        } else {
            Err(UserError::new("You do not have permission to run that command.").into())
        }
    }
}

/// Returns Ok(()) if this member has the permissions to take on this role, false otherwise.
/// Necessary to avoid confused deputy issues.
#[instrument(level = "debug", skip(ctx, mem, role), fields(r = % role.id))]
pub async fn ensure_authorized_for_role(ctx: &Context, mem: &Member, role: &Role) -> crate::error::Result<()> {
    let guild = mem.guild_id.to_guild_cached(ctx)
        .await
        .ok_or(GuildNotInCache)?;
    debug!("Checking if owner.");
    if guild.owner_id == mem.user.id {
        debug!("Command run by guild owner.");
        return Ok(());
    }

    debug!("Not owner; checking highest role.");
    let (_max_role, pos) = mem.highest_role_info(ctx)
        .await
        .ok_or(RoleNotInCache)?;

    if pos < role.position {
        debug!("User role not high enough: {} < {}", pos, role.position);
        Err(DeputyConfused.into())
    } else {
        debug!("User authorized.");
        Ok(())
    }
}
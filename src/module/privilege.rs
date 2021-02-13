use crate::module::{Module, ModInfo, Sensitivity};
use once_cell::sync::Lazy;
use crate::dispatch::{config, Dispatch};
use serenity::model::id::{RoleId, GuildId};
use std::ops::Deref;
use crate::dispatch::config::FromStrWithCtx;
use serenity::client::Context;
use std::str::FromStr;
use crate::error::{IntoBotErr, SysError, UserError};
use serenity::model::channel::Message;
use crate::db::DbContext;
use std::fmt;
use std::fmt::Formatter;

#[derive(Debug, Copy, Clone, serde::Serialize, serde::Deserialize)]
pub struct VerifiedRole(u64);

impl VerifiedRole {
    pub fn into_inner(self) -> RoleId {
        self.0.into()
    }
}

#[async_trait::async_trait]
impl FromStrWithCtx for VerifiedRole {
    type Err = crate::error::Error;

    async fn from_str_with_ctx(s: &str, ctx: &Context, gid: GuildId) -> Result<Self, Self::Err> {
        let guild_info = gid.to_guild_cached(ctx)
            .await
            .ok_or_else(|| SysError::new("Couldn't find guild in cache"))?;
        let role_id = if let Ok(id) = RoleId::from_str(s) {
            guild_info.roles.get(&id)
        } else {
            guild_info.role_by_name(s)
        }.ok_or_else(|| UserError::new(format!("No such role in this guild: {}", s)))?;

        Ok(Self(role_id.id.0))
    }
}

impl fmt::Display for VerifiedRole {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "<@&{}>", self.0)
    }
}

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
        let db = DbContext::new(orig.guild_id.unwrap()).await?;
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
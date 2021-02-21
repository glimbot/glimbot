use std::any::Any;
use std::fmt;
use std::fmt::Formatter;
use std::str::FromStr;
use std::sync::Arc;

use downcast_rs::DowncastSync;
use downcast_rs::impl_downcast;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serenity::client::Context;
use serenity::model::id::{GuildId, RoleId, ChannelId, UserId};
use sled::IVec;

use crate::db::{DbContext, DbKey};
use crate::error::{IntoBotErr, SysError, UserError, GuildNotInCache};
use std::borrow::Cow;
use serenity::model::misc::Mentionable;
use serenity::model::guild::Member;

pub trait ValueType: Serialize + DeserializeOwned + FromStrWithCtx + Send + Sync + Any + Sized + fmt::Display {}

#[derive(Debug)]
pub struct Value<T>
    where T: ValueType {
    name: &'static str,
    help: &'static str,
    default: Option<T>,
}

impl<T: Serialize + DeserializeOwned + FromStrWithCtx + Send + Sync + Any + Sized + fmt::Display> ValueType for T {}

impl<T> Value<T> where T: ValueType {
    pub fn new(name: &'static str, help: &'static str) -> Self {
        Value { name, help, default: None }
    }

    pub fn with_default(name: &'static str, help: &'static str, default: T) -> Self {
        let mut out = Self::new(name, help);
        out.default = Some(default);
        out
    }

    pub async fn get_or_insert(&self, ctx: &DbContext, value: T) -> crate::error::Result<T> {
        ctx.get_or_insert(self.name, value).await
    }

    pub async fn get_or_default(&self, ctx: &DbContext) -> crate::error::Result<T> where T: Clone {
        if self.default.is_none() {
            return Err(UserError::new(format!("No default specified for {}", self.name)).into());
        }

        ctx.get_or_insert(self.name, self.default.clone().unwrap()).await
    }

    pub async fn get(&self, ctx: &DbContext) -> crate::error::Result<Option<T>> {
        ctx.get(self.name).await
    }

    pub async fn set(&self, ctx: &DbContext, value: T) -> crate::error::Result<()> {
        ctx.insert(self.name, value).await
    }
}

#[async_trait::async_trait]
pub trait FromStrWithCtx: Sized {
    type Err: std::error::Error + Send + Sized + 'static;

    async fn from_str_with_ctx(s: &str, ctx: &Context, gid: GuildId) -> Result<Self, Self::Err>;
}

#[async_trait::async_trait]
impl<T> FromStrWithCtx for T where T: FromStr, T::Err: std::error::Error + Send + Sized + 'static {
    type Err = <T as FromStr>::Err;

    async fn from_str_with_ctx(s: &str, ctx: &Context, gid: GuildId) -> Result<Self, Self::Err> {
        Self::from_str(s)
    }
}

#[macro_export]
macro_rules! impl_not_user_settable {
    ($t:path) => {
        #[async_trait::async_trait]
        impl FromStrWithCtx for $t {
            type Err = $crate::error::UserError;

            async fn from_str_with_ctx(_s: &str, _ctx: &Context, _gid: GuildId) -> Result<Self, Self::Err> {
                Err(UserError::new("This config value cannot be set by users."))
            }
        }
    };
}


#[async_trait::async_trait]
pub trait Validator: Send + Sync + Any + DowncastSync + 'static {
    fn name(&self) -> &'static str;
    fn help(&self) -> &'static str;
    async fn validate(&self, ctx: &Context, gid: GuildId, s: &str) -> crate::error::Result<IVec>;
    fn display_value(&self, v: IVec) -> crate::error::Result<String>;
}
impl_downcast!(sync Validator);

#[async_trait::async_trait]
impl<T> Validator for Value<T> where T: ValueType {
    fn name(&self) -> &'static str {
        self.name
    }

    fn help(&self) -> &'static str {
        self.help
    }

    async fn validate(&self, ctx: &Context, gid: GuildId, s: &str) -> crate::error::Result<IVec> {
        let s: T = T::from_str_with_ctx(s, ctx, gid).await.into_user_err()?;
        let out = rmp_serde::to_vec(&s)?;
        Ok(out.into())
    }

    fn display_value(&self, v: IVec) -> crate::error::Result<String> {
        let v: T = rmp_serde::from_read(v.as_ref())?;
        Ok(v.to_string())
    }
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize, Hash, Eq, PartialEq)]
pub struct VerifiedRole(u64);

impl VerifiedRole {
    pub fn into_inner(self) -> RoleId {
        self.0.into()
    }

    pub fn into_be_bytes(self) -> [u8; 8] {
        self.0.to_be_bytes()
    }

    pub async fn to_role_name(&self, ctx: &Context, guild: GuildId) -> crate::error::Result<String> {
        let rid = self.into_inner();
        rid.to_role_name(ctx, guild).await
    }

    pub async fn to_role_name_or_id(&self, ctx: &Context, guild: GuildId) -> String {
        self.into_inner().to_role_name_or_id(ctx, guild).await
    }
}

#[async_trait::async_trait]
pub trait RoleExt {
    async fn to_role_name(&self, ctx: &Context, guild: GuildId) -> crate::error::Result<String>;
    async fn to_role_name_or_id(&self, ctx: &Context, guild: GuildId) -> String;
}

#[async_trait::async_trait]
impl RoleExt for RoleId {
    async fn to_role_name(&self, ctx: &Context, guild: GuildId) -> crate::error::Result<String> {
        let rid = *self;
        let g = guild.to_guild_cached(ctx).await
            .ok_or_else(|| SysError::new("Couldn't find guild in cache."))?;
        let role = g.roles.get(&rid)
            .ok_or_else(|| UserError::new("No such role in this guild."))?;
        Ok(role.name.clone())
    }

    async fn to_role_name_or_id(&self, ctx: &Context, guild: GuildId) -> String {
        self.to_role_name(ctx, guild)
            .await
            .unwrap_or_else(|_| self.to_string())
    }
}

#[async_trait::async_trait]
impl FromStrWithCtx for VerifiedRole {
    type Err = crate::error::Error;

    async fn from_str_with_ctx(s: &str, ctx: &Context, gid: GuildId) -> Result<Self, Self::Err> {
        let guild_info = gid.to_guild_cached(ctx)
            .await
            .ok_or(GuildNotInCache)?;
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

#[derive(Debug, Copy, Clone, Serialize, Deserialize, Hash, Eq, PartialEq)]
pub struct VerifiedChannel(ChannelId);

#[async_trait::async_trait]
impl FromStrWithCtx for VerifiedChannel {
    type Err = crate::error::Error;

    async fn from_str_with_ctx(s: &str, ctx: &Context, gid: GuildId) -> Result<Self, Self::Err> {
        let guild_info = gid.to_guild_cached(ctx)
            .await
            .ok_or(GuildNotInCache)?;
        let chan_id = if let Ok(id) = ChannelId::from_str(s) {
            guild_info.channels.get(&id).map(|c| c.id)
        } else {
            guild_info.channel_id_from_name(ctx, s).await
        }.ok_or_else(|| UserError::new(format!("No such channel in this guild: {}", s)))?;

        Ok(Self(chan_id))
    }
}

impl fmt::Display for VerifiedChannel {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "<#{}>", self.0)
    }
}

impl VerifiedChannel {
    pub fn into_inner(self) -> ChannelId {
        self.0
    }

    pub async fn to_channel_name(&self, ctx: &Context, guild: GuildId) -> crate::error::Result<String> {
        let cid = self.into_inner();
        let g = guild.to_guild_cached(ctx).await
            .ok_or_else(|| SysError::new("Couldn't find guild in cache."))?;
        let role = g.channels.get(&cid)
            .ok_or_else(|| UserError::new(format!("No such channel in this guild: {}", &self)))?;
        Ok(role.name.clone())
    }

    pub async fn to_channel_name_or_id(&self, ctx: &Context, guild: GuildId) -> String {
        self.to_channel_name(ctx, guild)
            .await
            .unwrap_or_else(|_| self.to_string())
    }
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize, Hash, Eq, PartialEq)]
pub struct VerifiedUser(UserId);

impl VerifiedUser {
    pub fn into_inner(self) -> UserId {
        self.0
    }

    pub async fn to_user_name(&self, ctx: &Context, guild: GuildId) -> crate::error::Result<String> {
        let uid = self.into_inner();
        let g = guild.to_guild_cached(ctx).await
            .ok_or_else(|| SysError::new("Couldn't find guild in cache."))?;
        let member = g.members.get(&uid)
            .ok_or_else(|| UserError::new(format!("No such user in this guild: {}", &self)))?;
        Ok(member.nick.clone().unwrap_or_else(|| member.user.name.clone()))
    }

    pub async fn to_user_name_or_id(&self, ctx: &Context, guild: GuildId) -> String {
        self.to_user_name(ctx, guild)
            .await
            .unwrap_or_else(|_| self.to_string())
    }
}

impl_err!(NoSuchUser, "No such user in guild, or two members have the same nickname.", true);

#[async_trait::async_trait]
impl FromStrWithCtx for VerifiedUser {
    type Err = crate::error::Error;

    async fn from_str_with_ctx(s: &str, ctx: &Context, gid: GuildId) -> Result<Self, Self::Err> {
        let guild = gid.to_guild_cached(ctx)
            .await
            .ok_or(GuildNotInCache)?;
        let uid: Member = if let Ok(id) = UserId::from_str(s) {
            guild.member(ctx, id)
                .await
                .ok()
        } else {
            guild.member_named(s).cloned()
        }.ok_or(NoSuchUser)?;

        Ok(VerifiedUser(uid.user.id))
    }
}

impl fmt::Display for VerifiedUser {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.mention())
    }
}

impl DbKey for VerifiedRole {
    fn to_key(&self) -> Cow<[u8]> {
        self.0.to_be_bytes().to_vec().into()
    }
}

impl_id_db_key! {
    VerifiedUser,
    VerifiedChannel
}
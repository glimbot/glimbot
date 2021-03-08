use std::any::Any;
use std::fmt;
use std::fmt::Formatter;
use std::str::FromStr;

use downcast_rs::DowncastSync;
use downcast_rs::impl_downcast;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serenity::client::Context;
use serenity::model::guild::Member;
use serenity::model::id::{ChannelId, GuildId, RoleId, UserId};
use serenity::model::misc::Mentionable;

use crate::db::DbContext;
use crate::error::{GuildNotInCache, IntoBotErr, SysError, UserError};
use serenity::model::error::Error::GuildNotFound;

/// A trait specifying that a type can be set as a value.
pub trait ValueType: Serialize + DeserializeOwned + FromStrWithCtx + Send + Sync + Any + Sized + fmt::Display {}

/// Represents a config value, allowing for enforcement of type issues.
#[derive(Debug)]
pub struct Value<T>
    where T: ValueType {
    /// The name of the config value.
    name: &'static str,
    /// An about description for the config value.
    help: &'static str,
    /// A default value which can be used if `T: Clone` to set an unset config value.
    default: Option<T>,
}

impl<T: Serialize + DeserializeOwned + FromStrWithCtx + Send + Sync + Any + Sized + fmt::Display> ValueType for T {}

impl_err!(NoDefaultSpecified, "No default is specified for that value.", true);

impl<T> Value<T> where T: ValueType {
    /// Creates a value with the given name and help, without support for get_or_default.
    pub fn new(name: &'static str, help: &'static str) -> Self {
        Value { name, help, default: None }
    }

    /// Creates a value with the given name and help, and with the specified default.
    pub fn with_default(name: &'static str, help: &'static str, default: T) -> Self where T: Clone {
        let mut out = Self::new(name, help);
        out.default = Some(default);
        out
    }

    /// Retrieves the value associated with this value's name, setting it atomically if it doesn't
    /// exist.
    pub async fn get_or_insert(&self, ctx: &DbContext<'_>, value: T) -> crate::error::Result<T> {
        ctx.get_or_insert(self.name, value).await
    }

    /// Retrieves the value associated with this value's name, setting it atomically if it doesn't
    /// exist using the default specified when this value was constructed.
    pub async fn get_or_default(&self, ctx: &DbContext<'_>) -> crate::error::Result<T> where T: Clone {
        if self.default.is_none() {
            return Err(NoDefaultSpecified.into());
        }

        ctx.get_or_insert(self.name, self.default.clone().unwrap()).await
    }

    /// Retrieves the value associated with this value's name, returning `None` if hasn't been set.
    pub async fn get(&self, ctx: &DbContext<'_>) -> crate::error::Result<Option<T>> {
        ctx.get(self.name).await
    }

    /// Sets the value associated with this value's name.
    pub async fn set(&self, ctx: &DbContext<'_>, value: T) -> crate::error::Result<()> {
        ctx.insert(self.name, value).await
    }
}

/// Trait for converting arbitrary types from strings into values
/// with the added context of a context and the relevant guild.
#[async_trait::async_trait]
pub trait FromStrWithCtx: Sized {
    /// The associated error type for the conversion.
    type Err: std::error::Error + Send + Sized + 'static;
    /// Converts a string into an object as mentioned above.
    async fn from_str_with_ctx(s: &str, ctx: &Context, gid: GuildId) -> Result<Self, Self::Err>;
}

#[async_trait::async_trait]
impl<T> FromStrWithCtx for T where T: FromStr, T::Err: std::error::Error + Send + Sized + 'static {
    type Err = <T as FromStr>::Err;

    async fn from_str_with_ctx(s: &str, _ctx: &Context, _gid: GuildId) -> Result<Self, Self::Err> {
        Self::from_str(s)
    }
}

/// Marks a value as not being able to be set by users.
#[deprecated]
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

/// A trait meant to be specified by validators of config values.
#[async_trait::async_trait]
pub trait Validator: Send + Sync + Any + DowncastSync + 'static {
    /// Retrieves the name associated with a config value.
    fn name(&self) -> &'static str;
    /// Retrieves the help string associated with a config value.
    fn help(&self) -> &'static str;
    /// Converts a string into a [`serde_json::Value`].
    async fn validate(&self, ctx: &Context, gid: GuildId, s: &str) -> crate::error::Result<serde_json::Value>;
    /// Converts a JSON representation of the associated type into a string.
    fn display_value(&self, v: serde_json::Value) -> crate::error::Result<String>;
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

    async fn validate(&self, ctx: &Context, gid: GuildId, s: &str) -> crate::error::Result<serde_json::Value> {
        let s: T = T::from_str_with_ctx(s, ctx, gid).await.into_user_err()?;
        Ok(serde_json::to_value(s)?)
    }

    fn display_value(&self, v: serde_json::Value) -> crate::error::Result<String> {
        let v: T = serde_json::from_value(v)?;
        Ok(v.to_string())
    }
}

// TODO: Make this a shrinkwrap
/// A role which has been verified to exist in a guild.
#[derive(Debug, Copy, Clone, Serialize, Deserialize, Hash, Eq, PartialEq, Shrinkwrap)]
pub struct VerifiedRole(RoleId);

impl VerifiedRole {
    /// Extracts the inner `RoleId`.
    pub fn into_inner(self) -> RoleId {
        self.0.into()
    }
    /// Converts the internal value into an i64, mostly for use with SQL DBs.
    pub fn to_i64(&self) -> i64 {
        self.0.0 as i64
    }
    /// Converts the internal value into its big-endian representation.
    pub fn into_be_bytes(self) -> [u8; 8] {
        self.0.0.to_be_bytes()
    }
}

/// Extension trait to convert RoleId into the relevant bits.
#[async_trait::async_trait]
pub trait RoleExt {
    /// Given a context, converts this role into the name in a guild.
    // TODO: replace with `context_safe`
    async fn to_role_name(&self, ctx: &Context, guild: GuildId) -> crate::error::Result<String>;
    /// Converts this role into the name in a guild, or just the id if it can't be determined.
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

impl_err!(NoSuchRole, "There is no such role in this guild.", true);

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
        }.ok_or(NoSuchRole)?;

        Ok(Self(role_id.id))
    }
}

impl fmt::Display for VerifiedRole {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "<@&{}>", self.0)
    }
}

/// A wrapper around a channel known to exist in a guild.
#[derive(Debug, Copy, Clone, Serialize, Deserialize, Hash, Eq, PartialEq, Shrinkwrap)]
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

impl_err!(NoSuchChannel, "No such channel in this guild.", true);

impl VerifiedChannel {
    /// Converts this value into its internal representation.
    pub fn into_inner(self) -> ChannelId {
        self.0
    }

    /// Converts this channel into the text name, returning an error if it can't.
    pub async fn to_channel_name(&self, ctx: &Context, guild: GuildId) -> crate::error::Result<String> {
        let cid = self.into_inner();
        let g = guild.to_guild_cached(ctx).await
            .ok_or(GuildNotInCache)?;
        let role = g.channels.get(&cid)
            .ok_or(NoSuchChannel)?;
        Ok(role.name.clone())
    }

    /// Converts this channel into the text name, returning the raw id if it can't.
    pub async fn to_channel_name_or_id(&self, ctx: &Context, guild: GuildId) -> String {
        self.to_channel_name(ctx, guild)
            .await
            .unwrap_or_else(|_| self.to_string())
    }
}

/// A wrapper around a user id guaranteed to exist in a guild.
#[derive(Debug, Copy, Clone, Serialize, Deserialize, Hash, Eq, PartialEq, Shrinkwrap)]
pub struct VerifiedUser(UserId);

impl VerifiedUser {
    /// Converts this value into its internal representation.
    pub fn into_inner(self) -> UserId {
        self.0
    }

    /// Converts this user into the text name.
    pub async fn to_user_name(&self, ctx: &Context, guild: GuildId) -> crate::error::Result<String> {
        let uid = self.into_inner();
        let g = guild.to_guild_cached(ctx).await
            .ok_or(GuildNotInCache)?;
        let member = g.members.get(&uid)
            .ok_or(NoSuchUser)?;
        Ok(member.nick.clone().unwrap_or_else(|| member.user.name.clone()))
    }

    /// Converts this user into the text name, or the raw id if it can't.
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
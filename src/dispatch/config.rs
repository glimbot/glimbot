use std::sync::Arc;
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::str::FromStr;
use sled::IVec;
use std::any::Any;
use crate::db::DbContext;
use crate::error::{IntoBotErr, SysError, UserError};
use downcast_rs::DowncastSync;
use downcast_rs::impl_downcast;
use serenity::client::Context;
use serenity::model::id::GuildId;
use std::fmt;

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
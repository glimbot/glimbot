use std::sync::Arc;
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::str::FromStr;
use std::marker::PhantomData;
use sled::IVec;
use std::any::Any;
use crate::db::DbContext;
use crate::error::{IntoBotErr, SysError, UserError};
use downcast_rs::DowncastSync;
use downcast_rs::impl_downcast;

pub trait ValueType: Serialize + DeserializeOwned + FromStr + Send + Sync + Any + Sized {}

#[derive(Debug)]
pub struct Value<T>
    where T: ValueType,
          <T as FromStr>::Err: std::error::Error + Send + Sized + 'static {
    name: &'static str,
    help: &'static str,
    default: Option<T>,
}

impl<T: Serialize + DeserializeOwned + FromStr + Send + Sync + Any + Sized> ValueType for T {}

impl<T> Value<T> where T: ValueType,
                       <T as FromStr>::Err: std::error::Error + Send + Sized + 'static {
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
pub trait Validator: Send + Sync + Any + DowncastSync + 'static {
    fn name(&self) -> &'static str;
    fn help(&self) -> &'static str;
    fn validate(&self, s: &str) -> crate::error::Result<IVec>;
}
impl_downcast!(sync Validator);

#[async_trait::async_trait]
impl<T> Validator for Value<T> where T: ValueType,
                                     <T as FromStr>::Err: std::error::Error + Send + Sized + 'static {
    fn name(&self) -> &'static str {
        self.name
    }

    fn help(&self) -> &'static str {
        self.help
    }

    fn validate(&self, s: &str) -> crate::error::Result<IVec> {
        let s: T = T::from_str(s).into_user_err()?;
        let out = bincode::serialize(&s)?;
        Ok(out.into())
    }
}
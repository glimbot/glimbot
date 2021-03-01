pub mod timed;

use once_cell::sync::Lazy;
use std::path::{Path, PathBuf};
use std::io;
use serenity::model::id::{GuildId, UserId, ChannelId, RoleId};
use tokio::task;
use serde::{Serialize};
use serde::de::DeserializeOwned;
use sled::CompareAndSwapError;
use serenity::futures::StreamExt;
use std::borrow::Cow;
use std::ops::Deref;
use smallvec::SmallVec;
use crate::util::FlipResultExt;
use sled::transaction::ConflictableTransactionError::Abort;
use sled::transaction::TransactionError;

pub fn default_data_folder() -> PathBuf {
    static DEFAULT_PATH: Lazy<PathBuf> = Lazy::new(|| {
        let mut base = dirs::data_local_dir().unwrap();
        base.push("glimbot");
        base
    });

    DEFAULT_PATH.clone()
}

pub fn ensure_data_folder() -> io::Result<PathBuf> {
    let dir = std::env::var("GLIMBOT_DIR")
        .map(|s| shellexpand::full(&s).expect("Failed while expanding directory").to_string())
        .map(PathBuf::from)
        .unwrap_or_else(|_| default_data_folder());

    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

pub fn ensure_db() -> sled::Db {
    let mut path = ensure_data_folder().expect("Failed to create data directory");
    path.push("glimbot.sled");
    let conf = sled::Config::default()
        .path(path);
    let db = conf.open().expect("Failed while opening db");
    db
}

fn db() -> &'static sled::Db {
    static DB: Lazy<sled::Db> = Lazy::new(|| {
        ensure_db()
    });

    &DB
}

#[derive(Clone)]
pub struct DbContext {
    guild: GuildId,
    tree: sled::Tree,
}

impl DbContext {
    pub fn tree(&self) -> &sled::Tree {
        &self.tree
    }
}

impl DbContext {
    pub async fn new(guild: GuildId) -> crate::error::Result<Self> {
        let bytes = guild.0.to_be_bytes();
        let tree = task::spawn_blocking(move || {
            let db = db();
            db.open_tree(bytes)
        }).await.unwrap()?;

        Ok(Self {
            guild,
            tree,
        })
    }

    pub async fn with_namespace(guild: GuildId, namespace: &str) -> crate::error::Result<Self> {
        let bytes = guild.0.to_be_bytes();
        // Avoids allocation in a hotpath.
        let mut name = SmallVec::<[_; 64]>::with_capacity(bytes.len() + namespace.as_bytes().len());
        name.extend_from_slice(&bytes[..]);
        name.extend_from_slice(namespace.as_bytes());
        let tree = task::spawn_blocking(move || {
            let db = db();
            db.open_tree(name)
        }).await.unwrap()?;

        Ok(Self {
            guild,
            tree,
        })
    }

    pub async fn do_async<F, R>(&self, f: F) -> R where F: (FnOnce(Self) -> R) + Send + 'static, R: Send + 'static {
        let c = self.clone();
        task::spawn_blocking(move || f(c)).await.unwrap()
    }

    pub async fn do_async_in_place<F, R>(&self, f: F) -> R where F: FnOnce(Self) -> R {
        let c = self.clone();
        task::block_in_place(move || f(c))
    }

    pub async fn get_or_insert<B, S>(&self, key: B, def: S) -> crate::error::Result<S>
        where B: DbKey + Send + 'static,
              S: Serialize + DeserializeOwned + Send + 'static {
        self.do_async(move |s| {
            s.get_or_insert_sync(key, def)
        }).await
    }

    pub async fn apply<B, S, F>(&self, key: B, f: F) -> crate::error::Result<Option<S>>
        where B: DbKey + Send + 'static,
              S: Serialize + DeserializeOwned + Send + 'static,
              F: Fn(Option<&S>) -> Option<S> + Send + 'static {
        self.do_async(move |c| {
            let key = key.to_key();
            let res = c.tree().transaction(move |t| {
                let v = t.get(key.as_ref())?;
                let value = v.map(|r| rmp_serde::from_read_ref(&r))
                    .flip()
                    .map_err(crate::error::Error::from)
                    .map_err(Abort)?;
                let new_v = f(value.as_ref());

                match new_v {
                    None => {
                        t.remove(key.as_ref())?;
                    }
                    Some(v) => {
                        let ser = rmp_serde::to_vec(&v)
                            .map_err(crate::error::Error::from)
                            .map_err(Abort)?;
                        t.insert(key.as_ref(), ser)?;
                    }
                }
                t.flush();
                Ok(value)
            });

            res.map_err(|e| {
                match e {
                    TransactionError::Abort(e) => e,
                    TransactionError::Storage(e) => e.into()
                }
            })
        }).await
    }

    pub fn get_or_insert_sync<B, S>(&self, key: B, def: S) -> crate::error::Result<S>
        where B: DbKey,
              S: Serialize + DeserializeOwned {
        let serialized = rmp_serde::to_vec(&def)?;
        let key = key.to_key();
        let csr = self.tree.compare_and_swap(key, None as Option<&[u8]>, Some(serialized));

        match csr {
            Ok(Ok(())) => {
                // this is the only case in which we actually changed something
                self.tree.flush()?;
                Ok(def)
            }
            Ok(Err(CompareAndSwapError { current, .. })) => Ok(rmp_serde::from_read(current.unwrap().as_ref())?),
            Err(e) => Err(e.into())
        }
    }

    pub async fn insert<B, S>(&self, key: B, val: S) -> crate::error::Result<()>
        where B: DbKey + Send + 'static,
              S: Serialize {
        let serialized = rmp_serde::to_vec(&val)?;
        self.do_async(move |c| {
            c.tree.insert(key.to_key(), serialized).map(|_| ())
        }).await?;
        Ok(())
    }

    pub async fn remove<B, D>(&self, key: B) -> crate::error::Result<Option<D>>
        where B: DbKey + Send + 'static,
              D: DeserializeOwned + Send + 'static {
        self.do_async(move |s| {
            let o = s.tree.remove(key.to_key())?;
            s.tree.flush()?;
            let r = o.map(|v| rmp_serde::from_read_ref(&v));
            Ok(r.flip()?)
        }).await
    }

    pub async fn contains_key<B>(&self, key: B) -> crate::error::Result<bool>
        where B: DbKey + Send + 'static {
        let exists = self.do_async(move |c| {
            c.tree.contains_key(key.to_key())
        }).await?;
        Ok(exists)
    }

    pub async fn get<B, D>(&self, key: B) -> crate::error::Result<Option<D>>
        where B: DbKey + Send + 'static,
              D: DeserializeOwned + Send + 'static {
        self.do_async(move |s| {
            let res: crate::error::Result<_> = try {
                s.tree.get(key.to_key())?
                    .map_or(Ok(None), |v| rmp_serde::from_read(v.as_ref()))?
            };
            res
        }).await
    }
}

#[derive(Clone)]
pub struct NamespacedDbContext {
    namespace: Cow<'static, str>,
    base: DbContext,
}


impl Deref for NamespacedDbContext {
    type Target = DbContext;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl NamespacedDbContext {
    pub async fn new<N>(guild: GuildId, namespace: N) -> crate::error::Result<Self> where N: Into<Cow<'static, str>> {
        let namespace = namespace.into();
        let base = DbContext::with_namespace(guild, &namespace)
            .await?;
        Ok(Self {
            namespace,
            base,
        })
    }

    pub async fn with_global_namespace(namespace: &'static str) -> crate::error::Result<Self> {
        let g = GuildId(0);
        Self::new(g, namespace).await
    }

    /// Workaround for backwards compat.
    pub async fn config_ctx(guild: GuildId) -> crate::error::Result<Self> {
        Self::new(guild, "").await
    }
}

pub trait DbKey {
    fn to_key(&self) -> Cow<[u8]>;
}

impl<T: AsRef<[u8]>> DbKey for T {
    fn to_key(&self) -> Cow<[u8]> {
        self.as_ref().into()
    }
}

#[macro_export]
macro_rules! impl_id_db_key {
    ($($key:path),+) => {
        $(
            impl $crate::db::DbKey for $key {
                fn to_key(&self) -> Cow<[u8]> {
                    self.0.0.to_be_bytes().to_vec().into()
                }
            }
        )+
    };
}


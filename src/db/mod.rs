use once_cell::sync::Lazy;
use std::path::{Path, PathBuf};
use std::io;
use serenity::model::id::GuildId;
use tokio::task;
use byteorder::ByteOrder;
use serde::{Serialize, Deserialize};
use serde::de::DeserializeOwned;
use sled::CompareAndSwapError;
use serenity::futures::StreamExt;
use std::borrow::Cow;
use std::ops::Deref;
use smallvec::SmallVec;

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
    tree: sled::Tree
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
            tree
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
            tree
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

    pub fn get_or_insert_sync<B, S>(&self, key: B, def: S) -> crate::error::Result<S>
        where B: DbKey,
              S: Serialize + DeserializeOwned {
        let serialized = rmp_serde::to_vec(&def)?;
        let key = key.to_key();
        let csr = self.tree.compare_and_swap(key, None as Option<&[u8]>, Some(serialized));

        match csr {
            Ok(Ok(())) => {self.tree.flush()?; Ok(def)}, // this is the only case in which we actually changed something
            Ok(Err(CompareAndSwapError{current, ..})) => Ok(rmp_serde::from_read(current.unwrap().as_ref())?),
            Err(e) => Err(e.into())
        }
    }

    pub async fn insert<B, S>(&self, key: B, val: S) -> crate::error::Result<()>
        where B: DbKey + Send + 'static,
              S: Serialize + DeserializeOwned {
        let serialized = rmp_serde::to_vec(&val)?;
        self.do_async(move |c| {
            c.tree.insert(key.to_key(), serialized).map(|_|())
        }).await?;
        Ok(())
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
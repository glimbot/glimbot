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

    pub async fn do_async<F, R>(&self, f: F) -> R where F: (FnOnce(Self) -> R) + Send + 'static, R: Send + 'static {
        let c = self.clone();
        task::spawn_blocking(move || f(c)).await.unwrap()
    }

    pub async fn do_async_in_place<F, R>(&self, f: F) -> R where F: FnOnce(Self) -> R {
        let c = self.clone();
        task::block_in_place(move || f(c))
    }

    pub async fn get_or_insert<B, S>(&self, key: B, def: S) -> crate::error::Result<S>
        where B: AsRef<[u8]> + Send + 'static,
              S: Serialize + DeserializeOwned + Send + 'static {
        self.do_async(move |s| {
            s.get_or_insert_sync(key, def)
        }).await
    }

    pub fn get_or_insert_sync<B, S>(&self, key: B, def: S) -> crate::error::Result<S>
        where B: AsRef<[u8]>,
              S: Serialize + DeserializeOwned {
        let serialized = bincode::serialize(&def)?;
        let csr = self.tree.compare_and_swap(key, None as Option<&[u8]>, Some(serialized));

        match csr {
            Ok(Ok(())) => {self.tree.flush()?; Ok(def)}, // this is the only case in which we actually changed something
            Ok(Err(CompareAndSwapError{current, ..})) => Ok(bincode::deserialize(&current.unwrap())?),
            Err(e) => Err(e.into())
        }
    }

    pub async fn get<B, D>(&self, key: B) -> crate::error::Result<Option<D>>
        where B: AsRef<[u8]> + Send + 'static,
              D: DeserializeOwned + Send + 'static {
        self.do_async(move |s| {
            let res: crate::error::Result<_> = try {
                s.tree.get(key)?
                    .map_or(Ok(None), |v| bincode::deserialize(&v))?
            };
            res
        }).await
    }
}
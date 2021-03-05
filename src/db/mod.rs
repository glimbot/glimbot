pub mod timed;

use once_cell::sync::Lazy;
use std::path::{Path, PathBuf};
use std::io;
use serenity::model::id::{GuildId, UserId, ChannelId, RoleId};
use tokio::task;
use byteorder::ByteOrder;
use serde::{Serialize, Deserialize};
use serde::de::DeserializeOwned;
use serenity::futures::StreamExt;
use std::borrow::Cow;
use std::ops::Deref;
use smallvec::SmallVec;
use crate::util::FlipResultExt;
use sqlx::pool::PoolConnection;
use sqlx::{Postgres, PgPool, Acquire};
use sqlx::types::Json;
use sqlx::postgres::PgConnectOptions;
use std::str::FromStr;
use sqlx::migrate::Migrator;
use tokio::sync::{Mutex, RwLock};
use std::hash::Hash;
use std::collections::HashMap;
use std::sync::Arc;
use serde_json::Value;
use futures::TryFuture;
use std::future::Future;
use std::sync::atomic::{AtomicU64, Ordering};

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

static MIGRATIONS: Migrator = sqlx::migrate!();

pub async fn create_pool() -> crate::error::Result<PgPool> {
    let db_url = std::env::var("DATABASE_URL")?;

    let pool = sqlx::PgPool::connect_with(
        PgConnectOptions::from_str(&db_url)?
            .application_name("glimbot")
    ).await?;

    info!("Running DB migrations if necessary.");
    MIGRATIONS.run(&pool).await?;
    Ok(pool)
}

#[derive(Clone)]
pub struct DbContext<'pool> {
    guild: GuildId,
    conn: &'pool PgPool,
}

impl DbContext<'_> {
    pub fn guild(&self) -> GuildId {
        self.guild
    }

    pub fn guild_as_i64(&self) -> i64 {
        self.guild.0 as i64
    }
}

#[derive(Debug)]
struct ConfigRow {
    value: serde_json::Value
}

pub type Arctex<T> = Arc<RwLock<T>>;
pub type ArctexMap<K, V> = Arctex<HashMap<K, V>>;
pub type CacheValue = Arctex<Option<serde_json::Value>>;

#[derive(Default)]
pub struct ConfigCache {
    cache: RwLock<HashMap<GuildId, ArctexMap<String, CacheValue>>>,
    cache_misses: AtomicU64,
    cache_accesses: AtomicU64,
}

pub static CONFIG_CACHE: Lazy<ConfigCache> = Lazy::new(|| Default::default());

pub struct CacheStats {
    pub accesses: u64,
    pub misses: u64
}

impl ConfigCache {
    pub async fn ensure_guild_cache(&self, gid: GuildId) -> ArctexMap<String, CacheValue> {
        let o = self.cache.read().await.get(&gid).cloned();

        if let Some(m) = o {
            return m;
        }

        let mut wg = self.cache.write().await;
        let e = wg.entry(gid);
        e.or_default().clone()
    }

    pub async fn entry(&self, gid: GuildId, key: impl ConfigKey) -> CacheValue {
        let k = key.to_key().into_owned();
        let guild_cache = self.ensure_guild_cache(gid).await;
        let potential = guild_cache.read().await.get(&k).cloned();

        let cv = match potential {
            None => {
                let mut wg = guild_cache.write().await;
                wg.entry(k).or_default().clone()
            }
            Some(v) => { v }
        };

        cv
    }

    pub fn statistics(&self) -> CacheStats {
        CacheStats {
            accesses: self.cache_accesses.load(Ordering::Relaxed),
            misses: self.cache_misses.load(Ordering::Relaxed)
        }
    }

    fn inc_access(&self) {
        self.cache_accesses.fetch_add(1, Ordering::Relaxed);
    }

    fn inc_miss(&self) {
        self.cache_misses.fetch_add(1, Ordering::Relaxed);
    }

    pub async fn get_or_insert_with<K, F, R>(&self, gid: GuildId, key: K, f: F)
                                             -> crate::error::Result<serde_json::Value>
        where K: ConfigKey,
              F: FnOnce() -> R,
              R: Future<Output=crate::error::Result<serde_json::Value>> {
        self.inc_access();
        let entry = self.entry(gid, key).await;
        let cur = entry.read().await.clone();
        if let Some(v) = cur {
            trace!("hit cache");
            return Ok(v);
        }

        self.inc_miss();
        let mut wg = entry.write().await;
        if let Some(v) = wg.as_ref() {
            trace!("hit cache; someone beat us to the punch");
            return Ok(v.clone());
        }

        trace!("cache miss");
        Ok(wg.get_or_insert(f().await?).clone())
    }

    pub async fn insert_with<K, Fut>(&self, gid: GuildId, key: K, f: Fut)
                                     -> crate::error::Result<()>
        where K: ConfigKey,
              Fut: Future<Output=crate::error::Result<serde_json::Value>> {
        self.inc_miss();
        self.inc_access();
        let entry = self.entry(gid, key).await;
        let mut wg = entry.write().await;
        trace!("updating cache");
        wg.insert(f.await?);
        Ok(())
    }

    pub async fn get<K, Fut>(&self, gid: GuildId, key: K, f: Fut) -> crate::error::Result<Option<serde_json::Value>>
        where K: ConfigKey,
              Fut: Future<Output=crate::error::Result<Option<serde_json::Value>>> {
        self.inc_access();
        let k = key.to_key().into_owned();
        let guild_cache = self.ensure_guild_cache(gid).await;
        let potential = guild_cache.read().await.get(&k).cloned();

        let cv = match potential {
            None => {
                trace!("first read");
                let mut wg = guild_cache.write().await;
                self.inc_miss();
                let e = wg.entry(k).or_default();
                let v = f.await?;
                if let Some(v) = &v {
                    let mut optg = e.write().await;
                    optg.insert(v.clone());
                }

                v
            }
            Some(v) => { v.read().await.clone() }
        };

        Ok(cv)
    }
}

impl DbContext<'_> {
    pub fn conn(&self) -> &PgPool {
        &self.conn
    }
}

impl<'pool> DbContext<'pool> {
    pub fn new<'b: 'pool>(pool: &'b PgPool, guild: GuildId) -> Self {
        Self {
            guild,
            conn: pool,
        }
    }

    #[instrument(level = "trace", skip(self, key, def), fields(g = % self.guild, k = % key.to_key()))]
    pub async fn get_or_insert<B, S>(&self, key: B, def: S) -> crate::error::Result<S>
        where B: ConfigKey,
              S: Serialize + DeserializeOwned {
        let v = CONFIG_CACHE.get_or_insert_with(self.guild, key.to_key(), || async {
            self.get_or_insert_uncached(key.to_key(), def).await
        }).await?;
        Ok(serde_json::from_value(v)?)
    }

    pub async fn get_or_insert_uncached<B, S>(&self, key: B, def: S) -> crate::error::Result<serde_json::Value>
        where B: ConfigKey,
              S: Serialize + DeserializeOwned {
        let v = serde_json::to_value(def)?;
        let key = key.to_key();

        let out: Option<serde_json::Value> = sqlx::query_scalar!(
            r#"
                SELECT res AS value FROM get_or_insert_config($1, $2, $3);
                "#,
                self.guild_as_i64(),
                key.as_ref(),
                v
        )
            .fetch_one(self.conn())
            .await?;
        Ok(out.expect("Failed to submit value to DB?"))
    }

    #[instrument(level = "trace", skip(self, key, val), fields(g = % self.guild, k = % key.to_key()))]
    pub async fn insert<B, S>(&self, key: B, val: S) -> crate::error::Result<()>
        where B: ConfigKey,
              S: Serialize {
        CONFIG_CACHE.insert_with(self.guild, key.to_key(), self.insert_uncached(key.to_key(), val)).await
    }

    pub async fn insert_uncached<B, S>(&self, key: B, val: S) -> crate::error::Result<serde_json::Value>
        where B: ConfigKey,
              S: Serialize {
        let key = key.to_key();
        let v = serde_json::to_value(val)?;

        sqlx::query!(
            r#"
            INSERT INTO config_values (guild, name, value)
            VALUES ($1, $2, $3)
            ON CONFLICT (guild, name) DO UPDATE
                SET value = EXCLUDED.value;
            "#,
            self.guild_as_i64(),
            key.as_ref(),
            &v
        )
            .execute(self.conn())
            .await
            .map(|_| ())?;
        Ok(v)
    }

    #[instrument(level = "trace", skip(self, key), fields(g = % self.guild, k = % key.to_key()))]
    pub async fn get<B, D>(&self, key: B) -> crate::error::Result<Option<D>>
        where B: ConfigKey,
              D: DeserializeOwned {
        let v = CONFIG_CACHE.get(self.guild,
                                 key.to_key(),
                                 self.get_uncached(key.to_key())).await?;
        Ok(v.map(|v| serde_json::from_value(v)).flip()?)
    }

    pub async fn get_uncached<B>(&self, key: B) -> crate::error::Result<Option<serde_json::Value>>
        where B: ConfigKey {
        let key = key.to_key();
        let o: Option<ConfigRow> = sqlx::query_as!(
            ConfigRow,
            r#"
            SELECT value FROM config_values WHERE guild = $1 AND name = $2;
            "#,
            self.guild_as_i64(),
            key.as_ref(),
        )
            .fetch_optional(self.conn())
            .await?;
        Ok(o.map(|c| c.value))
    }
}

pub trait ConfigKey {
    fn to_key(&self) -> Cow<str>;
}

impl<T: AsRef<str>> ConfigKey for T {
    fn to_key(&self) -> Cow<str> {
        self.as_ref().into()
    }
}
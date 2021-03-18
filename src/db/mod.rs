//! Contains abstractions over the persistent store connections for glimbot.
//! Currently, glimbot relies on a PostgreSQL server for its persistent store.

use std::borrow::Cow;
use std::collections::HashMap;
use std::future::Future;
use std::io;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use once_cell::sync::Lazy;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serenity::model::id::GuildId;
use sqlx::migrate::Migrator;
use sqlx::PgPool;
use sqlx::postgres::PgConnectOptions;
use tokio::sync::RwLock;

use crate::util::FlipResultExt;
use std::any::Any;
use crate::dispatch::config::ValueType;
use downcast_rs::DowncastSync;
use downcast_rs::impl_downcast;
use dyn_clone::DynClone;

pub mod timed;

/// Gets the path to the default data folder.
pub fn default_data_folder() -> PathBuf {
    #[doc(hidden)]
    static DEFAULT_PATH: Lazy<PathBuf> = Lazy::new(|| {
        let mut base = dirs::data_local_dir().unwrap();
        base.push("glimbot");
        base
    });

    DEFAULT_PATH.clone()
}

/// Gets the path to the default data folder, creating it if necessary.
pub fn ensure_data_folder() -> io::Result<PathBuf> {
    let dir = std::env::var("GLIMBOT_DIR")
        .map(|s| shellexpand::full(&s).expect("Failed while expanding directory").to_string())
        .map(PathBuf::from)
        .unwrap_or_else(|_| default_data_folder());

    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// The SQL migrations to be automatically applied on startup.
static MIGRATIONS: Migrator = sqlx::migrate!();

/// Create the database connection pool. This will eagerly spawn a single connection,
/// and spawn more as contention occurs.
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

/// A thin wrapper around a DB pool and the guild which queries should target.
#[derive(Clone)]
pub struct DbContext<'pool> {
    /// The guild that queries will target.
    guild: GuildId,
    /// A reference to the connection pool. We don't take a connection because we can usually
    /// significantly reduce contention on the connections by only holding one for the duration
    /// of the query.
    conn: &'pool PgPool,
}

impl DbContext<'_> {
    /// Gets the guild this context refers to.
    pub fn guild(&self) -> GuildId {
        self.guild
    }

    /// Gets the guild this context refers to as an i64.
    pub fn guild_as_i64(&self) -> i64 {
        self.guild.0 as i64
    }
}

#[doc(hidden)]
#[derive(Debug)]
struct ConfigRow {
    value: serde_json::Value
}

/// An arc containing a read-write locked type.
pub type Arctex<T> = Arc<RwLock<T>>;
/// An arctex containing a hashmap.
pub type ArctexMap<K, V> = Arctex<HashMap<K, V>>;
/// The value of a cache member.
pub type CacheValue = Arctex<Option<CVal>>;
/// The actual contents of a cache member
pub type CVal = Arc<dyn Cacheable>;

/// Traits for a cacheable type
pub trait Cacheable: Any + Send + Sync + DowncastSync + DynClone {}
impl_downcast!(sync Cacheable);
dyn_clone::clone_trait_object!(Cacheable);
impl<T> Cacheable for T where T: Any + Send + Sync + DowncastSync + DynClone {}


/// The global cache for glimbot configurations.
#[derive(Default)]
pub struct ConfigCache {
    /// The backing cache
    cache: RwLock<HashMap<GuildId, ArctexMap<String, CacheValue>>>,
    /// The number of times we had to query the DB backend.
    cache_misses: AtomicU64,
    /// The number of times the cache was accessed.
    cache_accesses: AtomicU64,
}

/// The global config cache.
pub static CONFIG_CACHE: Lazy<ConfigCache> = Lazy::new(Default::default);

/// Represents the values of the cache statistics.
pub struct CacheStats {
    /// Number of times the cache was accessed.
    pub accesses: u64,
    /// Number of times we had to access the DB
    pub misses: u64,
}

impl_err!(BadCast, "Cache contained a mismatched type.", false);

impl ConfigCache {
    /// Ensures that a cache map exists for a specific guild.
    pub async fn ensure_guild_cache(&self, gid: GuildId) -> ArctexMap<String, CacheValue> {
        let o = self.cache.read().await.get(&gid).cloned();

        if let Some(m) = o {
            return m;
        }

        let mut wg = self.cache.write().await;
        let e = wg.entry(gid);
        e.or_default().clone()
    }

    /// Gets the entry for the specified guild and key for update or retrieval.
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

    /// Gets a view of the current cache statistics. May or may not be accurate.
    pub fn statistics(&self) -> CacheStats {
        CacheStats {
            accesses: self.cache_accesses.load(Ordering::Relaxed),
            misses: self.cache_misses.load(Ordering::Relaxed),
        }
    }

    /// Track an access
    fn inc_access(&self) {
        self.cache_accesses.fetch_add(1, Ordering::Relaxed);
    }

    /// Track a miss
    fn inc_miss(&self) {
        self.cache_misses.fetch_add(1, Ordering::Relaxed);
    }

    /// Retrieves or inserts a value from the guild cache, using the given future.
    pub async fn get_or_insert_with<K, Fut, R>(&self, gid: GuildId, key: K, f: Fut)
                                               -> crate::error::Result<R>
        where K: ConfigKey,
              Fut: Future<Output=crate::error::Result<R>>,
              R: Cacheable + Sized + Clone {
        self.inc_access();
        let entry = self.entry(gid, key).await;
        let cur = entry.read().await.clone();
        if let Some(v) = cur {
            trace!("hit cache");
            return v.downcast_ref::<R>()
                .ok_or_else(|| BadCast.into())
                .map(|r: &R| r.clone());
        }

        self.inc_miss();
        let mut wg = entry.write().await;
        if let Some(v) = wg.as_ref() {
            trace!("hit cache; someone beat us to the punch");
            let v = v.clone();
            std::mem::drop(wg);
            return v.downcast_ref::<R>()
                .ok_or_else(|| BadCast.into())
                .map(|r: &R| r.clone());
        }

        trace!("cache miss");
        let ins = f.await?;
        wg.insert(Arc::new(ins.clone()));
        Ok(ins)
    }

    /// Inserts a value into the cache from the given future.
    pub async fn insert_with<K, Fut, R>(&self, gid: GuildId, key: K, f: Fut)
                                        -> crate::error::Result<()>
        where K: ConfigKey,
              Fut: Future<Output=crate::error::Result<R>>,
              R: Cacheable + Sized + Clone {
        self.inc_miss();
        self.inc_access();
        let entry = self.entry(gid, key).await;
        let mut wg = entry.write().await;
        trace!("updating cache");
        let ins = f.await?;
        wg.insert(Arc::new(ins));
        Ok(())
    }

    /// Retrieves a value (which may not be set) from the given future or the cache.
    pub async fn get<K, Fut, R>(&self, gid: GuildId, key: K, f: Fut) -> crate::error::Result<Option<R>>
        where K: ConfigKey,
              Fut: Future<Output=crate::error::Result<Option<R>>>,
              R: Cacheable + Sized + Clone {
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
                    optg.insert(Arc::new(v.clone()));
                }

                v
            }
            Some(v) => {
                let o = v.read().await.clone();
                o.map(|c| c.downcast_ref::<R>()
                    .cloned()
                    .ok_or(BadCast))
                    .flip()?
            }
        };

        Ok(cv)
    }
}

impl DbContext<'_> {
    /// Retrieves a reference to the underlying connection pool.
    pub fn conn(&self) -> &PgPool {
        &self.conn
    }
}

impl<'pool> DbContext<'pool> {
    /// Creates a guild-focused context wrapping around a connection pool.
    pub fn new<'b: 'pool>(pool: &'b PgPool, guild: GuildId) -> Self {
        Self {
            guild,
            conn: pool,
        }
    }

    /// Retrieves or inserts a value for the guild config.
    #[instrument(level = "trace", skip(self, key, def), fields(g = % self.guild, k = % key.to_key()))]
    pub async fn get_or_insert<B, S>(&self, key: B, def: S) -> crate::error::Result<S>
        where B: ConfigKey,
              S: Cacheable + Sized + Clone + Serialize + DeserializeOwned {
        CONFIG_CACHE.get_or_insert_with(self.guild, key.to_key(),
            self.get_or_insert_uncached(key.to_key(), def)
        ).await
    }

    /// Retrieves or inserts a value for the guild config. This version always hits the DB.
    async fn get_or_insert_uncached<B, S>(&self, key: B, def: S) -> crate::error::Result<S>
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
        Ok(serde_json::from_value(out.expect("Failed to submit value to DB?"))?)
    }

    /// Inserts a value into the guild config. This version will hit the cache in addition to the database.
    #[instrument(level = "trace", skip(self, key, val), fields(g = % self.guild, k = % key.to_key()))]
    pub async fn insert<B, S>(&self, key: B, val: S) -> crate::error::Result<()>
        where B: ConfigKey,
              S: Cacheable + Clone + Sized + Serialize {
        CONFIG_CACHE.insert_with(self.guild, key.to_key(), self.insert_uncached(key.to_key(), val)).await
    }

    /// Inserts a value into the guild config, and will bypass the cache. This should be avoided to avoid stale reads from the cache.
    async fn insert_uncached<B, S>(&self, key: B, val: S) -> crate::error::Result<S>
        where B: ConfigKey,
              S: Serialize + Clone + Sized {
        let key = key.to_key();
        let v = serde_json::to_value(&val)?;

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
        Ok(val)
    }

    /// Hits the cache to retrieve a config value, hitting the DB if necessary.
    #[instrument(level = "trace", skip(self, key), fields(g = % self.guild, k = % key.to_key()))]
    pub async fn get<B, D>(&self, key: B) -> crate::error::Result<Option<D>>
        where B: ConfigKey,
              D: Cacheable + Sized + Clone + DeserializeOwned {
        CONFIG_CACHE.get(self.guild,
                         key.to_key(),
                         self.get_uncached(key.to_key())).await
    }

    /// Grabs a value from the database.
    async fn get_uncached<B, D>(&self, key: B) -> crate::error::Result<Option<D>>
        where B: ConfigKey,
              D: DeserializeOwned {
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
        Ok(o.map(|c| c.value).map(serde_json::from_value).flip()?)
    }
}

/// Trait for configuration keys to implement.
pub trait ConfigKey {
    /// Should return this key as a view on a string.
    fn to_key(&self) -> Cow<str>;
}

impl<T: AsRef<str>> ConfigKey for T {
    fn to_key(&self) -> Cow<str> {
        self.as_ref().into()
    }
}
//! Contains abstractions over the persistent store connections for glimbot.
//! Currently, glimbot relies on a PostgreSQL server for its persistent store.

use std::borrow::Cow;
use std::collections::HashMap;
use std::future::Future;
use std::{io, path};
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
use dashmap::DashMap;
use crate::dispatch::Dispatch;
use arc_swap::ArcSwap;
use crate::db::cache::TimedCache;
use futures::{TryFutureExt, FutureExt};

pub mod timed;
pub mod cache;

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
    conn: &'pool Dispatch,
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
pub type ArctexMap<K, V> = Arc<DashMap<K, V>>;
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
    cache: HashMap<String, TimedCache<CVal>>,
    /// The number of times we had to query the DB backend.
    cache_misses: AtomicU64,
    /// The number of times the cache was accessed.
    cache_accesses: AtomicU64,
}

/// Represents the values of the cache statistics.
pub struct CacheStats {
    /// Number of times the cache was accessed.
    pub accesses: u64,
    /// Number of times we had to access the DB
    pub misses: u64,
}

impl_err!(BadCast, "Cache contained a mismatched type.", false);

impl ConfigCache {

    /// Gets a view of the current cache statistics. May or may not be accurate.
    pub fn statistics(&self) -> CacheStats {
        CacheStats {
            accesses: self.cache_accesses.load(Ordering::Relaxed),
            misses: self.cache_misses.load(Ordering::Relaxed),
        }
    }

    pub fn add_key(&mut self, s: impl Into<String>) {
        self.cache.insert(s.into(), TimedCache::new(std::time::Duration::from_secs(3600)));
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
                                               -> crate::error::Result<Arc<R>>
        where K: ConfigKey,
              Fut: Future<Output=crate::error::Result<R>>,
              R: Cacheable + Sized + Clone {

        self.inc_access();
        let f = f.and_then(|r: R| async { self.inc_miss();
            let cv: CVal = Arc::new(r);
            Ok(cv) });
        let cv = self.cache.get(key.to_key().as_ref())
            .expect("Unexpected config key")
            .get_or_insert_with(gid, f)
            .await?;
        Arc::clone(cv.as_ref()).downcast_arc::<R>().map_err(|_| BadCast.into())
    }

    /// Inserts a value into the cache from the given future.
    pub async fn insert_with<K, Fut, R>(&self, gid: GuildId, key: K, f: Fut)
                                        -> crate::error::Result<()>
        where K: ConfigKey,
              Fut: Future<Output=crate::error::Result<R>>,
              R: Cacheable + Sized + Clone {
        self.inc_miss();
        self.inc_access();
        trace!("updating cache");
        let ins = f.await?;
        self.cache.get(key.to_key().as_ref())
            .expect("Unexpected config key")
            .insert(gid, Arc::new(ins));
        Ok(())
    }

    /// Retrieves a value (which may not be set) from the given future or the cache.
    pub async fn get<K, Fut, R>(&self, gid: GuildId, key: K, f: Fut) -> crate::error::Result<Option<Arc<R>>>
        where K: ConfigKey,
              Fut: Future<Output=crate::error::Result<Option<R>>>,
              R: Cacheable + Sized + Clone {
        self.inc_access();
        let val_cache = self.cache.get(key.to_key().as_ref()).expect("Unexpected config key").get(gid);
        if let Some(v) = val_cache {
            Arc::clone(v.as_ref()).downcast_arc::<R>().map_err(|_| BadCast.into()).map(Some)
        } else if let Some(v) = f.await? {
            self.get_or_insert_with(gid, key, async { Ok(v) }).await.map(Some)
        } else {
            Ok(None)
        }
    }
}

impl DbContext<'_> {
    /// Retrieves a reference to the underlying connection pool.
    pub fn conn(&self) -> &PgPool {
        self.conn.pool()
    }
}

impl<'pool> DbContext<'pool> {
    /// Creates a guild-focused context wrapping around a connection pool.
    pub fn new<'b: 'pool>(pool: &'b Dispatch, guild: GuildId) -> Self {
        Self {
            guild,
            conn: pool,
        }
    }

    /// Retrieves or inserts a value for the guild config.
    #[instrument(level = "trace", skip(self, key, def), fields(g = % self.guild, k = % key.to_key()))]
    pub async fn get_or_insert_with<B, S, F>(&self, key: B, def: F) -> crate::error::Result<Arc<S>>
        where B: ConfigKey,
              S: Cacheable + Sized + Clone + Serialize + DeserializeOwned,
              F: (Fn() -> S) + Send + Sync {
        self.conn.config_cache()
            .get_or_insert_with(self.guild, key.to_key(),
            self.get_or_insert_uncached_with(key.to_key(), def)
        ).await
    }

    /// Retrieves or inserts a value for the guild config. This version always hits the DB.
    async fn get_or_insert_uncached_with<B, S, F>(&self, key: B, def: F) -> crate::error::Result<S>
        where B: ConfigKey,
              S: Serialize + DeserializeOwned,
              F: (Fn() -> S) + Send + Sync {
        let v = serde_json::to_value(def())?;
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
        self.conn.config_cache().insert_with(self.guild, key.to_key(), self.insert_uncached(key.to_key(), val)).await
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
    pub async fn get<B, D>(&self, key: B) -> crate::error::Result<Option<Arc<D>>>
        where B: ConfigKey,
              D: Cacheable + Sized + Clone + DeserializeOwned {
        self.conn.config_cache().get(self.guild,
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
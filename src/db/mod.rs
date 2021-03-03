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

    pub async fn get_or_insert<B, S>(&self, key: B, def: S) -> crate::error::Result<S>
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

    pub async fn insert<B, S>(&self, key: B, val: S) -> crate::error::Result<()>
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
            v
        )
            .execute(self.conn())
            .await
            .map(|_| ())?;
        Ok(())
    }

    pub async fn get<B, D>(&self, key: B) -> crate::error::Result<Option<D>>
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

pub trait ConfigKey {
    fn to_key(&self) -> Cow<str>;
}

impl<T: AsRef<str>> ConfigKey for T {
    fn to_key(&self) -> Cow<str> {
        self.as_ref().into()
    }
}
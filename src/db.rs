use std::path::{PathBuf, Path};
use serenity::model::prelude::GuildId;
use std::io;
use rusqlite::{Connection, OpenFlags, Statement, NO_PARAMS, TransactionBehavior, Transaction, ToSql};
use std::sync::Arc;
use parking_lot::{Mutex, RwLock};
use crate::data::Resources;
use failure::Fail;
use serenity::model::id::UserId;
use chrono::{NaiveDateTime, Utc, DateTime};
use once_cell::sync::Lazy;
use std::borrow::Cow;
use crate::util::string_from_cow;
use itertools::Itertools;
use failure::_core::cmp::Ordering;

#[derive(thiserror::Error, Debug)]
pub enum DatabaseError {
    #[error("An I/O error occurred: {0}")]
    IOError(#[from] io::Error),
    #[error("A SQL error occurred: {0}")]
    SQLError(#[from] rusqlite::Error),
    #[error("Database from a newer version of glimbot.")]
    TooNew,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Ord)]
pub enum DatabaseVersion {
    Uninitialized,
    Version(u32),
}

impl PartialOrd for DatabaseVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match self {
            DatabaseVersion::Uninitialized => {
                match other {
                    DatabaseVersion::Uninitialized => Some(Ordering::Equal),
                    DatabaseVersion::Version(_) => Some(Ordering::Less),
                }
            }
            DatabaseVersion::Version(v) => {
                match other {
                    DatabaseVersion::Uninitialized => Some(Ordering::Greater),
                    DatabaseVersion::Version(ov) => v.partial_cmp(ov)
                }
            }
        }
    }
}

impl DatabaseVersion {
    pub const INITIALIZE_MASK: u32 = (1 << 31);
    pub const VERSION_MASK: u32 = std::u32::MAX & (!Self::INITIALIZE_MASK);

    pub fn next_migration(&self) -> u32 {
        match self {
            DatabaseVersion::Uninitialized => { 0 }
            DatabaseVersion::Version(v) => { v + 1 }
        }
    }
}

impl From<i32> for DatabaseVersion {
    fn from(i: i32) -> Self {
        let i = i as u32;
        if i & Self::INITIALIZE_MASK == 0 {
            DatabaseVersion::Uninitialized
        } else {
            DatabaseVersion::Version(i & Self::VERSION_MASK)
        }
    }
}

impl From<u32> for DatabaseVersion {
    fn from(i: u32) -> Self {
        let i = i as i32;
        Self::from(i)
    }
}

impl From<DatabaseVersion> for u32 {
    fn from(v: DatabaseVersion) -> Self {
        match v {
            DatabaseVersion::Uninitialized => { DatabaseVersion::INITIALIZE_MASK }
            DatabaseVersion::Version(v) => { v | DatabaseVersion::INITIALIZE_MASK }
        }
    }
}

impl From<DatabaseVersion> for i32 {
    fn from(v: DatabaseVersion) -> Self {
        let o: u32 = v.into();
        o as i32
    }
}

#[derive(rust_embed::RustEmbed)]
#[folder = "$CARGO_MANIFEST_DIR/migrations/"]
pub struct Migrations;

pub type Result<T> = std::result::Result<T, DatabaseError>;

pub fn new_conn(p: impl AsRef<Path>) -> Result<rusqlite::Connection> {
    let db = Connection::open_with_flags(
        p,
        OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_CREATE
            | OpenFlags::SQLITE_OPEN_NO_MUTEX
            | OpenFlags::SQLITE_OPEN_URI
            | OpenFlags::SQLITE_OPEN_SHARED_CACHE,
    )?;

    static PRELUDE_SQL: Lazy<String> = Lazy::new(
        || Resources::get("conn_prelude.sql")
            .map(string_from_cow).unwrap()
    );

    // Do some connection setup.
    db.execute_batch(
        &PRELUDE_SQL
    )?;

    Ok(db)
}


pub fn ensure_guild_db(data_dir: impl Into<PathBuf>, g: GuildId) -> Result<rusqlite::Connection> {
    let mut db_name = data_dir.into();
    db_name.push(format!("{}.sqlite3", g));
    new_conn(&db_name)
}

pub static MIGRATIONS: Lazy<Vec<String>> = Lazy::new(
    || Migrations::iter()
        .map(String::from)
        .filter(|s: &String| s.ends_with("up.sql"))
        .sorted()
        .collect()
);

pub static DB_VERSION: Lazy<DatabaseVersion> = Lazy::new(
    || DatabaseVersion::Version((MIGRATIONS.len().saturating_sub(1)) as u32)
);

pub fn run_migrations(conn: &mut Connection, until: Option<DatabaseVersion>) -> Result<()> {
    // Migrations should be run offline.

    let until = until.unwrap_or(*DB_VERSION).min(*DB_VERSION);

    let trans = conn.transaction_with_behavior(TransactionBehavior::Exclusive)?; // TRAAAAAAAAAAAAAANS
    let ver = trans.query_row(
        "PRAGMA user_version;",
        NO_PARAMS,
        |r| r.get(0),
    ).map(|i: u32| DatabaseVersion::from(i))?;

    if ver > *DB_VERSION {
        return Err(DatabaseError::TooNew);
    } else if ver == *DB_VERSION {
        return Ok(());
    }

    for idx in ver.next_migration()..until.next_migration() {
        run_migration(idx, &trans)?;
    }

    trans.commit()?;

    Ok(())
}

fn run_migration(idx: u32, t: &Transaction) -> Result<()> {
    let migration = &MIGRATIONS[idx as usize];
    info!("Applying migration {}...", migration);

    let mig_sql = Migrations::get(migration).map(string_from_cow).unwrap();
    t.execute_batch(
        &mig_sql
    ).map_err(DatabaseError::from)?;

    let new_ver = DatabaseVersion::Version(idx);
    t.execute(
        &format!("PRAGMA user_version = {}", i32::from(new_ver)),
        NO_PARAMS,
    ).map_err(DatabaseError::from)
        .map(|_| ())
}

pub fn user_pressure(since: &DateTime<Utc>, u: UserId, conn: &Connection) -> Result<i64> {
    let uid = u.0 as i64;
    let ts = since.naive_utc().timestamp();

    static PRESSURE_SQL: Lazy<String> = Lazy::new(
        || Resources::get("user_pressure.sql")
            .map(string_from_cow).unwrap()
    );

    conn.query_row_named(
        &PRESSURE_SQL,
        named_params! {
        ":uid": uid,
        ":since": ts
    },
        |r| r.get(0),
    ).map_err(DatabaseError::from)
}

pub fn get_db_version(conn: &Connection) -> Result<DatabaseVersion> {
    let v = conn.query_row(
        "PRAGMA user_version;",
        NO_PARAMS,
        |r| r.get(0).map(|i: i32| DatabaseVersion::from(i))
    )?;

    Ok(v)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempdir::TempDir;

    #[test]
    pub fn test_migration_up() {
        let dummy_dir = TempDir::new("migrations").unwrap();
        let mut dummy_conn = ensure_guild_db(dummy_dir.as_ref(), GuildId::from(std::u64::MAX)).unwrap();
        run_migrations(&mut dummy_conn, None).unwrap();
        assert_eq!(get_db_version(&dummy_conn).unwrap(), DatabaseVersion::Version(0))
    }
}
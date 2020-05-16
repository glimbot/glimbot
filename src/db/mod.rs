//  Glimbot - A Discord anti-spam and administration bot.
//  Copyright (C) 2020 Nick Samson

//  This program is free software: you can redistribute it and/or modify
//  it under the terms of the GNU General Public License as published by
//  the Free Software Foundation, either version 3 of the License, or
//  (at your option) any later version.

//  This program is distributed in the hope that it will be useful,
//  but WITHOUT ANY WARRANTY; without even the implied warranty of
//  MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
//  GNU General Public License for more details.

//  You should have received a copy of the GNU General Public License
//  along with this program.  If not, see <https://www.gnu.org/licenses/>.

//! This module contains functionality related to the databases created for each guild.

use std::path::{PathBuf, Path};
use serenity::model::prelude::GuildId;
use std::io;
use rusqlite::{Connection, OpenFlags, NO_PARAMS, TransactionBehavior, Transaction,};
use crate::data::{Resources, Migrations, data_folder};
use serenity::model::id::UserId;
use chrono::{Utc, DateTime};
use once_cell::sync::Lazy;
use crate::util::string_from_cow;
use itertools::Itertools;
use std::cmp::Ordering;
use std::num::ParseIntError;
use std::fmt::Display;

pub mod args;
pub mod cache;

/// Errors related to database I/O
#[derive(thiserror::Error, Debug)]
pub enum DatabaseError {
    /// Created if an underlying file I/O issue occurs, e.g. ENOENT
    #[error("An I/O error occurred: {0}")]
    IOError(#[from] io::Error),
    /// Created if the SQL failed, probably a constraint violation in this case.
    #[error("A SQL error occurred: {0}")]
    SQLError(#[from] rusqlite::Error),
    /// The database that operations were attempted on is too new for this version of glimbot to open.
    /// If you see this error, you will need a newer version of Glimbot to be able to reverse the migration.
    #[error("Database from a newer version of glimbot.")]
    TooNew,
}

impl DatabaseError {
    /// True if the internal error represents no rows being returned
    pub fn no_rows_returned(&self) -> bool {
        match self {
            DatabaseError::SQLError(rusqlite::Error::QueryReturnedNoRows) => true,
            _ => false
        }
    }
}

impl BotError for DatabaseError {
    fn is_user_error(&self) -> bool {
        false
    }
}

impl From<DatabaseError> for crate::modules::commands::Error {
    fn from(e: DatabaseError) -> Self {
        crate::modules::commands::Error::RuntimeFailure(e.into())
    }
}

/// A struct representing the value of the user_version field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Ord)]
pub enum DatabaseVersion {
    /// The database is uninitialized (INITIALIZE_MASK not set)
    Uninitialized,
    /// The version of the database. Version numbers start at 0 and increment with each new migration.
    Version(u32),
}

mod guild_conn;
pub use guild_conn::GuildConn;
use crate::error::BotError;

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

impl Display for DatabaseVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}",
            match self {
                DatabaseVersion::Uninitialized => {"uninitialized".to_string()},
                DatabaseVersion::Version(v) => v.to_string(),
            }
        )
    }
}


impl DatabaseVersion {
    /// Bitmask for initialization bit. If set, the db is assumed to be initialized.
    pub const INITIALIZE_MASK: u32 = (1 << 31);
    /// Bitmask for version num. See enum docs for more info.
    pub const VERSION_MASK: u32 = std::u32::MAX & (!Self::INITIALIZE_MASK);

    /// Returns the version number of the migration that would follow this version.
    pub fn next_migration(&self) -> u32 {
        match self {
            DatabaseVersion::Uninitialized => { 0 }
            DatabaseVersion::Version(v) => { v + 1 }
        }
    }

    /// The version number if initialized, otherwise None.
    pub fn version(&self) -> Option<u32> {
        match self {
            DatabaseVersion::Uninitialized => {None},
            DatabaseVersion::Version(v) => {Some(*v)},
        }
    }

    /// The index into the downgrades list to revert this version of the database.
    pub fn next_revert(&self) -> Option<u32> {
        self.version()
    }

    /// The version the database would be if a reversion were applied.
    pub fn next_downgrade_ver(&self) -> Option<DatabaseVersion> {
        match self {
            DatabaseVersion::Uninitialized => {None},
            DatabaseVersion::Version(0) => {Some(DatabaseVersion::Uninitialized)},
            DatabaseVersion::Version(v) => {Some(DatabaseVersion::Version(v-1))}
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
            DatabaseVersion::Uninitialized => { 0 }
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

impl std::convert::TryFrom<&str> for DatabaseVersion {
    type Error = ParseIntError;

    fn try_from(value: &str) -> std::result::Result<Self, Self::Error> {
        value.parse::<u32>().map(DatabaseVersion::from)
    }
}

impl std::convert::TryFrom<String> for DatabaseVersion {
    type Error = ParseIntError;

    fn try_from(value: String) -> std::result::Result<Self, Self::Error> {
        value.parse::<u32>().map(DatabaseVersion::from)
    }
}


///
pub type Result<T> = std::result::Result<T, DatabaseError>;

/// Creates a connection to a guild database and runs the prelude statements through.
/// Prelude statements set up the busy handler, cache settings, WAL mode, etc.
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

/// Opens or creates a guild database in the specified directory.
/// Does not initialize the guild! Call init_guild_db to ensure initialization is complete.
pub fn ensure_guild_db(data_dir: impl Into<PathBuf>, g: GuildId) -> Result<rusqlite::Connection> {
    trace!("Encountered guild {}", g);
    let mut db_name = data_dir.into();
    db_name.push(format!("{}.sqlite3", g));
    let conn = new_conn(&db_name)?;
    Ok(conn)
}

/// Creates a guild database inside the data folder. See [ensure_guild_db] for more info.
pub fn ensure_guild_db_in_data_dir(g: GuildId) -> Result<rusqlite::Connection> {
    let data_dir = data_folder();
    ensure_guild_db(data_dir, g)
}

/// Updates a guild database to the latest version, then ensures the guild configuration is initialized.
pub fn init_guild_db(conn: &mut Connection) -> Result<()> {
    upgrade(conn, None)?;
    conn.execute(
        "INSERT OR IGNORE INTO guild_config DEFAULT VALUES;",
        NO_PARAMS
    )?;
    Ok(())
}

/// The names of the migration files for upgrading guild databases, sorted in ascending order of version.
/// Apply in order to upgrade a database.
/// Each migration is idempotent.
pub static MIGRATIONS: Lazy<Vec<String>> = Lazy::new(
    || Migrations::iter()
        .map(String::from)
        .filter(|s: &String| s.ends_with("up.sql"))
        .sorted()
        .collect()
);

/// The names of migration files for downgrading guild databases, sorted in ascending order of version.
/// Apply in reverse order to downgrade a database.
/// Each migration is idempotent.
pub static REVERTS: Lazy<Vec<String>> = Lazy::new(
    || Migrations::iter()
        .map(String::from)
        .filter(|s: &String| s.ends_with("down.sql"))
        .sorted()
        .collect()
);

/// The latest version of guild databases this build of Glimbot supports.
pub static DB_VERSION: Lazy<DatabaseVersion> = Lazy::new(
    || DatabaseVersion::Version((MIGRATIONS.len().saturating_sub(1)) as u32)
);

/// [DB_VERSION] as a String.
pub static DB_VERSION_STRING: Lazy<String> = Lazy::new(
    || DB_VERSION.to_string()
);


/// Upgrades a database connection to the latest version or the version specified in `until`.
/// This will either apply all available upgrades between the two versions or none of them.
pub fn upgrade(conn: &mut Connection, until: Option<DatabaseVersion>) -> Result<()> {
    // Migrations should be run offline.

    let until = until.unwrap_or(*DB_VERSION).min(*DB_VERSION);
    // Check before we have to grab an exclusive lock.
    let ver = get_db_version(conn)?;

    if ver > *DB_VERSION {
        return Err(DatabaseError::TooNew);
    } else if ver == *DB_VERSION {
        return Ok(());
    }

    let trans = conn.transaction_with_behavior(TransactionBehavior::Exclusive)?; // TRAAAAAAAAAAAAAANS
    let ver = trans.query_row(
        "PRAGMA user_version;",
        NO_PARAMS,
        |r| r.get(0),
    ).map(|i: i32| DatabaseVersion::from(i))?;

    if ver > *DB_VERSION {
        return Err(DatabaseError::TooNew);
    } else if ver == *DB_VERSION {
        return Ok(());
    }

    for idx in ver.next_migration()..until.next_migration() {
        run_upgrade(idx, &trans)?;
    }

    trans.commit()?;

    Ok(())
}

/// Migrates the connected database to the specified version, up or down.
/// Prefer using [upgrade] or [downgrade] directly.
pub fn migrate_to(conn: &mut Connection, when: DatabaseVersion) -> Result<()> {
    let ver = get_db_version(conn)?;
    if ver < when {
        upgrade(conn, Some(when))?;
    } else {
        downgrade(conn, when)?;
    }
    Ok(())
}

/// Downgrades the connected database to the specified version.
pub fn downgrade(conn: &mut Connection, when: DatabaseVersion) -> Result<()> {

    let when = if when == DatabaseVersion::Uninitialized {
        DatabaseVersion::Version(0)
    } else {
        when
    };

    let trans = conn.transaction_with_behavior(TransactionBehavior::Exclusive)?; // TRAAAAAAAAAAAAAANS
    let mut ver = trans.query_row(
        "PRAGMA user_version;",
        NO_PARAMS,
        |r| r.get(0),
    ).map(|i: i32| DatabaseVersion::from(i))?;

    if ver > *DB_VERSION {
        return Err(DatabaseError::TooNew);
    } else if ver < when || ver == DatabaseVersion::Uninitialized {
        return Ok(())
    }

    while ver >= when {
        trace!("Downgrading from {}", ver);
        let idx = ver.next_revert().unwrap();
        run_downgrade(idx, &trans)?;
        ver = if let Some(v) = ver.next_downgrade_ver() {
            v
        } else {
            break;
        };
    }

    trans.commit()?;

    Ok(())
}

/// Applies a single downgrade to the database connected to the specified [Transaction].
fn run_upgrade(idx: u32, t: &Transaction) -> Result<()> {
    let migration = &MIGRATIONS[idx as usize];
    debug!("Applying migration {}...", migration);

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

fn run_downgrade(idx: u32, t: &Transaction) -> Result<()> {
    let migration = &REVERTS[idx as usize];
    debug!("Applying downgrade {}...", migration);

    let mig_sql = Migrations::get(migration).map(string_from_cow).unwrap();
    t.execute_batch(
        &mig_sql
    ).map_err(DatabaseError::from)?;


    let new_ver = if idx > 0 {
        DatabaseVersion::Version(idx - 1)
    } else {
        DatabaseVersion::Uninitialized
    };
    t.execute(
        &format!("PRAGMA user_version = {}", i32::from(new_ver)),
        NO_PARAMS,
    ).map_err(DatabaseError::from)
        .map(|_| ())
}

/// Grabs the user pressure generated since the specified time in the guild database specified by the
/// [Connection]
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

/// Retrieves the current database version from a guild database.
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
        upgrade(&mut dummy_conn, None).unwrap();
        assert_eq!(get_db_version(&dummy_conn).unwrap(), *DB_VERSION)
    }

    #[test]
    pub fn test_migration_down() {
        let dummy_dir = TempDir::new("migrations").unwrap();
        let mut dummy_conn = ensure_guild_db(dummy_dir.as_ref(), GuildId::from(std::u64::MAX)).unwrap();
        upgrade(&mut dummy_conn, None).unwrap();
        downgrade(&mut dummy_conn, DatabaseVersion::Uninitialized).unwrap();
        assert_eq!(get_db_version(&dummy_conn).unwrap(), DatabaseVersion::Uninitialized);
        upgrade(&mut dummy_conn, None).unwrap();
        assert_eq!(get_db_version(&dummy_conn).unwrap(), *DB_VERSION)
    }
}
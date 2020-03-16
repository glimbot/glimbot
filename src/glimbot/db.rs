use diesel::{Associations, Identifiable, Insertable, Queryable};
use diesel::connection::{SimpleConnection, TransactionManager};
use diesel::deserialize::QueryableByName;
use diesel::prelude::*;
use diesel::query_builder::{AsQuery, QueryFragment, QueryId};
use diesel::r2d2::{ConnectionManager, Pool, PooledConnection};
use diesel::result::Error;
use diesel::sql_types::HasSqlType;
use diesel::sqlite::SqliteConnection;
use serenity::model::prelude::GuildId;

use crate::glimbot::schema::*;

pub struct GlimConn(SqliteConnection);

impl SimpleConnection for GlimConn {
    fn batch_execute(&self, query: &str) -> QueryResult<()> {
        self.0.batch_execute(query)
    }
}

/// Sets up a connection to the SQLite DB and ensures that busy time out is high, sync mode is NORMAL,
/// foreign key constraints are enforced, and that the DB is in WAL mode.
///
/// This gives much better write performance for SQLite.
impl Connection for GlimConn {
    type Backend = <SqliteConnection as Connection>::Backend;
    type TransactionManager = <SqliteConnection as Connection>::TransactionManager;

    fn establish(database_url: &str) -> ConnectionResult<Self> {
        let c = SqliteConnection::establish(database_url)?;
        c.batch_execute("PRAGMA foreign_keys = ON; PRAGMA busy_timeout = 60000; PRAGMA synchronous = NORMAL; PRAGMA journal_mode = WAL;")
            .unwrap();
        Ok(Self(c))
    }

    fn execute(&self, query: &str) -> QueryResult<usize> {
        (&self.0).execute(query)
    }

    fn query_by_index<T, U>(&self, source: T) -> QueryResult<Vec<U>> where
        T: AsQuery,
        T::Query: QueryFragment<Self::Backend> + QueryId,
        Self::Backend: HasSqlType<T::SqlType>,
        U: Queryable<T::SqlType, Self::Backend> {

        (&self.0).query_by_index(source)
    }

    fn query_by_name<T, U>(&self, source: &T) -> QueryResult<Vec<U>> where
        T: QueryFragment<Self::Backend> + QueryId,
        U: QueryableByName<Self::Backend> {
        (&self.0).query_by_name(source)
    }

    fn execute_returning_count<T>(&self, source: &T) -> QueryResult<usize> where
        T: QueryFragment<Self::Backend> + QueryId {
        (&self.0).execute_returning_count(source)
    }

    fn transaction_manager(&self) -> &Self::TransactionManager {
        (&self.0).transaction_manager()
    }
}

impl GlimConn {
    pub fn exclusive_transaction<T, E, F>(&self, f: F) -> Result<T, E>
        where
            F: FnOnce() -> Result<T, E>,
            E: From<Error>,
    {
        self.transaction_sql(f, "BEGIN EXCLUSIVE")
    }

    fn transaction_sql<T, E, F>(&self, f: F, sql: &str) -> Result<T, E>
        where
            F: FnOnce() -> Result<T, E>,
            E: From<Error>,
    {
        let transaction_manager = self.transaction_manager();

        transaction_manager.begin_transaction_sql(self, sql)?;
        match f() {
            Ok(value) => {
                transaction_manager.commit_transaction(self)?;
                Ok(value)
            }
            Err(e) => {
                transaction_manager.rollback_transaction(self)?;
                Err(e)
            }
        }
    }
}

pub fn establish_connection(p: impl AsRef<str>) -> SqliteConnection {
    SqliteConnection::establish(p.as_ref())
        .expect(&format!("Couldn't connect to db at {}", p.as_ref()))
}

pub fn connection_manager(p: impl AsRef<str>) -> ConnectionManager<GlimConn> {
    ConnectionManager::new(p.as_ref())
}

pub fn pooled_connection(p: impl AsRef<str>) -> Pool<ConnectionManager<GlimConn>> {
    let manager = connection_manager(p.as_ref());
    Pool::builder()
        .build(manager)
        .expect("Couldn't set up database connection.")
}

pub type DBPool = Pool<ConnectionManager<GlimConn>>;
pub type Conn = PooledConnection<ConnectionManager<GlimConn>>;

impl AsRef<SqliteConnection> for GlimConn {
    fn as_ref(&self) -> &SqliteConnection {
        &self.0
    }
}

#[derive(Queryable, Insertable, Identifiable, Eq, PartialEq, Hash, Debug)]
#[table_name = "guilds"]
pub struct Guild {
    id: i64
}

impl From<GuildId> for Guild {
    fn from(gid: GuildId) -> Self {
        Guild {
            id: gid.0 as i64
        }
    }
}

impl Into<GuildId> for Guild {
    fn into(self) -> GuildId {
        GuildId(self.id as u64)
    }
}

impl Guild {
    pub fn gid(&self) -> GuildId {
        GuildId(self.id as u64)
    }
}

#[derive(Associations, Queryable, Identifiable, Insertable, PartialEq, Eq, Hash, Debug)]
#[primary_key(guild_id, name)]
#[belongs_to(Guild)]
#[table_name = "incrementers"]
pub struct Incrementer {
    pub guild_id: i64,
    pub name: String,
    pub count: i64
}

impl Incrementer {
    pub fn new(g: GuildId, name: impl Into<String>) -> Incrementer {
        Incrementer {guild_id: g.0 as i64, name: name.into(), count: 0}
    }

    pub fn with_count(g: GuildId, name: impl Into<String>, init: i64) -> Incrementer {
        let mut out = Self::new(g, name);
        out.count = init;
        out
    }
}

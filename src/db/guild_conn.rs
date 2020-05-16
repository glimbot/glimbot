use rusqlite::{Connection};
use serenity::model::id::GuildId;

/// Wrapper around [Connection] to perform typical guild operations.
pub struct GuildConn { conn: Connection, id: GuildId }

impl GuildConn {
    /// Wraps a Connection to create a [GuildConn]
    pub fn new(id: GuildId, c: Connection) -> Self {
        GuildConn { conn: c, id }
    }

    /// Retrieves the command prefix of the guild from the database.
    pub fn command_prefix(&self) -> super::Result<char> {
        let s: String = self.get_value("command_prefix")?;
        Ok(s.chars().next().unwrap())
    }

    /// Sets the command prefix to the given character.
    pub fn set_command_prefix(&self, cmd: char) -> super::Result<()> {
        self.set_value("command_prefix", cmd.to_string())
    }

    /// Retrieves the config element with the given key.
    pub fn get_value(&self, key: impl AsRef<str>) -> super::Result<String> {
        let s: String = self.as_ref()
            .query_row(r#"SELECT value FROM guild_config WHERE key = ?;"#,
                params!(key.as_ref()),
                |r| r.get(0),
            )?;

        Ok(s)
    }

    /// Get or else set value.
    pub fn get_or_else_set_value(&self, key: impl AsRef<str>, els: impl FnOnce() -> String) -> super::Result<String> {
        let o = self.get_value(key.as_ref());
        if matches!(&o, Err(crate::db::DatabaseError::SQLError(rusqlite::Error::QueryReturnedNoRows))) {
            // This is IGNORE in the off chance another thread snipes us and adds the value before we get here.
            self.as_ref()
                .execute("INSERT OR IGNORE INTO guild_config VALUES (?, ?);",
                    params![key.as_ref(), &els()]
                )?;
            self.get_value(key)
        } else {
            o
        }
    }

    /// Sets the config element with the given key.
    pub fn set_value(&self, key: impl AsRef<str>, value: impl AsRef<str>) -> super::Result<()> {
        self.as_ref()
            .execute(
                r#"INSERT OR REPLACE INTO guild_config VALUES (?, ?);"#,
                params!(key.as_ref(), value.as_ref())
            )?;

        Ok(())
    }

    /// Retrieves the [GuildId] from this connection
    pub fn as_id(&self) -> &GuildId {
        &self.id
    }
}

impl AsRef<Connection> for GuildConn {
    fn as_ref(&self) -> &Connection {
        &self.conn
    }
}

impl AsMut<Connection> for GuildConn {
    fn as_mut(&mut self) -> &mut Connection {
        &mut self.conn
    }
}

#[cfg(test)]
mod tests {
    use tempdir::TempDir;
    use crate::db::{ensure_guild_db, init_guild_db, GuildConn};
    use serenity::model::id::GuildId;

    #[test]
    fn test_command_prefix() {
        let dummy_dir = TempDir::new("migrations").unwrap();
        let id = GuildId::from(std::u64::MAX);
        let mut dummy_conn = ensure_guild_db(dummy_dir.as_ref(), id).unwrap();
        init_guild_db(&mut dummy_conn).unwrap();
        let gconn = GuildConn::new(id, dummy_conn);
        let c = gconn.command_prefix().unwrap();
        assert_eq!(c, '!');
        gconn.set_command_prefix('~').unwrap();
        let c = gconn.command_prefix().unwrap();
        assert_eq!(c, '~');
    }
}
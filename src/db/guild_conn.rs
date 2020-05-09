use rusqlite::{Connection, NO_PARAMS};

/// Wrapper around [Connection] to perform typical guild operations.
pub struct GuildConn(Connection);

impl GuildConn {
    /// Wraps a Connection to create a [GuildConn]
    pub fn new(c: Connection) -> Self {
        GuildConn(c)
    }

    /// Retrieves the command prefix of the guild from the database.
    pub fn command_prefix(&self) -> super::Result<char> {
        let s: String = self.as_ref()
            .query_row(r#"SELECT command_prefix FROM guild_config;"#,
                       NO_PARAMS,
                       |r| r.get(0),
            )?;

        Ok(s.chars().next().unwrap())
    }

    /// Sets the command prefix to the given character.
    pub fn set_command_prefix(&self, cmd: char) -> super::Result<()> {
        let _ = self.as_ref()
            .execute(r#"UPDATE guild_config SET command_prefix = ?;"#, params!(cmd.to_string()))?;

        Ok(())
    }
}

impl AsRef<Connection> for GuildConn {
    fn as_ref(&self) -> &Connection {
        &self.0
    }
}

impl AsMut<Connection> for GuildConn {
    fn as_mut(&mut self) -> &mut Connection {
        &mut self.0
    }
}
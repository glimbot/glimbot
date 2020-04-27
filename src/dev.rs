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

use clap::{App, SubCommand, ArgMatches, Arg, AppSettings};
use serenity::model::id::GuildId;
use rusqlite::Connection;
use failure::Fallible;
use crate::db::{ensure_guild_db, init_guild_db};
use crate::db;

pub fn command_parser() -> App<'static, 'static> {
    trace!("Generating test command parser.");
    SubCommand::with_name("dev")
        .about("Commands related to development.")
        .subcommand(
            SubCommand::with_name(
                "dummy-db"
            ).arg(Arg::with_name("guild-id")
                .takes_value(true)
                .required(true)
                .value_name("GUILD_ID")
                .help("The guild to generate a dummy database file for. Created in $CWD."))
                .about("Creates a dummy database with the latest migrations for use in testing.")
        )
        .setting(AppSettings::SubcommandRequiredElseHelp)
}

pub fn handle_matches(m: &ArgMatches) -> Fallible<()> {
    if let ("dev", Some(m)) = m.subcommand() {
        match m.subcommand() {
            ("dummy-db", Some(m)) => {
                let gid = m.value_of("guild-id")
                    .unwrap()
                    .parse::<u64>()?;
                create_dummy_db(GuildId::from(gid))?;
            }
            _ => ()
        }
    }
    Ok(())
}

fn create_dummy_db(gid: GuildId) -> db::Result<Connection> {
    info!("Creating db for guild id {} in current directory.", gid);
    let mut conn = ensure_guild_db("./", gid)?;
    init_guild_db(&mut conn)?;
    info!("Done!");
    Ok(conn)
}
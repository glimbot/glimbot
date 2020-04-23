use clap::{App, SubCommand, ArgMatches, Arg, AppSettings};
use serenity::model::id::GuildId;
use rusqlite::Connection;
use failure::Fallible;
use crate::db::{ensure_guild_db, run_migrations, get_db_version, DB_VERSION};
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
    let conn = ensure_guild_db("./", gid)?;
    info!("Done!");
    Ok(conn)
}
//! Module to allow users to look up dictionary definitions.

use crate::modules::commands::{Command};
use serenity::prelude::Context;
use serenity::model::channel::Message;
use crate::dispatch::Dispatch;
use std::borrow::Cow;
use once_cell::unsync::Lazy;
use once_cell::sync::Lazy as SyncLazy;
use clap::{App, Arg, ArgMatches};
use crate::util::{help_str, string_from_cow, LogErrorExt};
use crate::args::parse_app_matches;
use crate::db::cache::get_cached_connection;
use crate::modules::{Module, config};
use std::collections::{HashSet};
use crate::modules::commands::Error::ConfigError;
use crate::db::DatabaseError;
use crate::data::Resources;
use std::rc::Rc;
use std::ops::Deref;
use crate::modules::config::{simple_validator, valid_parseable, fallible_validator};
use std::sync::Arc;
use rusqlite::OpenFlags;
use percent_encoding::utf8_percent_encode;
use std::num::ParseIntError;

const DISCORD_EMBED_FIELD_LIMIT: u64 = 25;

static DEFINITIONS_QUERY: SyncLazy<String> = SyncLazy::new(
    || Resources::get("definitions.sql")
        .map(string_from_cow).unwrap()
);

const DEFINES_LIMIT_KEY: &str = "defines_limit";

thread_local! {
    static PARSER: Lazy<App<'static, 'static>> = Lazy::new(
        || {
            App::new("define")
                .about("Retrieves a dictionary definition for the given word.")
                .arg(Arg::with_name("word")
                    .help("The word to look up. Short phrases are also accepted.")
                    .takes_value(true)
                    .value_name("WORD")
                    .required(true))
                .arg(Arg::with_name("pos")
                    .short("p")
                    .help("Converts ")
                    .takes_value(true)
                    .value_name("PARTS_OF_SPEECH")
                    .value_delimiter(",")
                    .require_delimiter(true)
                    .required(false)
                ).arg(Arg::with_name("cursor")
                .short("s")
                .alias("skip")
                .takes_value(true)
                .validator(fallible_validator::<usize, ParseIntError>)
                .default_value("0")
                .required(false)
                )
        }
    );

}

/// ZST for Defines command.
pub struct Define;

impl Command for Define {
    fn invoke(&self, disp: &Dispatch, ctx: &Context, msg: &Message, args: Cow<str>) -> super::commands::Result<()> {
        static DB_PATH: SyncLazy<Option<String>> = SyncLazy::new(|| {
            std::env::var("GLIMBOT_DICTIONARY").map(|s| shellexpand::tilde(&s).into_owned()).ok()
        });

        thread_local! {
            static CONN: Lazy<Rc<super::commands::Result<rusqlite::Connection>>> = Lazy::new(||Rc::new(if DB_PATH.is_none() {
                Err(ConfigError("Glimbot does not have a dictionary configured.".into()))
            } else {
                debug!("Dictionary at {}", DB_PATH.as_ref().unwrap());
                rusqlite::Connection::open_with_flags(DB_PATH.as_ref().unwrap(),
                      OpenFlags::SQLITE_OPEN_READ_ONLY
                    | OpenFlags::SQLITE_OPEN_NO_MUTEX
                    | OpenFlags::SQLITE_OPEN_URI
                    | OpenFlags::SQLITE_OPEN_SHARED_CACHE,
                ).map_err(DatabaseError::SQLError).map_err(DatabaseError::into)
            }));
        }

        let m: ArgMatches = PARSER.with(|p| parse_app_matches("define", args, p))?;

        let word = m.value_of("word").unwrap();
        let pos = m.values_of_lossy("pos").unwrap_or_default().into_iter().collect::<HashSet<_>>();

        let c = CONN.with(|c| c.deref().clone());
        let conn_res = c.as_ref();
        conn_res.log_error();

        let conn = match conn_res {
            Ok(c) => { c }
            Err(_) => { return Err(ConfigError("dictionary incorrectly configured.".into())); }
        };

        let mut stmt = conn.prepare_cached(&DEFINITIONS_QUERY).map_err(DatabaseError::SQLError)?;
        let defs = stmt.query_map_named(
            named_params! {
                ":word": word
            },
            |row| { Ok((row.get::<usize, String>(0)?, row.get::<usize, String>(1)?)) },
        ).map_err(DatabaseError::SQLError)?;

        let gid = msg.guild_id.unwrap().clone();
        let gconn = get_cached_connection(gid)?;
        let rf = gconn.borrow();

        let skip_cnt = m.value_of("cursor").unwrap().parse::<usize>().unwrap();


        let limit = disp.get_or_set_config(&rf, DEFINES_LIMIT_KEY)?.parse::<u64>().unwrap().min(DISCORD_EMBED_FIELD_LIMIT);
        let defs = defs.filter(Result::is_ok)
            .map(Result::unwrap)
            .filter(|(p, _)| pos.is_empty() || pos.contains(p))
            .enumerate()
            .skip(skip_cnt)
            .take(limit as usize)
            .map(|(i, (pos, def))| {
                (format!("{}: {}", i + 1, pos), def)
            });

        msg.channel_id.send_message(ctx, |m| {
            m.embed(|e| {
                e.title(format!("{}", word));
                e.url(format!("https://en.wiktionary.org/wiki/{}", utf8_percent_encode(word, percent_encoding::NON_ALPHANUMERIC)));
                defs.for_each(|(k, v)| {
                    trace!("{}: {}", k, v);
                    e.field(k, v, false);
                });

                e.footer(|f| f.text(format!("Displaying up to {} definitions.{} Copyright Wiktionary", limit,
                    if skip_cnt > 0 {
                        format!(" Skipped {}.", skip_cnt)
                    } else {
                        "".to_string()
                    }
                )))
            })
        })?;

        Ok(())
    }

    fn help(&self) -> Cow<'static, str> {
        PARSER.with(|p| help_str(&p).into())
    }
}

/// Creates a dictionary module.
pub fn define_mod() -> Module {
    Module::with_name("define")
        .with_sensitivity(false)
        .with_command(Define)
        .with_config_value(config::Value::new(
            DEFINES_LIMIT_KEY,
            "The maximum number of definitions to return. Numbers greater than Discord embed limit are treated as Discord embed limit.",
            Arc::new(simple_validator(valid_parseable::<u64>)),
            Some("10"),
        ))
        .with_dependency("config")
}
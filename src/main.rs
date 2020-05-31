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

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![allow(dead_code)]
#![deny(unused_imports)]

//! Glimbot is a general admin and anti-spam bot for Discord, written in Rust.
//! The primary design goal is to create a robust Discord bot with high performance to
//! manage large servers in the spirit of SweetieBot.


#[macro_use]
extern crate log;
#[macro_use]
extern crate rusqlite;

use std::env;
use std::path::Path;
use crate::data::{data_folder, AUTHORS, VERSION};
use clap::{App, AppSettings, Arg};
use log4rs::config::{Config, Appender, Logger, Root};
use log4rs::append::console::ConsoleAppender;
use log::LevelFilter;
use log4rs::encode::pattern::PatternEncoder;

pub mod data;
pub mod db;
pub mod util;

#[cfg(feature = "development")]
pub mod dev;

pub mod args;
pub mod dispatch;
pub mod modules;
pub mod error;

fn main() -> anyhow::Result<()> {
    better_panic::install();
    let _ = dotenv::dotenv();

    let mut subcommands = vec![];

    #[cfg(feature = "development")]
        subcommands.push(dev::command_parser());

    subcommands.push(db::args::command_parser());
    subcommands.push(dispatch::args::command_parser());

    let matches = App::new(env!("CARGO_PKG_NAME"))
        .about(env!("CARGO_PKG_DESCRIPTION"))
        .author(AUTHORS)
        .version(VERSION)
        .subcommands(subcommands)
        .arg(Arg::with_name("verbosity")
            .short("v")
            .multiple(true)
            .help("Sets the logging verbosity level. Default: INFO")
        )
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .get_matches();

    let verbosity = match matches.occurrences_of("verbosity") {
        0 => LevelFilter::Info,
        1 => LevelFilter::Debug,
        i if i >= 2 => LevelFilter::Trace,
        _ => unreachable!()
    };

    init_logging(verbosity)?;
    // Create our working directory
    let data_dir = data_folder();
    ensure_data_folder(&data_dir);

    #[cfg(feature = "development")]
        dev::handle_matches(&matches)?;

    db::args::handle_matches(&matches)?;
    dispatch::args::handle_matches(&matches)?;

    Ok(())
}

fn ensure_data_folder(p: impl AsRef<Path>) {
    std::fs::create_dir_all(p).expect("Couldn't create the path to default directory.");
}


fn init_logging(l: LevelFilter) -> anyhow::Result<()> {
    let stdout = ConsoleAppender::builder()
        .encoder(Box::new(
            PatternEncoder::new("[{d(%s%.3f)(utc)}][{h({l:<5})}][{M}][{I}]  {m}{n}")
        ))
        .build();

    let config = Config::builder()
        .appender(Appender::builder().build("stdout", Box::new(stdout)))
        .logger(Logger::builder().build("glimbot", l))
        // FIXME Remove trace logging
        .logger(Logger::builder().build("serenity", LevelFilter::Debug))
        .build(Root::builder().appender("stdout").build(LevelFilter::Info))?;

    log4rs::init_config(config)?;

    Ok(())
}

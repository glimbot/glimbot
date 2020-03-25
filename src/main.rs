#![forbid(unsafe_code)]
#![allow(dead_code)]

#[macro_use] extern crate diesel;
#[macro_use] extern crate log;
#[macro_use] extern crate pest_derive;

use std::fs::File;
use std::path::Path;
use std::thread;

use clap::{App, Arg};
use log::{debug, error, info};
use log::LevelFilter::Info;
use serenity::Client;
use serenity::framework::standard::CommandResult;
use serenity::model::prelude::*;
use serenity::prelude::*;

use crate::glimbot::config::Config;
use crate::glimbot::GlimDispatch;
use crate::glimbot::modules::bag::bag_module;
use crate::glimbot::modules::help::help_module;
use crate::glimbot::modules::ping::ping_module;
use crate::glimbot::modules::bot_admin::bot_admin_module;
use crate::glimbot::modules::incrementer::incrementer_module;
use crate::glimbot::modules::dice::roll_module;

mod glimbot;

fn init_logging(cwd: &str, level: log::LevelFilter) -> std::result::Result<(), fern::InitError> {
    fern::Dispatch::new()
        .format(|out, msg, rec| {
            let now = chrono::Local::now();
            out.finish(format_args!(
                "[{}.{:03}][{:?}][{}][{}] {}",
                now.timestamp(),
                now.timestamp_subsec_millis(),
                thread::current().id(),
                rec.level(),
                rec.module_path().unwrap_or("<unk>"),
                msg
            ))
        })
        .chain(fern::Dispatch::new()
            .level(log::LevelFilter::Warn)
            .level_for("glimbot", level)
            .chain(std::io::stdout()))
        .chain(fern::Dispatch::new()
            .level(log::LevelFilter::Warn)
            .level_for("glimbot", log::LevelFilter::Debug)
            .chain(
                std::fs::OpenOptions::new()
                    .write(true)
                    .create(true)
                    .truncate(true)
                    .open(Path::new(cwd).join("glimlog.txt"))?))
        .apply()?;

    Ok(())
}


fn main() {
    better_panic::install();

    let matches = App::new("Glimbot - The Discord Admin Bot")
        .version(glimbot::env::VERSION)
        .author(glimbot::env::AUTHORS)
        .about("She is always watching.")
        .arg(Arg::with_name("CONFIG")
            .help("Glimbot config file.")
            .required(true)
            .index(1))
        .arg(Arg::with_name("working_dir")
            .short("w")
            .long("working-dir")
            .takes_value(true)
            .value_name("DIR")
            .help("The directory in which to read/write logs, server configs, database, etc. Will be created if doesn't exist.")
        ).arg(Arg::with_name("verbose")
        .short("v")
        .multiple(true)
        .help("Specify multiple times to increase stdout logging level.")
    )
        .get_matches();

    let config = matches.value_of("CONFIG").unwrap();
    let config_file = File::open(config).expect("Glimmy needs her config file.");

    let wd = matches.value_of("working_dir").unwrap_or("./");
    std::fs::create_dir_all(wd).expect("Couldn't create working directory.");

    let stdout_log_level = match matches.occurrences_of("verbose") {
        0 => Info,
        1 => log::LevelFilter::Debug,
        _ => log::LevelFilter::Trace
    };

    init_logging(wd, stdout_log_level).unwrap();
    info!("Glimbot version {} coming online.", glimbot::env::VERSION);

    let conf_map: Config = serde_yaml::from_reader(config_file).unwrap();

    let glim = GlimDispatch::new()
        .with_module(incrementer_module())
        .with_module(roll_module())
        .with_module(ping_module())
        .with_module(help_module())
        .with_module(bag_module())
        .with_module(bot_admin_module());

    let mut client = Client::new(conf_map.token(), glim)
        .expect("Could not connect to Discord. B̵a̵n̵i̵s̵h̵ ̵s̵p̵e̵l̵l̵ ̵i̵n̵e̵f̵f̵e̵c̵t̵i̵v̵e̵.");

    client.start_autosharded().expect("Could not start")
}

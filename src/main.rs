use clap::{App, Arg};
use std::fs::File;
use crate::glimbot::config::Config;
use serenity::prelude::*;
use serenity::model::prelude::*;
use serenity::Client;
use serenity::framework::standard::CommandResult;
use log::{info, debug};
use std::path::Path;

mod glimbot;

struct Handler;

impl EventHandler for Handler {
    fn message(&self, ctx: Context, new_message: Message) {
        if new_message.content == "!ping" {
            let _ = new_message.channel_id.say(&ctx, "Pong!");
        }
    }

    fn ready(&self, ctx: Context, data_about_bot: Ready) {
        use serenity::model::gateway::Activity;
        info!("Connected to Discord!");
        data_about_bot.guilds.iter().for_each(
            |g| debug!("Connected to guild: {}", g.id()
                .to_guild_cached(&ctx.cache)
                .map_or("<unk>".to_string(), |g| g.read().name.clone()))
        );
        ctx.set_activity(Activity::playing("Cultist Simulator"))
    }
}

fn init_logging(cwd: &str) -> std::result::Result<(), fern::InitError> {
    fern::Dispatch::new()
        .format(|out, msg, rec| {
            let now = chrono::Local::now();
            out.finish(format_args!(
                "[{}.{:03}][{}][{}] {}",
                now.timestamp(),
                now.timestamp_subsec_millis(),
                rec.level(),
                rec.module_path().unwrap_or("<unk>"),
                msg
            ))
        })
        .chain(fern::Dispatch::new()
            .level(log::LevelFilter::Warn)
            .level_for("glimbot", log::LevelFilter::Info)
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
        )
        .get_matches();

    let config = matches.value_of("CONFIG").unwrap();
    let config_file = File::open(config).expect("Glimmy needs her config file.");

    let wd = matches.value_of("working_dir").unwrap_or("./");
    std::fs::create_dir_all(wd).expect("Couldn't create working directory.");

    init_logging(wd).unwrap();
    info!("Glimbot version {} coming online.", glimbot::env::VERSION);

    let conf_map: Config = serde_yaml::from_reader(config_file).unwrap();
    let mut client = Client::new(conf_map.token(), Handler)
        .expect("Could not connect to Discord. B̵a̵n̵i̵s̵h̵ ̵s̵p̵e̵l̵l̵ ̵i̵n̵e̵f̵f̵e̵c̵t̵i̵v̵e̵.");

    client.start_autosharded().expect("Could not start")
}

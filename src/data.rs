pub const VERSION: &'static str = env!("CARGO_PKG_VERSION");
pub const AUTHORS: &'static str = env!("CARGO_PKG_AUTHORS");

use dirs;
use std::path::PathBuf;
use serenity::model::id::GuildId;

embed_migrations!();

pub fn data_folder() -> PathBuf {
    let mut path = std::env::var("GLIMBOT_DIR")
        .map_or_else(default_folder,
                     PathBuf::from);
    path
}

pub fn default_folder() -> PathBuf {
    let mut base = dirs::data_dir().expect("Running on an unsupported platform.");
    base.push("glimbot");
    base
}

pub fn create_guild_db(g: GuildId) ->
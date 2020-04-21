pub const VERSION: &'static str = env!("CARGO_PKG_VERSION");
pub const AUTHORS: &'static str = env!("CARGO_PKG_AUTHORS");
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "$CARGO_MANIFEST_DIR/resources/"]
pub struct Resources;

use dirs;
use std::path::{PathBuf, Path};
use serenity::model::id::GuildId;
use std::io;
use rusqlite::Connection;

pub fn data_folder() -> PathBuf {
    let mut path = std::env::var("GLIMBOT_DIR")
        .map_or_else(|_| default_folder(),
                     PathBuf::from);
    path
}

pub fn default_folder() -> PathBuf {
    let mut base = dirs::data_dir().expect("Running on an unsupported platform.");
    base.push("glimbot");
    base
}
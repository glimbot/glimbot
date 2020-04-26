//! This module contains strings, resources, and other functions related to
//! importation and management of the data infrastructure for Glimbot.

#[doc(hidden)]
pub const VERSION: &'static str = env!("CARGO_PKG_VERSION");
#[doc(hidden)]
pub const AUTHORS: &'static str = env!("CARGO_PKG_AUTHORS");
use rust_embed::RustEmbed;


#[derive(RustEmbed)]
#[folder = "$CARGO_MANIFEST_DIR/resources/"]
/// Embedded resources (mostly SQL files).
pub struct Resources;

use dirs;
use std::path::{PathBuf};

/// Grabs either the current data folder path from GLIMBOT_DIR
pub fn data_folder() -> PathBuf {
    let path = std::env::var("GLIMBOT_DIR")
        .map_or_else(|_| default_folder(),
                     PathBuf::from);
    path
}

/// Gets the default data folder for applications on the platform.
fn default_folder() -> PathBuf {
    let mut base = dirs::data_dir().expect("Running on an unsupported platform.");
    base.push("glimbot");
    base
}
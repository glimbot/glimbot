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
use std::path::{PathBuf, Path};
use anyhow::Error as FErr;
use std::ops::Deref;
use once_cell::sync::OnceCell;


/// Grabs either the current data folder path from GLIMBOT_DIR
pub fn data_folder() -> &'static Path {

    static DATA_FOLDER: OnceCell<PathBuf> = OnceCell::new();

    &DATA_FOLDER.get_or_init(|| {
        let path = std::env::var("GLIMBOT_DIR")
            .map_err(FErr::from)
            .and_then(|s| shellexpand::full(&s).map_err(FErr::from).map(|x|x.into_owned()))
            .map_or_else(|_| default_folder(), |c| PathBuf::from(c.deref()));
        trace!("Data directory is {}", path.to_str().unwrap_or("<unk>"));
        path
    })

}

/// Gets the default data folder for applications on the platform.
fn default_folder() -> PathBuf {
    let mut base = dirs::data_dir().expect("Running on an unsupported platform.");
    base.push("glimbot");
    base
}
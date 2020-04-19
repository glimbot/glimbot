#![forbid(unsafe_code)]
#![allow(dead_code)]

#[macro_use] extern crate diesel;
#[macro_use] extern crate log;
#[macro_use] extern crate diesel_migrations;

use std::env;
use std::path::Path;
use crate::data::data_folder;

pub mod data;
fn main() {
    better_panic::install();
    let _ = dotenv::dotenv();
    let token = std::env::var("GLIMBOT_TOKEN").expect("Need a token to connect to Discord.");
    let owner = std::env::var("GLIMBOT_OWNER").unwrap_or_default().parse::<u64>().ok();

    // Create our working directory
    let data_dir = data_folder();
    ensure_data_folder(&data_dir);
}

fn ensure_data_folder(p: impl AsRef<Path>) {
    std::fs::create_dir_all(p).expect("Couldn't create the path to default directory.");
}
use std::collections::HashSet;

pub mod env;
pub mod config;
pub mod modules;
pub mod guilds;
pub mod models;
pub mod schema;

type BotPermissions = HashSet<String>;
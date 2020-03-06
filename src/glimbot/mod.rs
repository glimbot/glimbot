use std::collections::HashSet;
use std::path::{Path, PathBuf};

pub mod env;
pub mod config;
pub mod modules;
pub mod guilds;
pub mod util;

type BotPermissions = HashSet<String>;

#[derive(Debug)]
pub struct GlimDispatch {
    working_directory: PathBuf
}

impl GlimDispatch {
    pub fn new(p: impl AsRef<Path>) -> Self {
        GlimDispatch {working_directory: p.as_ref().to_owned()}
    }

    pub fn with_working_directory(mut self, p: impl AsRef<Path>) -> Self {
        self.working_directory = p.as_ref().to_owned();
        self
    }
}
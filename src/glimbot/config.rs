use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    token: String,
    bot_owner: u64,
}

impl Config {
    pub fn token(&self) -> &str {
        &self.token
    }
    pub fn bot_owner(&self) -> u64 { self.bot_owner }
}

#[cfg(test)]
mod tests {
    use crate::glimbot::config::Config;
}
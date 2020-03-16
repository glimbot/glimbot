use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    token: String,
}

impl Config {
    pub fn token(&self) -> &str {
        &self.token
    }
}

#[cfg(test)]
mod tests {
    use crate::glimbot::config::Config;
}
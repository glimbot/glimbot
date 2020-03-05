

use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    token: String
}

impl Config {
    pub fn token(&self) -> &str {
        &self.token
    }
}

#[cfg(test)]
mod tests {
    use crate::glimbot::config::Config;

    #[test]
    fn serialize_config() {
        let c = Config {
            token: "ABCDEF".to_owned()
        };

        let serialized = serde_yaml::to_string(&c).unwrap();
        println!("{}", serialized)
    }

}
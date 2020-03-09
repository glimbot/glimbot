use std::collections::HashMap;
use std::fmt;
use std::fs::File;
use std::ops::Deref;
use std::sync::Arc;

use log::{error, trace};
use parking_lot::RwLock;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde::de::{MapAccess, Visitor};
use serde::export::fmt::Error;
use serde::export::Formatter;
use serde::ser::SerializeMap;
use serenity::model::id::GuildId;

use crate::glimbot::modules::{ModuleConfig, RwModuleConfigPtr};

#[derive(Debug, Serialize, Deserialize)]
pub struct GuildContext {
    pub guild: GuildId,
    pub command_prefix: String,
    #[serde(serialize_with = "write_mod_configs")]
    #[serde(deserialize_with = "read_mod_configs")]
    pub module_configs: HashMap<String, RwModuleConfigPtr>,
}

impl GuildContext {
    pub fn new(g: GuildId) -> Self {
        GuildContext {
            guild: g,
            command_prefix: "!".to_string(),
            module_configs: HashMap::new()
        }
    }
}

fn write_mod_configs<S>(confs: &HashMap<String, RwModuleConfigPtr>, s: S) -> Result<S::Ok, S::Error> where S : Serializer {
    let mut m = s.serialize_map(Some(confs.len()))?;
    for (k, v) in confs {
        let rv = v.read();
        m.serialize_entry(k, rv.deref())?;
    };
    m.end()
}

struct ModuleConfigsDe;

impl <'de> Visitor<'de> for ModuleConfigsDe {
    type Value = HashMap<String, RwModuleConfigPtr>;

    fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Module configurations and the modules they relate to.")
    }

    fn visit_map<A>(self, mut access: A) -> Result<Self::Value, <A as MapAccess<'de>>::Error> where
        A: MapAccess<'de>, {
        let mut map: Self::Value = Self::Value::with_capacity(access.size_hint().unwrap_or(0));

        while let Some((k, v)) = access.next_entry()? {
            map.insert(k, RwModuleConfigPtr::new(RwLock::new(v)));
        };

        Ok(map)
    }
}

fn read_mod_configs<'de, D>(d: D) -> Result<HashMap<String, RwModuleConfigPtr>, D::Error> where D: Deserializer<'de> {
    d.deserialize_map(ModuleConfigsDe)
}

pub type RwGuildPtr = Arc<RwLock<GuildContext>>;

impl From<GuildContext> for RwGuildPtr {
    fn from(g: GuildContext) -> Self {
        Arc::new(RwLock::new(g))
    }
}

impl GuildContext {
    pub fn file_name(&self) -> String {
        return format!("{}_conf.yaml", self.guild)
    }

    pub fn commit_to_disk(&mut self) {
        let f = std::fs::OpenOptions::new()
            .write(true)
            .truncate(true)
            .create(true)
            .open(self.file_name());

        match f {
            Ok(f) => {
                let r = serde_yaml::to_writer(f, self);
                if let Some(e) = r.err() {
                    error!("While writing guild {}: {}", self.guild, e);
                } else {
                    trace!("Saved guild {}", self.guild)
                }
            },
            Err(e) => {error!("While writing guild {}: {}", self.guild, e);},
        }
    }
}
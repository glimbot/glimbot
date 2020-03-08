use crate::glimbot::modules::command::Commander;
use std::collections::{HashMap, HashSet, BTreeMap};
use serenity::model::permissions::Permissions;
use std::sync::Arc;

pub mod command;
pub mod ping;

use serde::{Deserialize, Serialize, Serializer};
use std::cell::RefCell;
use std::convert::TryFrom;
use std::error::Error;
use std::fmt::Display;
use serde::export::Formatter;
use serenity::model::prelude::EventType;
use crate::glimbot::EventHandler;

pub type ModuleConfig = HashMap<String, serde_yaml::Value>;

pub type ConfigFn = fn() -> ModuleConfig;

#[derive(Clone)]
pub struct Module {
    name: String,
    commands: HashMap<String, Commander>,
    hooks: BTreeMap<EventType, EventHandler>,
    create_config: ConfigFn,
    req_permissions: Permissions,
}

impl Module {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn commands(&self) -> &HashMap<String, Commander> {
        &self.commands
    }

    pub fn hooks(&self) -> &BTreeMap<EventType, EventHandler> {
        &self.hooks
    }

    pub fn default_config(&self) -> ModuleConfig {
        (self.create_config)()
    }

    pub fn required_perms(&self) -> Permissions {
        self.req_permissions
    }
}

#[derive(Clone)]
pub struct ModuleBuilder {
    module: Module
}

pub fn default_config() -> ModuleConfig {
    ModuleConfig::new()
}

impl ModuleBuilder {
    pub fn new(name: impl Into<String>) -> ModuleBuilder {
        ModuleBuilder {
            module: Module {
                name: name.into(),
                commands: HashMap::new(),
                hooks: BTreeMap::new(),
                create_config: default_config,
                req_permissions: Permissions::default(),
            }
        }
    }

    pub fn with_command(mut self, cmd: Commander) -> Self {
        self.module.req_permissions |= cmd.permissions();
        self.module.commands.insert(cmd.name().to_string(), cmd);
        self
    }

    pub fn with_default_config_gen(mut self, conf: ConfigFn) -> Self {
        self.module.create_config = conf;
        self
    }

    pub fn with_hook(mut self, typ: EventType, hook: EventHandler) -> Self {
        match &typ {
            EventType::MessageCreate => {
                match &hook {
                    EventHandler::MessageHandler(_) => {}
                    EventHandler::CommandHandler(_) => {}
                    _ => panic!("MessageCreate cannot use GenericHandlers")
                }
            }
            EventType::MessageDelete |
            EventType::MessageDeleteBulk |
            EventType::MessageUpdate => {
                match &hook {
                    EventHandler::MessageHandler(_) => {}
                    _ => panic!("MessageUpdate/Delete/DeleteBulk can only use MessageHandler")
                }
            }
            _ => {
                match &hook {
                    EventHandler::GenericHandler(_) => {}
                    _ => panic!("All non-message events must use GenericHandler")
                }
            }
        };

        self.module.hooks.insert(typ, hook);
        self
    }

    pub fn build(self) -> Module {
        self.module
    }
}
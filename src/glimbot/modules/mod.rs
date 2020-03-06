use crate::glimbot::modules::command::Commander;
use std::collections::{HashMap, HashSet};
use serenity::model::permissions::Permissions;

pub mod command;
pub mod ping;

#[derive(Clone, Debug)]
pub struct Module {
    name: String,
    commands: HashMap<String, Commander>,
    req_permissions: Permissions,
}

#[derive(Clone, Debug)]
pub struct ModuleBuilder {
    module: Module
}

impl ModuleBuilder {
    pub fn new(name: impl Into<String>) -> ModuleBuilder {
        ModuleBuilder {
            module: Module {
                name: name.into(),
                commands: HashMap::new(),
                req_permissions: Permissions::default()
            }
        }
    }

    pub fn with_command(mut self, cmd: Commander) -> Self {
        self.module.req_permissions |= cmd.permissions();
        self.module.commands.insert(cmd.name().to_string(), cmd);
        self
    }

    pub fn build(self) -> Module {
        self.module
    }
}
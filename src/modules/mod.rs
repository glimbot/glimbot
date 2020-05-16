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

//! Contains functionality related to the module system with Glimbot.
//! Modules represent functionality for Glimbot, and will generally either process events from the server,
//! process commands from users, or some combination of the two.

use crate::modules::commands::Command;
use std::sync::Arc;
use crate::modules::hook::CommandHookFn;
use std::collections::HashSet;

pub mod commands;
pub mod hook;
pub mod ping;
pub mod base_hooks;
pub mod no_bot;
pub mod config;
pub mod roles;

/// An integrated unit of functionality for Glimbot. A module may have a command module associated with it,
/// and one or more hooks.
pub struct Module {
    name: String,
    command_handler: Option<Arc<dyn Command>>,
    command_hooks: Vec<CommandHookFn>,
    config_values: Vec<config::Value>,
    dependencies: HashSet<String>,
    sensitive: bool
}

impl Module {
    /// Creates a new module with the given name.
    pub fn with_name(name: impl Into<String>) -> Self {
        let mut o = Module {
            name: name.into(),
            command_hooks: Vec::new(),
            command_handler: None,
            config_values: Vec::new(),
            dependencies: HashSet::new(),
            sensitive: true
        };

        o.dependencies.insert("base_hooks".to_string());
        o
    }

    /// Clears all dependencies from a module.
    pub fn clear_dependencies(mut self) -> Self {
        self.dependencies.clear();
        self
    }

    /// Sets the sensitivity for the module.
    pub fn with_sensitivity(mut self, is_sensitive: bool) -> Self {
        self.sensitive = is_sensitive;
        self
    }

    /// Adds a command hook to the current module.
    pub fn with_command_hook(mut self, f: CommandHookFn) -> Self {
        self.command_hooks.push(f);
        self
    }

    /// Sets the command handler for the current module.
    pub fn with_command<T: Command + 'static>(mut self, cmd: T) -> Self {
        let ptr: Arc<dyn Command> = Arc::new(cmd);
        self.command_handler = Some(ptr);
        self
    }

    /// Associates a [Value][config::Value] with this module.
    pub fn with_config_value(mut self, v: config::Value) -> Self {
        self.config_values.push(v);
        self
    }

    /// Associates a module with another module as a dependency.
    /// Loading this module will panic if the other modules are not loaded.
    pub fn with_dependency(mut self, d: impl Into<String>) -> Self {
        self.dependencies.insert(d.into());
        self
    }

    /// Associates several modules with other modules.
    pub fn with_dependencies(mut self, ds: impl IntoIterator<Item = String>) -> Self {
        self.dependencies.extend(ds);
        self
    }

    /// Accessor for associated config values.
    pub fn config_values(&self) -> &[config::Value] {
        &self.config_values
    }

    /// Accessor for the name of the module.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Accessor for the command handler, if any.
    pub fn command_handler(&self) -> Option<&Arc<dyn Command>> {
        self.command_handler.as_ref()
    }

    /// Accessor for any command hooks held in the Module.
    pub fn command_hooks(&self) -> &[CommandHookFn] {
        &self.command_hooks
    }

    /// Accessor for the dependencies on other modules for this module.
    pub fn dependencies(&self) -> &HashSet<String> {
        &self.dependencies
    }

    /// Accessor for whether or not this command should be restricted by default.
    pub fn is_sensitive(&self) -> bool {
        self.sensitive
    }
}
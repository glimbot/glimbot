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

pub mod commands;
pub mod hook;
pub mod ping;

/// An integrated unit of functionality for Glimbot. A module may have a command module associated with it,
/// and one or more hooks.
pub struct Module {
    name: String,
    command_handler: Option<Arc<dyn Command>>,
    command_hooks: Vec<CommandHookFn>
}

impl Module {
    /// Creates a new module with the given name.
    pub fn with_name(name: impl Into<String>) -> Self {
        Module {
            name: name.into(),
            command_hooks: Vec::new(),
            command_handler: None,
        }
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
}
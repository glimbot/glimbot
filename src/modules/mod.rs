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

pub mod commands;

pub struct Module {
    name: String,
    command_handler: Option<Arc<dyn Command>>
}
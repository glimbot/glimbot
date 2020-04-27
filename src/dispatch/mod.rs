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

use serenity::prelude::{EventHandler, Context};
use serenity::model::gateway::{Ready, Activity};
use crate::util::LogErrorExt;
use crate::db::cache::get_cached_connection;

pub mod args;

pub struct Dispatch {

}

impl EventHandler for Dispatch {
    fn ready(&self, ctx: Context, data_about_bot: Ready) {
        ctx.set_activity(Activity::playing("Cultist Simulator"));
        let active_guilds = &data_about_bot.guilds;
        active_guilds.iter().for_each(
            |g| get_cached_connection(g.id()).log_error()
        )
    }
}

impl Dispatch {
    pub fn new() -> Self {
        Dispatch {
        }
    }
}
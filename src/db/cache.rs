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

use once_cell::sync::Lazy;
use once_cell::unsync::Lazy as UnsyncLazy;
use lru_cache::LruCache;
use rusqlite::Connection;
use std::cell::RefCell;
use serenity::model::prelude::GuildId;
use std::rc::Rc;
use crate::db::{ensure_guild_db_in_data_dir};

pub static NUM_CACHED_CONNECTIONS: Lazy<usize> = Lazy::new(
    || std::env::var("GLIMBOT_DB_CONN_PER_THREAD")
        .unwrap_or_else(|_| "16".to_string())
        .parse::<usize>()
        .expect("GLIMBOT_DB_CONN_PER_THREAD must be a valid usize.")
);

thread_local! {
    static CONNECTION_CACHE: UnsyncLazy<RefCell<LruCache<GuildId, Rc<RefCell<Connection>>>>> = UnsyncLazy::new(
        || RefCell::new(LruCache::new(*NUM_CACHED_CONNECTIONS))
    );
}

pub fn get_cached_connection(g: GuildId) -> super::Result<Rc<RefCell<Connection>>> {
    CONNECTION_CACHE.with(
        |cache| {
            let mut cache_ref = cache.borrow_mut();
            match cache_ref.get_mut(&g) {
                None => {
                    let out = Rc::new(
                        RefCell::new(
                            ensure_guild_db_in_data_dir(g)?
                        )
                    );

                    cache_ref.insert(g, out.clone());
                    Ok(out)
                },
                Some(rc) => {Ok(rc.clone())},
            }
        }
    )
}
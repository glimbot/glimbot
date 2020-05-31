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

//! Contains utility types and functions related to common functionality which would otherwise
//! be in a module by itself.
use std::borrow::Cow;
use std::fmt::Display;
use clap::App;
use std::io::Cursor;

/// Converts a string into a [Cow], unwrapping the result.
/// # Panics
/// If the given string is not valid UTF-8
pub fn string_from_cow(s: Cow<'static, [u8]>) -> String {
    String::from_utf8(s.into_owned()).unwrap()
}

/// A generic extension for result types that logs the error in a result if present.
pub trait LogErrorExt<E: Display> {
    /// Logs an error for this type.
    fn log_error(&self);
}

impl<T, E: Display> LogErrorExt<E> for Result<T, E> {
    fn log_error(&self) {
        if let Err(e) = self {
            error!("{}", e)
        }
    }
}

/// Grabs the help string from an [App]
pub fn help_str(app: &App) -> String {
    let mut curs = Cursor::new(Vec::new());
    app.write_help(&mut curs).unwrap();
    String::from_utf8(curs.into_inner()).unwrap()
}

/// Hashmap literal.
#[macro_export]
macro_rules! hashmap {
    ($( $key: expr => $val: expr ),*) => {{
         let mut map = ::std::collections::HashMap::new();
         $( map.insert($key, $val); )*
         map
    }}
}
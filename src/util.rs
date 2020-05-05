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
use std::borrow::Cow;
use std::error::Error;

pub type Fallible<T> = anyhow::Result<T>;

pub fn string_from_cow(s: Cow<'static, [u8]>) -> String {
    String::from_utf8(s.into_owned()).unwrap()
}

pub trait LogErrorExt<E: Error> {
    fn log_error(&self);
}

impl<T, E: Error> LogErrorExt<E> for Result<T, E> {
    fn log_error(&self) {
        if let Err(e) = self {
            error!("{}", e)
        }
    }
}
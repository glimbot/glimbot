// Glimbot - A Discord anti-spam and administration bot.
// Copyright (C) 2020-2021 Nick Samson

// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Main entry point for Glimbot. Additionally controls DB migration.

#![forbid(unsafe_code)]
// #![deny(clippy::missing_docs_in_private_items, missing_docs, missing_crate_level_docs)]
#![deny(unused_must_use)]
#![allow(dead_code)]

#[macro_use]
extern crate serde;
#[macro_use]
extern crate shrinkwraprs;
#[macro_use]
extern crate tracing;

#[macro_use]
pub mod error;
#[macro_use]
pub mod db;
#[macro_use]
pub mod dispatch;
pub mod about;
pub mod example;
pub mod module;
pub mod run;
pub mod util;

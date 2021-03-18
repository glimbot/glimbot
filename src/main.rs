// Glimbot - A Discord anti-spam and administration bot.
// Copyright (C) 2020-2021 Nick Samson

// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Main entry point for Glimbot. Additionally controls DB migration.

#![forbid(unsafe_code)]
// #![deny(clippy::missing_docs_in_private_items, missing_docs, missing_crate_level_docs)]
#![deny(unused_must_use, )]
#![allow(dead_code)]
#![feature(const_panic)]
#![feature(try_blocks)]
#![feature(array_chunks)]
#![feature(option_insert, stmt_expr_attributes)]


#[macro_use]
extern crate serde;
#[macro_use]
extern crate shrinkwraprs;
#[macro_use]
extern crate tracing;

use clap::{AppSettings, SubCommand};
#[cfg(target_env = "gnu")]
use jemallocator::Jemalloc;
use tracing_subscriber::{EnvFilter, FmtSubscriber};
use std::panic::PanicInfo;

#[macro_use]
pub mod error;
#[macro_use]
pub mod db;
#[macro_use]
pub mod dispatch;
pub mod about;
pub mod run;
pub mod module;
pub mod util;
pub mod example;

#[doc(hidden)]
#[cfg(target_env = "gnu")]
#[global_allocator]
static GLOBAL: Jemalloc = Jemalloc;

fn main() -> crate::error::Result<()> {
    better_panic::install();

    let pre_hook = std::panic::take_hook();
    let hook =  move |p: &PanicInfo<'_>| {
        if let Err(e) = run::PANIC_ALERT_CHANNEL.send(()) {
            error!("Unable to alert panic watchdog of failure because {}. Aborting...", e);
            std::process::abort();
        }
        pre_hook(p)
    };

    std::panic::set_hook(Box::new(hook));

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Unable to build runtime.");

    rt.block_on(async_main())
}

#[doc(hidden)] // it's a main function
async fn async_main() -> crate::error::Result<()> {
    let _ = dotenv::dotenv()?;
    let sub = FmtSubscriber::builder()
        .with_env_filter(
            EnvFilter::from_env("GLIMBOT_LOG")
        )
        .finish();

    tracing::subscriber::set_global_default(sub)?;
    let matches = clap::App::new(about::BIN_NAME)
        .version(about::VERSION)
        .about(about::LICENSE_HEADER)
        .author(about::AUTHOR_NAME)
        .subcommand(
            SubCommand::with_name("run")
                .about("Starts Glimbot.")
        )
        .subcommand(
            example::subcommand()
        )
        .setting(AppSettings::SubcommandRequired)
        .get_matches()
        ;

    match matches.subcommand() {
        ("run", _) => {
            info!("Starting Glimbot.");
            run::start_bot().await?;
        }
        ("make-config", Some(m)) => {
            example::handle_matches(m).await?;
        }
        _ => unreachable!("Unrecognized command; we should have errored out already.")
    }
    Ok(())
}
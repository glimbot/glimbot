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

//! This module allows users to manage themselves.

use crate::modules::commands::Command;
use serenity::prelude::Context;
use serenity::model::channel::Message;
use crate::dispatch::Dispatch;
use std::borrow::Cow;
use once_cell::unsync::Lazy;
use clap::{App, AppSettings, SubCommand, Arg, ArgMatches};
use crate::util::help_str;
use crate::args::parse_app_matches;
use crate::modules::roles::resolve_role;
use crate::error::{AnyError};
use crate::db::cache::get_cached_connection;
use crate::modules::hook::Error::DeniedWithReason;
use serenity::utils::MessageBuilder;
use crate::modules::Module;

/// ZST struct for processing the `me` command
pub struct Me;

thread_local! {
    static PARSER: Lazy<App<'static, 'static>> = Lazy::new(
        || {
            let role_id = Arg::with_name("role-id")
                .value_name("ROLE")
                .help("Any string interpretable as a Discord role.")
                .takes_value(true)
                .required(true);

            App::new("me")
                .about("Command to allow users to self-manage things like roles.")
                .setting(AppSettings::SubcommandRequiredElseHelp)
                .subcommand(
                    SubCommand::with_name("join-role")
                        .about("Allows a user to join a joinable role. Ask server admins for more info.")
                        .arg(role_id.clone())
                )
                .subcommand(
                    SubCommand::with_name("leave-role")
                        .about("Allows a user to leave a joinable role. Ask server admins for more info.")
                        .arg(role_id.clone())
                )
        }
    );
}

impl Command for Me {
    fn invoke(&self, _disp: &Dispatch, ctx: &Context, msg: &Message, args: Cow<str>) -> super::commands::Result<()> {
        let m: ArgMatches = PARSER.with(|p| parse_app_matches("me", args, p))?;

        let reply = match m.subcommand() {
            (s, Some(m)) => {
                let joining = s == "join-role";
                let guild = msg.guild(ctx).unwrap();
                let rg = guild.read();
                let mut member = rg.member(ctx, msg.author.id).map_err(AnyError::boxed)?;
                let role_str = m.value_of("role-id").unwrap();
                let role = resolve_role(&rg, role_str)?;

                let conn = get_cached_connection(msg.guild_id.unwrap())?;
                let rconn = conn.borrow();
                if !rconn.role_is_joinable(role.id)? {
                    return Err(DeniedWithReason("That role cannot be joined or left without admin intervention.".into()).into())
                }

                if joining {
                    member.add_role(ctx, role.id)?;
                    format!("Joined role {}", &role.name)
                } else {
                    member.remove_role(ctx, role.id)?;
                    format!("Left role {}", &role.name)
                }
            },
            _ => unreachable!()
        };

        msg.channel_id.say(ctx, MessageBuilder::new()
            .push_codeblock_safe(reply, None)
            .build())?;

        Ok(())

    }

    fn help(&self) -> Cow<'static, str> {
        PARSER.with(|p| help_str(&p).into())
    }
}

/// Creates the module which allows users to manage certain aspects about their membership in the Guild.
pub fn me_mod() -> Module {
    Module::with_name("me")
        .with_command(Me)
        .with_sensitivity(false)
        .with_dependency("roles")
}
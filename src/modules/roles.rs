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

//! Module for managing roles and ensuring users can't run restricted (admin-only) commands.

use crate::modules::Module;
use crate::modules::config;
use crate::dispatch::Dispatch;
use serenity::prelude::Context;
use serenity::model::prelude::{Message, Role};
use std::borrow::Cow;
use serenity::model::id::{UserId, RoleId};
use crate::db::cache::get_cached_connection;
use crate::modules::hook::Error::{DeniedWithReason, NeedRole};
use once_cell::unsync::Lazy;
use clap::{App, Arg, AppSettings, SubCommand, ArgMatches};
use crate::error::{AnyError, BotError};
use std::str::{FromStr, ParseBoolError};
use crate::modules::commands::Command;
use crate::args::parse_app_matches;
use crate::modules::config::{fallible_validator};
use serenity::utils::MessageBuilder;
use std::sync::Arc;
use crate::db::GuildConn;
use serenity::model::guild::Guild;
use crate::util::help_str;

static ADMIN_KEY: &'static str = "admin_role";

/// Errors related to role resolution.
#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// There was no such role in this context.
    #[error("No such role could be identified in this guild: {0}")]
    NoSuchRole(Cow<'static, str>),
    /// Some other error occurred
    #[error("{0}")]
    Other(#[from] AnyError)
}

impl BotError for Error {
    fn is_user_error(&self) -> bool {
        matches!(self, Error::NoSuchRole(_))
    }
}

impl From<Error> for super::commands::Error {
    fn from(e: Error) -> Self {
        super::commands::Error::RuntimeFailure(e.into())
    }
}

/// Resolves a string into a role, given the guild to resolve it in.
pub fn resolve_role(guild: &Guild, s: impl AsRef<str>) -> Result<&Role, Error> {
    let parsed = s.as_ref().parse::<RoleId>();
    let real_role = if let Ok(id) = parsed {
        guild.roles.get(&id)
    } else {
        // Maybe it's a name?
        guild.role_by_name(s.as_ref())
    }.ok_or_else(|| Error::NoSuchRole(s.as_ref().to_string().into()))?;
    Ok(real_role)
}

fn role_hook<'a, 'b, 'c, 'd>(disp: &'a Dispatch, ctx: &'b Context, msg: &'c Message, name: Cow<'d, str>) -> super::hook::Result<Cow<'d, str>> {
    trace!("Applying role hook.");
    let guild = msg.guild_id.unwrap();

    let owner: UserId = ctx.cache.read().guild(guild).unwrap().read().owner_id;
    let author = msg.author.id;
    if owner == author {
        trace!("User is server owner.");
        return Ok(name);
    }

    let conn = get_cached_connection(guild)?;
    let rconn = conn.as_ref().borrow();



    // Now we need to see if the desired command is sensitive or not.
    let module = disp.modules().get(name.as_ref()).ok_or(DeniedWithReason("No such command.".into()))?;
    if module.sensitive || {
        rconn.as_ref().query_row(
            "SELECT ? IN restricted_commands;",
            params![name.as_ref()],
            |r| r.get(0),
        ).map_err(crate::db::DatabaseError::SQLError)?
    } {
        let admin_role = disp.get_config(&rconn, ADMIN_KEY)?.parse::<RoleId>().unwrap().into();
        if msg.author.has_role(ctx, guild, admin_role)? {
            trace!("User is admin.");
            return Ok(name);
        }
        trace!("Command is sensitive and user is not admin or owner.");
        let full_guild = ctx.cache.read().guild(guild).unwrap();
        let role_name = full_guild.read().roles.get(&admin_role).ok_or(DeniedWithReason("Not an admin or admin role outdated.".into()))?.name.clone();

        let needed_role = vec![role_name];
        Err(NeedRole(needed_role))
    } else {
        trace!("Command not sensitive.");
        Ok(name)
    }
}

thread_local! {
static PARSER: Lazy<App<'static, 'static>> = Lazy::new(
    || {
        let role_id = Arg::with_name("role-id")
            .value_name("ROLE")
            .help("Any string interpretable as a Discord role snowflake.")
            .takes_value(true)
            .required(true);

        let user_id = Arg::with_name("user-id")
            .value_name("USER")
            .help("Any string interpretable as a Discord user snowflake.")
            .takes_value(true)
            .required(true);

        App::new("roles")
            .arg(role_id.clone())
            .about("Command for administering user roles. Non-admins probably want the \"me\" command")
            .setting(AppSettings::SubcommandRequiredElseHelp)
            .subcommand(SubCommand::with_name("add-user")
                .arg(user_id.clone())
                .about("Adds a role to a user."))
            .subcommand(SubCommand::with_name("rem-user")
                .arg(user_id.clone())
                .about("Removes a role from a user.")
            )
            .subcommand(
            SubCommand::with_name("set-joinable")
                .arg(Arg::with_name("joinable")
                    .validator(fallible_validator::<bool, ParseBoolError>)
                    .help("Whether or not the role should be joinable using `join-role` and leaveable using `leave-role`")
                    .value_name("JOINABLE")
                    .default_value("true")
                    .required(false)
                    )
                    .about("Sets a role as user joinable.")
            )
    }
);
}

///
pub struct Roles;

impl Command for Roles {
    fn invoke(&self, _disp: &Dispatch, ctx: &Context, msg: &Message, args: Cow<str>) -> super::commands::Result<()> {
        let m: ArgMatches = PARSER.with(|p|
            parse_app_matches("roles", args, &p)
        )?;

        let role = m.value_of("role-id").unwrap();
        let guild = msg.guild(ctx).unwrap();
        let rg = guild.read();

        let role_id = {
            let real_role = resolve_role(&rg, role).map_err(|_| DeniedWithReason("No such role.".into()))?;
            real_role.id
        };

        let reply = match m.subcommand() {
            ("set-joinable", Some(m)) => {
                let joinable = m.value_of("joinable").unwrap().parse::<bool>().unwrap();
                let c = get_cached_connection(guild.read().id)?;
                let cr = c.borrow();
                let sql = if joinable {
                    "INSERT OR IGNORE INTO joinable_roles VALUES (?);"
                } else {
                    "DELETE FROM joinable_roles WHERE role = ?;"
                };

                cr.as_ref().execute(
                    sql,
                    params![role_id.0 as i64]
                ).map_err(crate::db::DatabaseError::SQLError)?;

                "Role updated."
            },
            (s, Some(m)) => {
                let user = m.value_of("user-id").unwrap();
                let parsed = UserId::from_str(user);
                let real_user_id = if let Ok(id) = parsed {
                    id
                } else {
                    rg.member_named(user).ok_or_else(|| DeniedWithReason("No such user.".into()))?.user.read().id
                };

                let adding = s == "add-user";
                let mut member = rg.member(ctx, real_user_id)?;
                if adding {
                    member.add_role(ctx, role_id)?;
                    "Added role to user."
                } else {
                    member.remove_role(ctx, role_id)?;
                    "Removed role from user."
                }
            },
            _ => unreachable!()
        };

        let reply = MessageBuilder::new()
            .push_codeblock_safe(reply, None)
            .build();

        msg.channel_id.say(ctx, reply)?;

        Ok(())
    }

    fn help(&self) -> Cow<'static, str> {
        PARSER.with(|p| help_str(&p).into())
    }
}

/// Checks the validity of a numerical role id
pub fn valid_role(_disp: &Dispatch, ctx: &Context, conn: &GuildConn, s: &str) -> bool {
    let id = *conn.as_id();
    let guild = ctx.cache.read().guild(id);
    if let Some(g) = guild {
        let rg = g.read();
        let parsed = RoleId::from_str(s);
        if let Ok(id) = parsed {
            rg.roles.get(&id).is_some()
        } else {
            false
        }
    } else {
        false
    }
}

/// Creates a roles [Module].
pub fn roles_module() -> Module {
    Module::with_name("roles")
        .with_sensitivity(true)
        .with_dependency("config")
        .with_config_value(config::Value::new(
            ADMIN_KEY,
            "The role which should be allowed to run restricted commands.",
            Arc::new(valid_role),
            Option::<String>::None,
        ))
        .with_command_hook(role_hook)
        .with_command(Roles)
}
use diesel::{ QueryDsl, ExpressionMethods, insert_or_ignore_into, RunQueryDsl, update, delete, QueryResult};
use serenity::model::channel::Message;
use serenity::prelude::Context;
use serenity::utils::{content_safe, ContentSafeOptions};

use crate::glimbot::GlimDispatch;
use crate::glimbot::modules::command::{Arg, Commander, CommanderError, ArgType};

use super::command::Result;
use crate::glimbot::db::Incrementer;
use serenity::model::prelude::GuildId;
use crate::glimbot::modules::command::CommanderError::{Other, RuntimeError};
use crate::diesel::BoolExpressionMethods;
use crate::glimbot::util::say_codeblock;
use diesel::result::Error;
use crate::glimbot::modules::{ModuleBuilder, Module};
use serenity::model::Permissions;

pub const MAX_NAME_LEN: usize = 100;

/// Args: <name> <initial value|optional>
fn create_incrementer(disp: &GlimDispatch, _cmd: &Commander, g: GuildId, ctx: &Context, msg: &Message, args: &[Arg]) -> Result<()> {
    use crate::glimbot::schema::incrementers::dsl::*;
    let inc_name = String::from(args[0].clone());
    let cleaned = content_safe(&ctx, inc_name, &ContentSafeOptions::default());

    let init = if args.len() > 1 {
        i64::from(args[1].clone())
    } else {
        0
    };

    let inc = Incrementer::with_count(g, cleaned.clone(), init);

    if incrementers.
        count().
        filter(guild_id.eq(inc.guild_id).and(name.eq(&inc.name)))
        .get_result::<i64>(&disp.rd_conn()).map_err(|_| Other)? == 0 {
        let new_rows = insert_or_ignore_into(incrementers)
            .values(inc)
            .execute(disp.wr_conn().lock().as_ref()).map_err(|_| Other)?;
        if new_rows > 0 {
            say_codeblock(ctx, msg.channel_id, format!("Created new incrementer {} with count {}", cleaned, init));
        } else {
            say_codeblock(ctx, msg.channel_id, "An incrementer already exists with that name.");
        }

        Ok(())
    } else {
        say_codeblock(ctx, msg.channel_id, "An incrementer already exists with that name.");
        Ok(())
    }
}

/// Args: <name>
fn increment(disp: &GlimDispatch, _cmd: &Commander, g: GuildId, ctx: &Context, msg: &Message, args: &[Arg]) -> Result<()> {
    use crate::glimbot::schema::incrementers::dsl::*;
    let inc_name = String::from(args[0].clone());
    let cleaned = content_safe(&ctx, inc_name, &ContentSafeOptions::default());

    trace!("Starting increment");
    let counter_value = {
        let conn = disp.wr_conn().lock();
        let res: diesel::result::QueryResult<i64> = incrementers
            .select(count)
            .filter(guild_id.eq(g.0 as i64)
                .and(name.eq(&cleaned)))
            .get_result(conn.as_ref());

        if let Err(e) = res {
            return match e {
                Error::NotFound =>
                    Err(CommanderError::RuntimeError(format!("No incrementer called {}", &cleaned))),
                e => Err(CommanderError::silent(e))
            };
        }

        let cur_value = res.unwrap() + 1;

        update(incrementers)
            .filter(guild_id.eq(g.0 as i64)
                .and(name.eq(&cleaned)))
            .set(count.eq(cur_value))
            .execute(conn.as_ref()).map_err(CommanderError::silent)?;
        cur_value
    };
    trace!("End increment");

    say_codeblock(ctx, msg.channel_id, format!("{} is now {}", &cleaned, counter_value));
    trace!("Incremented {} in guild {}; new value {}", &cleaned, g, counter_value);
    Ok(())
}

fn get_incrementer_value(disp: &GlimDispatch, _cmd: &Commander, g: GuildId, ctx: &Context, msg: &Message, args: &[Arg]) -> Result<()> {
    use crate::glimbot::schema::incrementers::dsl::*;

    let inc_name = String::from(args[0].clone());
    let cleaned  = content_safe(&ctx, inc_name, &ContentSafeOptions::default());

    let value: i64 = incrementers
        .select(count)
        .filter(guild_id.eq(g.0 as i64).and(name.eq(&cleaned)))
        .get_result(&disp.rd_conn())
        .map_err(|e: Error| match e {
            Error::NotFound => RuntimeError(format!("No incrementer called {}", &cleaned)),
            e => CommanderError::silent(e)
        })?;

    say_codeblock(ctx, msg.channel_id, format!("{} incremented {} time(s)", &cleaned, value));
    Ok(())
}


fn list_incrementers(disp: &GlimDispatch, _cmd: &Commander, g: GuildId, ctx: &Context, msg: &Message, _args: &[Arg]) -> Result<()> {
    use crate::glimbot::schema::incrementers::dsl::*;

    let incs: Vec<String> = incrementers.select(name)
        .filter(guild_id.eq(g.0 as i64))
        .load(&disp.rd_conn())
        .map_err(CommanderError::silent)?;

    let s = if incs.len() > 0 {
        String::from("Incrementers:\n    ") + &incs.join("\n    ")
    } else {
        String::from("There are no incrementers.")
    };

    say_codeblock(ctx, msg.channel_id, s);
    Ok(())
}

fn delete_incrementer(disp: &GlimDispatch, _cmd: &Commander, g: GuildId, ctx: &Context, msg: &Message, args: &[Arg]) -> Result<()> {
    use crate::glimbot::schema::incrementers::dsl::*;

    let inc_name = String::from(args[0].clone());
    let cleaned = content_safe(&ctx, inc_name, &ContentSafeOptions::default());

    if incrementers.
        count()
        .filter(guild_id.eq(g.0 as i64).and(name.eq(&cleaned)))
        .get_result::<i64>(&disp.rd_conn()).map_err(CommanderError::silent)? != 0 {
        let res: QueryResult<usize> = delete(incrementers)
            .filter(guild_id.eq(g.0 as i64).and(name.eq(&cleaned)))
            .execute(disp.wr_conn().lock().as_ref());

        match res {
            Ok(affected) => {
                if affected > 0 {
                    say_codeblock(ctx, msg.channel_id, format!("Deleted {}", &cleaned));
                } else {
                    say_codeblock(ctx, msg.channel_id, format!("No incrementer called {}", &cleaned));
                }
            },
            Err(e) => match e {
                Error::NotFound => say_codeblock(ctx, msg.channel_id, format!("No incrementer called {}", &cleaned)),
                e => return Err(CommanderError::silent(e))
            },
        };
    } else {
        say_codeblock(ctx, msg.channel_id, format!("No incrementer called {}", &cleaned));
    }

    Ok(())

}

pub fn incrementer_module() -> Module {
    ModuleBuilder::new("incrementer")
        .with_command(
            Commander::new(
                "inc_new",
                Some("Creates a new incrementer with the optional value if given, otherwise 0."),
                vec!["name", "initial_value"],
                vec![ArgType::Str],
                vec![ArgType::Int],
                Permissions::SEND_MESSAGES,
                create_incrementer
            )
        )
        .with_command(
            Commander::new(
                "inc",
                Some("Increments an incrementer."),
                vec!["name"],
                vec![ArgType::Str],
                vec![],
                Permissions::SEND_MESSAGES,
                increment
            )
        )
        .with_command(
            Commander::new(
                "inc_get",
                Some("Retrieves the value of an incrementer."),
                vec!["name"],
                vec![ArgType::Str],
                vec![],
                Permissions::SEND_MESSAGES,
                get_incrementer_value
            )
        )
        .with_command(
            Commander::new(
                "inc_list",
                Some("Retrieves the names of all incrementers."),
                Vec::<String>::new(),
                vec![],
                vec![],
                Permissions::SEND_MESSAGES,
                list_incrementers
            )
        )
        .with_command(
            Commander::new(
                "inc_del",
                Some("Deletes the incrementer with the given name."),
                vec!["name"],
                vec![ArgType::Str],
                vec![],
                Permissions::SEND_MESSAGES,
                delete_incrementer
            )
        )
        .build()
}
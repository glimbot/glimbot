use std::result::Result as StdRes;

use diesel::{Connection, delete, insert_into, insert_or_ignore_into, RunQueryDsl};
use diesel::result::Error;
use serenity::model::Permissions;
use serenity::model::prelude::{GuildId, Message};
use serenity::prelude::Context;
use serenity::utils::{content_safe, ContentSafeOptions, MessageBuilder};
use thiserror::Error as ThisErr;

use crate::diesel::ExpressionMethods;
use crate::diesel::QueryDsl;
use crate::glimbot::GlimDispatch;
use crate::glimbot::modules::{Module, ModuleBuilder};
use crate::glimbot::modules::command::{Arg, ArgType, Commander, };
use crate::glimbot::modules::command::CommanderError::{RuntimeError, };
use crate::glimbot::util::{say_codeblock};

use super::command::Result;

const BAG_MAX_ITEMS: usize = 10;
const BAG_ITEM_MAX_SIZE: usize = 50;

#[derive(ThisErr, Debug, Clone)]
pub enum BagError {
    #[error("The bag is full. It can only hold {0} items.")]
    BagFull(usize),
    #[error("The bag is empty.")]
    BagEmpty,
}

pub fn bag_module_config(disp: &GlimDispatch, g: GuildId) {
    use crate::glimbot::schema::bag_configs::dsl::*;
    if bag_configs.count().filter(guild_id.eq(g.0 as i64)).get_result::<i64>(&disp.rd_conn()).unwrap() == 0 {
        let conn = disp.wr_conn().lock();
        insert_or_ignore_into(bag_configs).values(guild_id.eq(g.0 as i64)).execute(conn.as_ref()).unwrap();
    }
}

fn bag_add(disp: &GlimDispatch, _cmd: &Commander, g: GuildId, ctx: &Context, msg: &Message, args: &[Arg]) -> Result<()> {
    use crate::glimbot::schema::bag_items::dsl::*;
    let item = String::from(args[0].clone());
    let cleaned_item = content_safe(&ctx, &item, &ContentSafeOptions::default());

    if cleaned_item.len() > BAG_ITEM_MAX_SIZE {
        return Err(RuntimeError("Item is too big!".to_string()));
    };

    disp.ensure_module_config(g, "bag");
    let res = insert_into(bag_items).values((guild_id.eq(g.0 as i64), name.eq(cleaned_item)))
        .execute(disp.wr_conn.lock().as_ref());

    if let Err(e) = res {
        let message = match &e {
            Error::DatabaseError(_k, i) => {i.message()},
            _ => {panic!("{}", e)}
        };
        say_codeblock(ctx, msg.channel_id, message);
    } else {
        say_codeblock(ctx, msg.channel_id, "Added item to bag.");
    }

    Ok(())
}

fn bag_show(disp: &GlimDispatch, _cmd: &Commander, g: GuildId, ctx: &Context, msg: &Message, _args: &[Arg]) -> Result<()> {
    disp.ensure_module_config(g, "bag");
    use crate::glimbot::schema::bag_items::dsl::*;

    let conn = disp.rd_conn();
    let local: Vec<String> = bag_items
        .select(name)
        .filter(guild_id.eq(g.0 as i64))
        .load(&conn).unwrap();

    let message = if local.len() > 0 {
        MessageBuilder::new()
            .push_line("Bag contains:")
            .push("    ".to_string() + &local.join("\n    "))
            .build()
    } else {
        "The bag is empty.".to_string()
    };

    say_codeblock(&ctx, msg.channel_id, message);
    Ok(())
}

no_arg_sql_function!(RANDOM, (), "Represents the sql RANDOM() function");

fn bag_yeet(disp: &GlimDispatch, _cmd: &Commander, g: GuildId, ctx: &Context, msg: &Message, _args: &[Arg]) -> Result<()> {
    disp.ensure_module_config(g, "bag");
    use crate::glimbot::schema::bag_items::dsl::*;

    let res: StdRes<String, diesel::result::Error> = {
        let conn = disp.wr_conn().lock();
        conn.transaction(|| {
            let item: Vec<(i32, String)> = bag_items
                .select((id, name))
                .filter(guild_id.eq(g.0 as i64))
                .order(RANDOM)
                .limit(1)
                .load(conn.as_ref())?;

            let row_id = &item[0].0;
            let i = item[0].1.to_owned();

            delete(bag_items.filter(id.eq(row_id))).execute(conn.as_ref())?;
            Ok(i)
        })
    };

    let message = match res {
        Ok(s) => {format!("Yeeted a(n) {}", s)},
        Err(e) => {format!("{}", e)},
    };

    say_codeblock(ctx, msg.channel_id, message);
    Ok(())
}

pub fn bag_module() -> Module {
    ModuleBuilder::new("bag")
        .with_command(
            Commander::new(
                "bag_add",
                Some("Adds an item to the bag (if it's not full)."),
                vec!["item"],
                vec![ArgType::Str],
                vec![],
                Permissions::SEND_MESSAGES,
                bag_add,
            )
        )
        .with_command(
            Commander::new(
                "bag_yeet",
                Some("Chucks an item randomly from the bag (if present)."),
                Vec::<String>::new(),
                vec![],
                vec![],
                Permissions::SEND_MESSAGES,
                bag_yeet,
            )
        )
        .with_command(
            Commander::new(
                "bag_show",
                Some("Shows what's in the bag."),
                Vec::<String>::new(),
                vec![],
                vec![],
                Permissions::SEND_MESSAGES,
                bag_show,
            )
        )
        .with_default_config_gen(bag_module_config)
        .build()
}


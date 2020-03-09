use crate::glimbot::modules::{ModuleConfig, Module, ModuleBuilder};
use serde_yaml::{Value, Sequence};
use crate::glimbot::GlimDispatch;
use crate::glimbot::modules::command::{Commander, Arg, ArgType, CommanderError};
use crate::glimbot::guilds::RwGuildPtr;
use serenity::prelude::Context;
use serenity::model::prelude::Message;
use super::command::Result;
use serenity::model::Permissions;
use serenity::utils::{content_safe, ContentSafeOptions, MessageBuilder};
use crate::glimbot::modules::command::CommanderError::{RuntimeError, Silent};
use crate::glimbot::util::{FromError, say_codeblock};
use parking_lot::RwLockUpgradableReadGuard;
use rand::prelude::*;

const BAG_MAX_ITEMS: usize = 10;
const BAG_ITEM_MAX_SIZE: usize = 50;

pub fn bag_module_config() -> ModuleConfig {
    let mut out = ModuleConfig::new();
    out.insert("bag".to_string(), Value::Sequence(Sequence::new()));
    out
}

fn bag_add(disp: &GlimDispatch, _cmd: &Commander, g: &RwGuildPtr, ctx: &Context, msg: &Message, args: &[Arg]) -> Result<()> {
    let item = String::from(args[0].clone());
    let cleaned_item = content_safe(&ctx, &item, &ContentSafeOptions::default());

    if cleaned_item.len() > BAG_ITEM_MAX_SIZE {
        return Err(RuntimeError("Item is too big!".to_string()));
    };

    let mod_config = {
        let gid = g.read().guild;
        disp.ensure_module_config(gid, "bag")
    };


    let rug = mod_config.upgradable_read();
    let changed = if rug.get("bag").unwrap().as_sequence().unwrap().len() < BAG_MAX_ITEMS {
        let mut wrg = RwLockUpgradableReadGuard::upgrade(rug);
        let seq = wrg.get_mut("bag").unwrap().as_sequence_mut().unwrap();
        seq.push(Value::from(cleaned_item));
        say_codeblock(&ctx, msg.channel_id, "Added item to bag.").map_err(|_e| Silent)?;
        true
    } else {
        say_codeblock(&ctx, msg.channel_id, "Bag is too full!")
            .map_err(|_e| Silent)?;
        false
    };

    if changed {
        g.write().commit_to_disk();
    };

    Ok(())
}

fn bag_show(disp: &GlimDispatch, cmd: &Commander, g: &RwGuildPtr, ctx: &Context, msg: &Message, args: &[Arg]) -> Result<()> {
    let mod_config = {
        let gid = g.read().guild;
        disp.ensure_module_config(gid, "bag")
    };

    let rug = mod_config.read();
    let contents = rug.get("bag").unwrap().as_sequence().unwrap();
    let local: Vec<String> = contents.iter().map(Value::as_str)
        .map(Option::unwrap)
        .map(String::from)
        .collect();
    std::mem::drop(rug);

    let message = if local.len() > 0 {
        MessageBuilder::new()
            .push_line("Bag contains:")
            .push("    ".to_string() + &local.join("\n    "))
            .build()
    } else {
        "The bag is empty.".to_string()
    };

    say_codeblock(&ctx, msg.channel_id, message).map_err(|_| Silent)?;
    Ok(())
}

fn bag_yeet(disp: &GlimDispatch, cmd: &Commander, g: &RwGuildPtr, ctx: &Context, msg: &Message, args: &[Arg]) -> Result<()> {
    let mod_config = {
        let gid = g.read().guild;
        disp.ensure_module_config(gid, "bag")
    };

    let rug = mod_config.upgradable_read();
    let changed = if !rug.get("bag").unwrap().as_sequence().unwrap().is_empty() {
        let mut wrg = RwLockUpgradableReadGuard::upgrade(rug);
        let seq = wrg.get_mut("bag").unwrap().as_sequence_mut().unwrap();
        let mut rng = rand::thread_rng();
        let idx = rng.gen_range(0, seq.len());
        let item = seq.remove(idx);
        say_codeblock(&ctx, msg.channel_id,
                      format!("Yeeted a(n) {}", item.as_str().unwrap()))
            .map_err(|_e| Silent)?;
        true
    } else {
        say_codeblock(&ctx, msg.channel_id, "The bag is empty.")
            .map_err(|_e| Silent)?;
        false
    };

    if changed {
        g.write().commit_to_disk();
    };
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


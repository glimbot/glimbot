use crate::glimbot::modules::{ModuleConfig, Module, ModuleBuilder};
use serde_yaml::{Value, Sequence};
use crate::glimbot::GlimDispatch;
use crate::glimbot::modules::command::{Commander, Arg, ArgType, CommanderError};
use crate::glimbot::guilds::RwGuildPtr;
use serenity::prelude::Context;
use serenity::model::prelude::Message;
use super::command::Result;
use serenity::model::Permissions;
use thiserror::Error as ThisErr;
use serenity::utils::{content_safe, ContentSafeOptions, MessageBuilder};
use crate::glimbot::modules::command::CommanderError::{RuntimeError, Silent};
use crate::glimbot::util::{FromError, say_codeblock};
use parking_lot::RwLockUpgradableReadGuard;
use rand::prelude::*;
use std::result::Result as StdRes;
use serde::{Deserialize, Serialize};

const BAG_MAX_ITEMS: usize = 10;
const BAG_ITEM_MAX_SIZE: usize = 50;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BagConfig {
    bag: Vec<String>,
    capacity: usize,
}

impl AsRef<BagConfig> for ModuleConfig {
    fn as_ref(&self) -> &BagConfig {
        match self {
            ModuleConfig::Bag(b) => { b }
            e => panic!("Got {:?}, expected Bag", e)
        }
    }
}

impl AsMut<BagConfig> for ModuleConfig {
    fn as_mut(&mut self) -> &mut BagConfig {
        match self {
            ModuleConfig::Bag(b) => { b }
            e => panic!("Got {:?}, expected Bag", e)
        }
    }
}

#[derive(ThisErr, Debug, Clone)]
pub enum BagError {
    #[error("The bag is full. It can only hold {0} items.")]
    BagFull(usize),
    #[error("The bag is empty.")]
    BagEmpty,
}

impl BagConfig {
    pub fn new() -> BagConfig {
        BagConfig {
            bag: Vec::new(),
            capacity: BAG_MAX_ITEMS,
        }
    }

    pub fn add_item(&mut self, val: impl Into<String>) -> StdRes<(), BagError> {
        if self.capacity > self.bag.len() {
            self.bag.push(val.into());
            Ok(())
        } else {
            Err(BagError::BagFull(self.capacity))
        }
    }

    pub fn remove_item_rand(&mut self) -> StdRes<String, BagError> {
        if self.bag.is_empty() {
            Err(BagError::BagEmpty)
        } else {
            let mut rng = rand::thread_rng();
            let idx = rng.gen_range(0, self.bag.len());
            Ok(self.bag.remove(idx))
        }
    }

    pub fn item_can_be_added(&self) -> bool {
        self.bag.len() < self.capacity
    }

    pub fn items(&self) -> &[String] {
        &self.bag
    }
}

pub fn bag_module_config() -> ModuleConfig {
    ModuleConfig::Bag(BagConfig::new())
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


    let res = {
        let mut guard = mod_config.write();
        let bag: &mut BagConfig = guard.as_mut();
        bag.add_item(cleaned_item)
    };

    if let Err(e) = res {
        say_codeblock(ctx, msg.channel_id, e).map_err(|_| Silent)?;
    } else {
        say_codeblock(ctx, msg.channel_id, "Added item to bag.").map_err(|_| Silent)?;
        g.write().commit_to_disk();
    }

    Ok(())
}

fn bag_show(disp: &GlimDispatch, cmd: &Commander, g: &RwGuildPtr, ctx: &Context, msg: &Message, args: &[Arg]) -> Result<()> {
    let mod_config = {
        let gid = g.read().guild;
        disp.ensure_module_config(gid, "bag")
    };

    let local: Vec<_> = {
        let rug = mod_config.read();
        let bag: &BagConfig = rug.as_ref();
        bag.items().iter().cloned().collect()
    };

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

    let res = {
        let mut guard = mod_config.write();
        let bag: &mut BagConfig = guard.as_mut();
        bag.remove_item_rand()
    };

    let changed = res.is_ok();
    let message = match res {
        Ok(s) => {s},
        Err(e) => {format!("{}", e)},
    };

    if changed {
        g.write().commit_to_disk();
    };

    say_codeblock(ctx, msg.channel_id, message).map_err(|_| Silent)?;
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


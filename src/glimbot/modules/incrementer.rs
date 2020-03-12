use diesel::insert_into;
use serenity::model::channel::Message;
use serenity::prelude::Context;
use serenity::utils::{content_safe, ContentSafeOptions};

use crate::glimbot::GlimDispatch;
use crate::glimbot::modules::command::{Arg, Commander};
use crate::glimbot::schema::incrementers::dsl::*;

use super::command::Result;

pub const MAX_NAME_LEN: usize = 100;

// pub fn create_incrementer(disp: &GlimDispatch, cmd: &Commander, _g: &RwGuildPtr, ctx: &Context, msg: &Message, args: &[Arg]) -> Result<()> {
//     let inc_name = String::from(&args[0].clone());
//     let cleaned = content_safe(&ctx, inc_name, &ContentSafeOptions::default());
//     let conn = disp.get_db_conn();
//     insert_into(incrementers).values().on_conflict_do_nothing();
// }
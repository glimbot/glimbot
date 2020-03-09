use std::error::Error;
use serenity::prelude::Context;
use serenity::model::id::ChannelId;
use std::fmt::Display;
use serenity::utils::MessageBuilder;
use serenity::model::prelude::Message;
use serenity::Result;

pub mod rate_limit;
pub mod snowflakes;

pub trait FromError {
    fn from_error(e: impl Error + 'static) -> Self;
}

pub fn say_codeblock(ctx: &Context, chan: ChannelId, d: impl Display) -> Result<Message> {
    chan.say(ctx, MessageBuilder::new()
        .push_codeblock_safe(d, None)
        .build())
}
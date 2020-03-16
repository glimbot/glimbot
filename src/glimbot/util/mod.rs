use std::error::Error;
use std::fmt::Display;

use serenity::model::id::ChannelId;
use serenity::model::prelude::Message;
use serenity::prelude::Context;
use serenity::Result;
use serenity::utils::MessageBuilder;

pub mod rate_limit;
pub mod snowflakes;

pub const MESSAGE_BYTE_LIMIT: usize = 2000;

pub trait FromError {
    fn from_error(e: impl Error + 'static) -> Self;
}

pub fn say_codeblock(ctx: &Context, chan: ChannelId, d: impl Display) {
    let s = d.to_string();
    let res = if s.len() > MESSAGE_BYTE_LIMIT {
        let mut split = s.split("\n");
        split.try_fold(String::new(), |mut acc, line| {
            if acc.len() + line.len() + 7 > MESSAGE_BYTE_LIMIT {
                let s = MessageBuilder::new()
                    .push_codeblock(&acc, None)
                    .build();
                chan.say(ctx, s).map(|_| {
                    acc.clear();
                    acc
                })
            } else {
                acc.push_str(line);
                acc.push('\n');
                Ok(acc)
            }
        }).map(|s| say_codeblock(ctx, chan, s))
    } else {
        chan.say(ctx, MessageBuilder::new()
            .push_codeblock_safe(d, None)
            .build()).map(|_| ())
    };

    if let Err(e) = &res {
        error!("Couldn't send message for some reason: {}", e);
    };
}
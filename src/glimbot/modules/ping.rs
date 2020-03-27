use log::error;
use log::trace;
use serenity::model::Permissions;
use serenity::model::prelude::{GuildId, Message};
use serenity::prelude::Context;
use serenity::utils::{content_safe, ContentSafeOptions};
use serenity::utils::MessageBuilder;

use crate::glimbot::GlimDispatch;
use crate::glimbot::modules::{Module, ModuleBuilder};
use crate::glimbot::modules::command::*;
use crate::glimbot::modules::command::ArgType::Str;
use crate::glimbot::modules::command::CommanderError::{OtherError};

fn ping(_d: &GlimDispatch, _cmd: &Commander, _g: GuildId, ctx: &Context, msg: &Message, args: &[Arg]) -> Result<()> {
    let response =
        if args.len() > 0 {
            if let Arg::Str(s) = &args[0] {
                MessageBuilder::new()
                    .push_quote_line_safe(s)
                    .push("— ")
                    .push_italic_line(msg.author_nick(ctx).unwrap_or_else(
                        || msg.author.name.clone()
                    ))
                    .build()
            } else {
                return Err(CommanderError::BadParameter(0, Str));
            }
        } else {
            "Echo!".to_string()
        };

    let response = content_safe(&ctx, response,
                                &ContentSafeOptions::default());

    trace!("{}", &response);

    let res = msg.channel_id.say(ctx, response);

    if let Err(e) = res {
        error!("{:?}", e);
        Err(OtherError(Box::new(e)))
    } else {
        Ok(())
    }
}

pub fn ping_module() -> Module {
    ModuleBuilder::new("echo")
        .with_command(Commander::new(
            "echo",
            Some("Responds with pong OR echoes the single optional argument."),
            vec!["echo"],
            vec![],
            vec![ArgType::Str],
            Permissions::SEND_MESSAGES,
            ping,
        ))
        .build()
}


#[cfg(test)]
mod tests {
    #[test]
    fn check_ping_help() {}
}
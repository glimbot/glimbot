use crate::glimbot::modules::command::*;
use once_cell::sync::Lazy;
use serenity::prelude::Context;
use serenity::model::prelude::Message;
use crate::glimbot::modules::command::ArgType::Str;
use log::error;
use crate::glimbot::modules::command::CommanderError::{Other, OtherError};
use crate::glimbot::guilds::{GuildContext, RwGuildPtr};
use serenity::model::Permissions;
use crate::glimbot::modules::{Module, ModuleBuilder};

fn ping(_cmd: &Commander, _g: &RwGuildPtr, ctx: &Context, msg: &Message, args: &[Arg]) -> Result<()> {
    let response =
        if args.len() > 0 {
            if let Arg::Str(s) = &args[0] {
                s
            } else {
                return Err(CommanderError::BadParameter(0, Str));
            }
        } else {
            "Pong!"
        };

    let res = msg.channel_id.say(ctx, response);

    if let Err(e) = res {
        error!("{:?}", e);
        Err(OtherError(Box::new(e)))
    } else {
        Ok(())
    }
}

pub fn ping_module() -> Module {
    ModuleBuilder::new("ping")
        .with_command(Commander::new(
            "ping",
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
    use super::*;

    #[test]
    fn check_ping_help() {}
}
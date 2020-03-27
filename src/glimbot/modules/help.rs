use log::error;
use log::trace;
use serenity::model::id::GuildId;
use serenity::model::Permissions;
use serenity::model::prelude::Message;
use serenity::prelude::Context;
use serenity::utils::MessageBuilder;

use crate::glimbot::GlimDispatch;
use crate::glimbot::modules::{Module, ModuleBuilder};
use crate::glimbot::modules::command::*;
use crate::glimbot::modules::command::CommanderError::{OtherError};

const PER_MESSAGE_BYTE_LIM: usize = 2000;

fn help(disp: &GlimDispatch, cmd: &Commander, _g: GuildId, ctx: &Context, msg: &Message, args: &[Arg]) -> Result<()> {
    let response =
        if args.len() > 0 {
            let cmd = String::from((&args[0]).clone());
            let m = disp.resolve_command(&cmd)
                .map_or_else(||format!("No such command: {}", &cmd), |s| s.help_msg());
            vec![m]
        } else {
            let commands = disp.command_map.keys();
            let mut v = Vec::<String>::new();
            v.push("Available commands:".to_string());
            commands.cloned().map(|s: String| { "    ".to_string() + &s }).for_each(
                |s| v.push(s)
            );
            v
        };

    let rem = response.iter().try_fold(String::new(), |mut acc, line| {
        if acc.len() + line.len() + 7 > PER_MESSAGE_BYTE_LIM {
            let s = MessageBuilder::new()
                .push_codeblock(&acc, None)
                .build();
            msg.channel_id.say(ctx, s).map(|_| {
                acc.clear();
                acc
            })
        } else {
            acc.push_str(line);
            acc.push('\n');
            Ok(acc)
        }
    })?;

    trace!("{}", &rem);


    let res = msg.channel_id.say(ctx,
                                 MessageBuilder::new().
                                     push_codeblock(rem.trim_end(), None)
                                     .build());

    if let Err(e) = res {
        error!("{:?}", e);
        Err(OtherError(Box::new(e)))
    } else {
        Ok(())
    }
}

pub fn help_module() -> Module {
    ModuleBuilder::new("help")
        .with_command(Commander::new(
            "help",
            Some("Gets info about available commands. Aliases excluded."),
            vec!["command"],
            vec![],
            vec![ArgType::Str],
            Permissions::SEND_MESSAGES,
            help,
        ))
        .build()
}
use crate::glimbot::{GlimDispatch, EventHandler};
use crate::glimbot::modules::command::{Commander, Arg, CommanderError, ArgType};
use serenity::model::id::GuildId;
use serenity::prelude::Context;
use serenity::model::prelude::Message;
use crate::glimbot::modules::command::Result;
use crate::glimbot::modules::dice::parser::parse_roll;
use crate::glimbot::util::{FromError, say_codeblock};
use crate::glimbot::modules::{ModuleBuilder, Module};
use serenity::model::Permissions;
use crate::glimbot::modules::command::parser::RawCmd;
use serenity::model::event::EventType::MessageCreate;

fn roll(_disp: &GlimDispatch, _cmd: &Commander, _g: GuildId, ctx: &Context, msg: &Message, args: &[Arg]) -> Result<()> {
    let arg = args[0].to_string();
    let roll = parse_roll(&arg)?;
    roll.valid().map_err(CommanderError::from_error)?;
    let res = roll.eval();
    trace!("{}", &res);
    say_codeblock(ctx, msg.channel_id, res);
    Ok(())
}

pub fn command_hook(_disp: &GlimDispatch, _g: GuildId, _ctx: &Context, _msg: &Message, cmd: RawCmd) -> crate::glimbot::modules::command::Result<RawCmd> {
    if &cmd.command != "roll" {
        Ok(cmd)
    } else {
        Ok(RawCmd {
            args: vec![cmd.args.join(" ")],
            command: cmd.command,
            prefix: cmd.prefix,
        })
    }
}

pub fn roll_module() -> Module {
    ModuleBuilder::new("roll")
        .with_command(
            Commander::new(
                "roll",
                Some("Rolls a die with expressions like 5d20 and (100 + 5d2) + 6d7"),
                vec!["dice"],
                vec![ArgType::Str],
                vec![],
                Permissions::SEND_MESSAGES,
                roll,
            )
        )
        .with_hook(MessageCreate, EventHandler::CommandHandler(command_hook))
        .build()
}
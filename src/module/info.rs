use crate::dispatch::Dispatch;
use crate::module::{ModInfo, Module, Sensitivity};
use crate::util::ClapExt;
use itertools::Itertools;
use once_cell::sync::Lazy;
use serenity::client::Context;
use serenity::model::channel::Message;

pub struct HelpModule;

/// Command to get information about commands available in glimbot.
#[derive(Debug, structopt::StructOpt)]
pub struct InfoOpt {
    /// If specified, prints information about the command. If unspecified, lists available commands.
    command: Option<String>,
}

#[async_trait::async_trait]
impl Module for HelpModule {
    fn info(&self) -> &ModInfo {
        static INFO: Lazy<ModInfo> = Lazy::new(|| {
            ModInfo::with_name("info", "get information about available commands.")
                .with_command(true)
                .with_sensitivity(Sensitivity::Low)
        });
        &INFO
    }

    async fn process(
        &self,
        dis: &Dispatch,
        ctx: &Context,
        orig: &Message,
        command: Vec<String>,
    ) -> crate::error::Result<()> {
        let opts = InfoOpt::from_iter_with_help(command)?;
        let msg = if let Some(cmd) = opts.command {
            let module = dis.command_module(&cmd)?;
            let help_str = format!("```{}: {}```", cmd, module.info().short_desc);
            help_str
        } else {
            let cmds = dis.commands().map(|(k, _)| k).join(", ");
            format!("```Available commands: {}```", cmds)
        };

        orig.reply(ctx, msg).await?;
        Ok(())
    }
}

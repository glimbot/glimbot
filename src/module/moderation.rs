use crate::module::{Module, ModInfo, Sensitivity, UnimplementedModule};
use serenity::client::Context;
use serenity::model::channel::Message;
use crate::dispatch::Dispatch;
use crate::dispatch::config::{Value, VerifiedChannel};
use once_cell::sync::Lazy;

pub struct ModerationModule;

#[async_trait::async_trait]
impl Module for ModerationModule {
    fn info(&self) -> &ModInfo {
        static INFO: Lazy<ModInfo> = Lazy::new(|| ModInfo::with_name("mod")
            .with_sensitivity(Sensitivity::High)
            .with_command(true)
            .with_config_value(Value::<VerifiedChannel>::new("mod_log_channel", "Channel for logging moderation actions.")));

        &INFO
    }

    async fn process(&self, _dis: &Dispatch, _ctx: &Context, _orig: &Message, _command: Vec<String>) -> crate::error::Result<()> {
        Err(UnimplementedModule.into())
    }
}
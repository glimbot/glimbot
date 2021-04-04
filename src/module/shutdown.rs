//! Contains the `shutdown` command, an owner-only command to shutdown Glimbot.

use once_cell::sync::Lazy;
use serenity::client::Context;
use serenity::model::channel::Message;

use crate::dispatch::{Dispatch, ShardManKey};
use crate::module::{ModInfo, Module, Sensitivity};

/// Owner-only command to shutdown Glimbot by terminating the shards.
pub struct Shutdown;

#[async_trait::async_trait]
impl Module for Shutdown {
    fn info(&self) -> &ModInfo {
        #[doc(hidden)]
        static INFO: Lazy<ModInfo> = Lazy::new(|| {
            ModInfo::with_name("shutdown", "shuts down glimbot.")
                .with_sensitivity(Sensitivity::Owner)
                .with_command(true)
        });
        &INFO
    }

    async fn process(
        &self,
        _dis: &Dispatch,
        ctx: &Context,
        orig: &Message,
        _command: Vec<String>,
    ) -> crate::error::Result<()> {
        info!("received shutdown command");
        let man = {
            ctx.data
                .read()
                .await
                .get::<ShardManKey>()
                .expect("expected to see the shard manager.")
                .clone()
        };

        let err = orig.reply(ctx, "Shutting down.").await;

        man.lock().await.shutdown_all().await;
        info!("shutdown complete");
        err?;
        Ok(())
    }
}

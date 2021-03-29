//! Contains code to get glimbot dispatch and background service started.

use serenity::client::bridge::gateway::GatewayIntents;

use crate::dispatch::{ArcDispatch, ShardManKey};
use crate::module::status::START_TIME;
use once_cell::sync::Lazy;
use tokio::sync::broadcast;

/// Channel for threads to alert Glimbot that it's panicked.
pub static PANIC_ALERT_CHANNEL: Lazy<(broadcast::Sender<()>, broadcast::Receiver<()>)> = Lazy::new(|| broadcast::channel(100));

/// Starts Glimbot.
/// This is where modules are loaded.
pub async fn start_bot() -> crate::error::Result<()> {

    let pool = crate::db::create_pool().await?;
    let mut dispatch = crate::dispatch::Dispatch::new(std::env::var("GLIMBOT_OWNER").expect("Couldn't find owner information.").parse().expect("Invalid owner token."), pool);
    dispatch.add_module(crate::module::base_filter::BaseFilter);
    dispatch.add_module(crate::module::owner::OwnerFilter);
    dispatch.add_module(crate::module::privilege::PrivilegeFilter);
    dispatch.add_module(crate::module::conf::ConfigModule);
    dispatch.add_module(crate::module::status::StatusModule::default());
    dispatch.add_module(crate::module::roles::RoleModule);
    dispatch.add_module(crate::module::moderation::ModerationModule);
    dispatch.add_module(crate::module::spam::SpamModule::default());
    dispatch.add_module(crate::module::shutdown::Shutdown);
    dispatch.add_module(crate::module::roles::ModRoleModule);
    dispatch.add_module(crate::module::mock_raid::MockRaidModule::default());

    let dispatch = ArcDispatch::from(dispatch);

    let mut client = serenity::Client::builder(std::env::var("GLIMBOT_TOKEN").expect("Didn't find a token."))
        .intents(GatewayIntents::privileged() | GatewayIntents::GUILD_MESSAGES | GatewayIntents::GUILD_BANS | GatewayIntents::GUILDS | GatewayIntents::DIRECT_MESSAGES)
        .event_handler(dispatch)
        .await?;

    let _ = START_TIME.elapsed();
    let shard_man = client.shard_manager.clone();
    let mut dg = client.data.write().await;

    let smc = shard_man.clone();
    let panic_smc = shard_man.clone();
    tokio::spawn(async move {
        // Gracefully handle shutting down due to interrupt.
        tokio::signal::ctrl_c().await.expect("failed to listen for Ctrl + C");
        smc.lock().await.shutdown_all().await;
    });

    tokio::spawn(async move {
        let mut rx = PANIC_ALERT_CHANNEL.0.subscribe();
        if rx.recv().await.is_ok() {
            error!("Glimbot panicked. Shutting down the shard manager.");
            panic_smc.lock().await.shutdown_all().await;
        }
    });

    dg.insert::<ShardManKey>(shard_man);
    std::mem::drop(dg);
    client.start_autosharded().await?;
    Ok(())
}
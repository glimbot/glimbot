use serenity::client::bridge::gateway::GatewayIntents;
use crate::module::status::START_TIME;
use once_cell::sync::Lazy;
use crate::dispatch::ShardManKey;
use crate::db::ensure_db;

// Starts Glimbot.
pub async fn start_bot() -> anyhow::Result<()> {

    let mut dispatch = crate::dispatch::Dispatch::new(std::env::var("GLIMBOT_OWNER").expect("Couldn't find owner information.").parse().expect("Invalid owner token."));
    dispatch.add_module(crate::module::base_filter::BaseFilter);
    dispatch.add_module(crate::module::owner::OwnerFilter);
    dispatch.add_module(crate::module::status::StatusModule);
    dispatch.add_module(crate::module::shutdown::Shutdown);

    let mut client = serenity::Client::builder(std::env::var("GLIMBOT_TOKEN").expect("Didn't find a token."))
        .intents(GatewayIntents::privileged() | GatewayIntents::GUILD_MESSAGES | GatewayIntents::GUILD_BANS | GatewayIntents::GUILDS | GatewayIntents::DIRECT_MESSAGES)
        .event_handler(dispatch)
        .await?;

    let _ = START_TIME.elapsed();
    let shard_man = client.shard_manager.clone();
    let mut dg = client.data.write().await;

    let smc = shard_man.clone();
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.expect("failed to listen for Ctrl + C");
        smc.lock().await.shutdown_all().await;
        tokio::task::spawn_blocking(|| ensure_db().flush().expect("Unable to sync DB."))
    });

    dg.insert::<ShardManKey>(shard_man);
    std::mem::drop(dg);
    client.start_autosharded().await?;
    Ok(())
}
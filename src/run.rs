use serenity::client::bridge::gateway::GatewayIntents;
use crate::module::status::START_TIME;
use once_cell::sync::Lazy;
use crate::dispatch::{ShardManKey, ArcDispatch};

// Starts Glimbot.
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
    dispatch.add_module(crate::module::shutdown::Shutdown);
    dispatch.add_module(crate::module::roles::ModRoleModule);

    let dispatch = ArcDispatch::from(dispatch);

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
    });

    dg.insert::<ShardManKey>(shard_man);
    std::mem::drop(dg);
    client.start_autosharded().await?;
    Ok(())
}
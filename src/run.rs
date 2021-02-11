use serenity::client::bridge::gateway::GatewayIntents;
use crate::module::status::START_TIME;
use once_cell::sync::Lazy;

// Starts Glimbot.
pub async fn start_bot() -> anyhow::Result<()> {

    let mut dispatch = crate::dispatch::Dispatch::new(std::env::var("GLIMBOT_OWNER").expect("Couldn't find owner information.").parse().expect("Invalid owner token."));
    dispatch.add_module(crate::module::owner::OwnerFilter);
    dispatch.add_module(crate::module::status::StatusModule);

    let mut client = serenity::Client::builder(std::env::var("GLIMBOT_TOKEN").expect("Didn't find a token."))
        .intents(GatewayIntents::privileged() | GatewayIntents::GUILD_MESSAGES | GatewayIntents::GUILD_BANS | GatewayIntents::GUILDS | GatewayIntents::DIRECT_MESSAGES)
        .event_handler(dispatch)
        .await?;

    let _ = START_TIME.elapsed();
    client.start_autosharded().await?;
    Ok(())
}
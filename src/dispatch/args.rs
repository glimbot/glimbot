use clap::{App, SubCommand, ArgMatches};
use crate::util::Fallible;
use serenity::Client;

pub fn command_parser() -> App<'static, 'static> {
    SubCommand::with_name("start")
        .about("Starts the Glimbot service.")
}

pub fn handle_matches(m: &ArgMatches) -> Fallible<()> {
    if let ("start", Some(_)) = m.subcommand() {
        let token = std::env::var("GLIMBOT_TOKEN")?;
        let owner = std::env::var("GLIMBOT_OWNER").unwrap_or_default().parse::<u64>()?;
        let dispatch = super::Dispatch::new(owner.into());
        let mut client = Client::new(token, dispatch)?;
        client.start_autosharded()?;
    }

    Ok(())
}
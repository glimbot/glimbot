//! Contains the CLI logic to emit a default configuration file with fill-in-the-blank slots.

use std::borrow::Cow;

use clap::ArgMatches;

#[derive(rust_embed::RustEmbed)]
#[folder = "examples/"]
struct ExampleEnv;

/// Creates a subcommand for creating an example configuration file for Glimbot.
pub fn subcommand() -> clap::App<'static, 'static> {
    clap::SubCommand::with_name("make-config")
        .arg(clap::Arg::with_name("output-file")
            .value_name("FILE")
            .default_value("./default.env")
            .help("The destination to write the dotenv file to.")
            .takes_value(true)
            .index(1))
        .about("Create a config file with placeholders to fill for a working dotenv file.")
}

/// Handles the case where someone invoked the output from the subcommand function.
pub async fn handle_matches(args: &ArgMatches<'_>) -> crate::error::Result<()> {
    let of = args.value_of("output-file").unwrap();
    let example_contents: Cow<[u8]> = ExampleEnv::get("example.env").unwrap();
    tokio::fs::write(of, &example_contents).await?;
    Ok(())
}
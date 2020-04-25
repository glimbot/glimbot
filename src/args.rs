use clap::{App, ArgMatches};

#[derive(thiserror::Error, Debug)]
pub enum ParseError {
    #[error("{0}")]
    Clap(#[from] clap::Error),
    #[error("An error occurred while parsing the arguments string: {0}")]
    Splitter(#[from] shell_words::ParseError),
}

pub type Result<T> = std::result::Result<T, ParseError>;

static DUMMY: [&'static str; 1] = ["dummy"];

pub fn parse_app_matches<'a, 'b>(s: impl AsRef<str>, a: &App<'a, 'b>) -> Result<ArgMatches<'a>> {
    let s = s.as_ref();
    let parts = shell_words::split(s)?;
    let app = a.clone();
    let matches = app.get_matches_from_safe(
        DUMMY.iter().cloned()
            .chain(parts.iter().map(|s| s.as_str())))?;
    Ok(matches)
}
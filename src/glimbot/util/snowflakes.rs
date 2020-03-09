use regex::Regex;
use once_cell::sync::Lazy;

static SNOWFLAKE_RE: Lazy<Regex> = Lazy::new(
    || Regex::new(r#"<(\D{1,2})(\d+)>"#).unwrap()
);
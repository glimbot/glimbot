use once_cell::sync::Lazy;
use regex::Regex;

static SNOWFLAKE_RE: Lazy<Regex> = Lazy::new(
    || Regex::new(r#"<(\D{1,2})(\d+)>"#).unwrap()
);
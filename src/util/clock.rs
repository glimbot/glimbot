use std::time::{Instant, Duration};
use once_cell::sync::Lazy;
use std::fmt;
use std::fmt::Formatter;

static BASE: Lazy<Instant> = Lazy::new(Instant::now);

#[derive(Debug, Ord, PartialOrd, Eq, PartialEq, Copy, Clone, Serialize, Deserialize, Hash, shrinkwraprs::Shrinkwrap)]
pub struct CacheInstant(Duration);

impl CacheInstant {
    pub fn now() -> Self {
        Self(BASE.elapsed())
    }

    pub fn elapsed(&self) -> Duration {
        Instant::now().saturating_duration_since(*BASE)
            .checked_sub(self.0)
            .unwrap_or_default()
    }
}
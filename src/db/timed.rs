use chrono::Utc;
use chrono::Duration;
use once_cell::sync::Lazy;
use serenity::model::id::{GuildId, UserId};

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ActionKind {
    Ban,
    Mute,
    Debug
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Action {
    expiry: chrono::DateTime<Utc>,
    target_user: UserId,
    guild: GuildId,
    kind: ActionKind
}

pub static ONE_MINUTE: Lazy<Duration> = Lazy::new(|| Duration::minutes(1));
pub static ONE_HUNDREDISH_YEARS: Lazy<Duration> = Lazy::new(|| Duration::days(365 * 100));

impl Action {
    pub fn new(user: UserId, guild: GuildId, action: ActionKind, expiry: impl Into<chrono::DateTime<Utc>>) -> Self {
        Self {
            expiry: expiry.into(),
            target_user: user,
            guild,
            kind: action
        }
    }

    pub fn with_duration(user: UserId, guild: GuildId, action: ActionKind, duration: impl Into<chrono::Duration>) -> Self {
        let expiry = chrono::DateTime::<Utc>::from(chrono::Local::now()).checked_add_signed(duration.into().clamp(*ONE_MINUTE, *ONE_HUNDREDISH_YEARS)).unwrap();
        Self::new(
            user,
            guild,
            action,
            expiry
        )
    }

    pub fn unban(user: UserId, guild: GuildId, duration: impl Into<chrono::Duration>) -> Self {
        Self::with_duration(user, guild, ActionKind::Ban, duration)
    }

    pub fn unmute(user: UserId, guild: GuildId, duration: impl Into<chrono::Duration>) -> Self {
        Self::with_duration(user, guild, ActionKind::Mute, duration)
    }

    pub fn debug(duration: impl Into<chrono::Duration>) -> Self {
        Self::with_duration(Default::default(), Default::default(), ActionKind::Debug, duration)
    }
}
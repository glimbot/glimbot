use chrono::Utc;
use chrono::Duration;
use once_cell::sync::Lazy;
use serenity::model::id::{GuildId, UserId};
use crate::db::DbContext;
use crate::dispatch::Dispatch;

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ActionKind {
    Ban,
    Mute,
    Debug
}

impl ActionKind {
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).expect("Failed to serialize ActionKind")
    }
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

#[derive(Clone)]
pub struct TimedEvents<'pool> {
    context: DbContext<'pool>
}

impl<'pool> TimedEvents<'pool> {
    pub fn new(context: DbContext<'pool>) -> Self {
        TimedEvents { context }
    }

    pub async fn store_action(&self, action: &Action) -> crate::error::Result<()> {
        sqlx::query!(
            r#"
            INSERT INTO timed_events (target_user, guild, action, expiry) VALUES ($1, $2, $3, $4);
            "#,
            action.target_user.0 as i64,
            self.context.guild_as_i64(),
            action.kind.to_json(),
            action.expiry.clone()
        ).execute(self.context.conn())
            .await?;
        Ok(())
    }
}


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

    pub async fn store_action(&self, dis: &Dispatch) -> crate::error::Result<()> {
        let db = dis.db(self.guild);
        let t = TimedEvents::new(db);
        t.store_action(self).await?;
        Ok(())
    }
}
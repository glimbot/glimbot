use serenity::model::id::{UserId, GuildId};
use chrono::{Utc, DateTime, FixedOffset};
use chrono::Duration;
use once_cell::sync::Lazy;
use std::cmp::Ordering;
use rand::{thread_rng, RngCore, Rng};
use byteorder::{BigEndian, LittleEndian, NativeEndian};
use zerocopy::{Unaligned, AsBytes, FromBytes, I64, U64, U128};
use crate::db::{DbKey, NamespacedDbContext, DbContext};
use std::borrow::Cow;
use serenity::client::Context;
use std::fmt;
use std::fmt::Formatter;

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
    user: UserId,
    guild: GuildId,
    action: ActionKind
}

pub const ONE_MINUTE: Lazy<Duration> = Lazy::new(|| Duration::minutes(1));
pub const ONE_HUNDREDISH_YEARS: Lazy<Duration> = Lazy::new(|| Duration::days(365 * 100));
pub const TIMED_EVENT_NS: &str = "timed_events";

impl Action {
    pub fn new(user: UserId, guild: GuildId, action: ActionKind, expiry: impl Into<chrono::DateTime<Utc>>) -> Self {
        Self {
            expiry: expiry.into(),
            user,
            guild,
            action
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

    /// Persist the action to DB.
    pub async fn store(&self) -> crate::error::Result<()> {
        let id = EventId::from(self.expiry.clone());
        let db = NamespacedDbContext::with_global_namespace(TIMED_EVENT_NS)
            .await?;
        db.insert(id, self).await
    }

}

/// It's a crappier UUID; mostly here because I want to be able to keep the time stamp in a predictable
/// format for byte serialization/deserialization
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, AsBytes, FromBytes, Unaligned)]
#[repr(C)]
pub struct EventId {
    event_time: U64<BigEndian>,
    pub id: U128<NativeEndian>,
}

impl EventId {
    pub fn to_datetime(&self) -> chrono::DateTime<Utc> {
        chrono::DateTime::from_utc(
            chrono::NaiveDateTime::from_timestamp(self.event_time.get() as i64, 0),
            Utc
        )
    }
}

impl From<chrono::DateTime<Utc>> for EventId {
    fn from(e: DateTime<Utc>) -> Self {
        let id = thread_rng().gen::<u128>();
        let time_stamp = e.timestamp() as u64;
        // If this is losing info, that's worrying; we shouldn't be looking at the past
        debug_assert!(time_stamp > 0);
        Self {
            event_time: U64::new(time_stamp),
            id: U128::new(id)
        }
    }
}

impl Ord for EventId {
    fn cmp(&self, other: &Self) -> Ordering {
        if self.event_time != other.event_time {
            self.to_datetime().cmp(&other.to_datetime())
        } else {
            self.id.get().cmp(&other.id.get())
        }
    }
}

impl PartialOrd for EventId {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl DbKey for EventId {
    fn to_key(&self) -> Cow<[u8]> {
        Cow::Borrowed(self.as_bytes())
    }
}

impl fmt::Display for EventId {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{:08x}-{:016x}", self.event_time.get(), self.id.get())
    }
}

#[derive(Shrinkwrap, Clone, Debug, Hash, PartialEq, PartialOrd)]
pub struct Cutoff(chrono::DateTime<Utc>);

impl DbKey for Cutoff {
    fn to_key(&self) -> Cow<[u8]> {
        (self.0.timestamp() as u64).to_be_bytes().to_vec().into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use more_asserts::*;
    use chrono::{NaiveDateTime, Local};

    #[test]
    pub fn test_ordering() {
        let first = EventId::from(DateTime::from_utc(
            NaiveDateTime::from_timestamp(1000, 0),
            Utc
        ));
        let now = EventId::from(DateTime::from(Local::now()));
        let later = EventId::from(DateTime::from(Local::now()) + chrono::Duration::days(10));

        assert_le!(first, now);
        assert_le!(now, later);
        assert_le!(first, later);

        let debug_event = Action::debug(chrono::Duration::minutes(10));
    }
}
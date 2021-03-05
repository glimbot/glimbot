use serenity::model::id::{UserId, GuildId};
use chrono::{Utc, DateTime, FixedOffset};
use chrono::Duration;
use once_cell::sync::Lazy;
use std::cmp::Ordering;
use rand::{thread_rng, RngCore, Rng};
use byteorder::{BigEndian, LittleEndian, NativeEndian};
use zerocopy::{Unaligned, AsBytes, FromBytes, I64, U64, U128};
use crate::db::{DbContext};
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
}
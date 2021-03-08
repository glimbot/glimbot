//! Contains types related to processing timed events.

use std::borrow::Cow;
use std::fmt::{Display, Formatter};
use std::fmt;

use chrono::Duration;
use chrono::Utc;
use futures::{Stream, TryStreamExt};
use once_cell::sync::Lazy;
use serenity::model::id::{GuildId, UserId};
use serenity::prelude::{Context, Mentionable};
use serenity::utils::content_safe;
use sqlx::PgPool;
use sqlx::query::QueryAs;

use crate::db::DbContext;
use crate::dispatch::config::VerifiedRole;
use crate::dispatch::Dispatch;
use crate::module::moderation::NoMuteRoleSet;

/// The kind of action to be taken once a timed event is processed.
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone, Copy)]
#[non_exhaustive]
pub enum ActionKind {
    /// A ban needs to be reversed.
    Ban,
    /// A user needs to be unmuted.
    Mute,
    /// Prints a debug message to the logger.
    Debug,
}

impl ActionKind {
    /// Converts this kind into its JSON representation.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).expect("Failed to serialize ActionKind")
    }
}

/// An action to be taken when expiry is reached.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Action {
    /// When the action should be taken
    expiry: chrono::DateTime<Utc>,
    /// The user affected by the action.
    target_user: UserId,
    /// The guild in which this action takes place.
    guild: GuildId,
    /// The kind of action to take.
    kind: ActionKind,
}

/// The kind of failure that occurred while processing the action.
#[derive(Clone, Debug)]
pub enum FailureKind {
    /// The target user was not in the guild.
    UserNotInGuild,
    /// No mute role has been set.
    NoMuteRole,
    /// Some other unspecified error.
    SysError(String),
}

/// The error type specifically for timed actions
#[derive(Clone, Debug)]
pub struct ActionFailure {
    /// The action that was being performed when the error occurred.
    action: Action,
    /// The kind of failure that occurred.
    kind: FailureKind,
}

impl ActionFailure {
    /// Creates an `ActionFailure` from an action and a kind of failure.
    pub fn new(action: Action, kind: FailureKind) -> Self {
        ActionFailure { action, kind }
    }

    /// Creates an `ActionFailure` from an action and any bot-compatible error.
    pub fn from_err(action: Action, e: impl Into<crate::error::Error>) -> Self {
        let e = e.into();
        let m = if e.is_user_error() {
            e.to_string()
        } else {
            "backend failure".into()
        };
        ActionFailure { action, kind: FailureKind::SysError(m) }
    }
}

impl ActionFailure {
    /// Returns a constant string describing what type of action failed.
    pub const fn failure_message(&self) -> &str {
        match self.action.kind {
            ActionKind::Ban => { "could not unban" }
            ActionKind::Mute => { "could not unmute" }
            ActionKind::Debug => { "could not print debug statement" }
        }
    }

    /// Returns a string describing the failure that occurred.
    pub fn failure_info(&self) -> Cow<str> {
        match &self.kind {
            FailureKind::UserNotInGuild => {
                format!("user {} is not a member of this guild", self.action.target_user).into()
            }
            FailureKind::NoMuteRole => {
                "guild doesn't have a mute role set".into()
            }
            FailureKind::SysError(s) => {
                Cow::Borrowed(s)
            }
        }
    }
}

impl fmt::Display for ActionFailure {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.failure_message(), self.failure_info())
    }
}

impl std::error::Error for ActionFailure {}

impl Action {
    /// Performs the action.
    // TODO: report when this fails into guild log channel
    #[instrument(level = "debug", skip(dis, ctx))]
    pub async fn act(&self, dis: &Dispatch, ctx: &Context) -> crate::error::Result<()> {
        let db = dis.db(self.guild);
        let res: Result<(), ActionFailure> = match self.kind {
            ActionKind::Ban => {
                self.do_unban(ctx).await
            }
            ActionKind::Mute => {
                self.do_unmute(dis, db.clone(), ctx).await
            }
            ActionKind::Debug => {
                debug!("Got debug action: {:?}", self);
                Ok(())
            }
        };

        if let Err(e) = res {
            warn!("{}", e);
        }


        let t = TimedEvents::new(db);
        t.drop_action(self).await?;
        Ok(())
    }

    /// Unmutes a user in a guild.
    #[instrument(level = "debug", skip(self, dis, db, ctx))]
    async fn do_unmute<'me, 'dis, 'a>(&'me self, dis: &'dis Dispatch, db: DbContext<'dis>, ctx: &'a Context) -> Result<(), ActionFailure> {
        let mute_role = dis.config_value_t::<VerifiedRole>(crate::module::moderation::MUTE_ROLE)
            .unwrap()
            .get(&db)
            .await.map_err(|e| {
            ActionFailure::from_err(*self, e)
        })?
            .ok_or_else(|| ActionFailure::from_err(*self, NoMuteRoleSet))?;

        let mut mem = self.guild.member(ctx, self.target_user).await
            .map_err(|_| ActionFailure::new(*self, FailureKind::UserNotInGuild))?;

        if mem.roles.contains(&mute_role.into_inner()) {
            debug!("unmuting user");
            mem.remove_role(ctx, mute_role.into_inner())
                .await
                .map_err(|e| ActionFailure::from_err(*self, e))?;
        } else {
            debug!("user wasn't muted");
        }

        Ok(())
    }

    /// Unbans a user in a guild.
    #[instrument(level = "debug", skip(self, ctx))]
    async fn do_unban(&self, ctx: &Context) -> Result<(), ActionFailure> {
        self.guild.unban(ctx, self.target_user)
            .await
            .map_err(|e| ActionFailure::from_err(*self, e))?;
        Ok(())
    }
}

impl Action {
    /// Accessor for the guild id.
    pub fn guild(&self) -> GuildId {
        self.guild
    }
}

/// A duration representing one minute.
pub static ONE_MINUTE: Lazy<Duration> = Lazy::new(|| Duration::minutes(1));
/// A duration representing about one hundred years.
pub static ONE_HUNDREDISH_YEARS: Lazy<Duration> = Lazy::new(|| Duration::days(365 * 100));

#[doc(hidden)]
struct Row {
    target_user: i64,
    guild: i64,
    expiry: chrono::DateTime<Utc>,
    action: serde_json::Value,
}

/// A wrapper for a database context for performing actions with timed actions.
#[derive(Clone)]
pub struct TimedEvents<'pool> {
    /// The wrapped database context.
    context: DbContext<'pool>
}

impl<'pool> TimedEvents<'pool> {
    /// The maximum number of items which will be pulled per timed event tick.
    pub const BATCH_LIMIT: usize = 1024;

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

    pub async fn drop_action(&self, action: &Action) -> crate::error::Result<()> {
        sqlx::query!(
            r#"
            DELETE FROM timed_events WHERE target_user = $1
                                       AND guild = $2
                                       AND action = $3
                                       AND expiry = $4;
            "#,
            action.target_user.0 as i64,
            self.context.guild_as_i64(),
            action.kind.to_json(),
            action.expiry.clone()
        ).execute(self.context.conn())
            .await?;
        Ok(())
    }

    pub async fn get_actions_before(pool: &PgPool, epoch: chrono::DateTime<Utc>) -> crate::error::Result<Vec<Action>> {

        let q: sqlx::query::Map<_, _, _> = sqlx::query_as!(
            Row,
            r#"
            SELECT * FROM timed_events WHERE expiry <= $1 LIMIT $2;
            "#,
            epoch,
            Self::BATCH_LIMIT as i64
        );

        q.try_map(|r: Row| {
            Ok(Action::new((r.target_user as u64).into(),
                           (r.guild as u64).into(),
                           serde_json::from_value(r.action)
                               .map_err(|e| sqlx::Error::Decode(e.into()))?,
                           r.expiry))
        }).fetch_all(pool)
            .await
            .map_err(crate::error::Error::from)
    }
}


impl Action {
    pub fn new(user: UserId, guild: GuildId, action: ActionKind, expiry: impl Into<chrono::DateTime<Utc>>) -> Self {
        Self {
            expiry: expiry.into(),
            target_user: user,
            guild,
            kind: action,
        }
    }

    pub fn with_duration(user: UserId, guild: GuildId, action: ActionKind, duration: impl Into<chrono::Duration>) -> Self {
        let expiry = chrono::DateTime::<Utc>::from(chrono::Local::now()).checked_add_signed(duration.into().clamp(*ONE_MINUTE, *ONE_HUNDREDISH_YEARS)).unwrap();
        Self::new(
            user,
            guild,
            action,
            expiry,
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
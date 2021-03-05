use std::borrow::{Borrow, Cow};
use once_cell::sync::Lazy;
use serenity::builder::CreateEmbed;
use serenity::client::Context;
use serenity::model::channel::Message;
use serenity::model::guild::Member;
use serenity::model::id::{ChannelId, GuildId, MessageId, UserId};
use serenity::model::misc::Mentionable;
use serenity::utils::Color;
use structopt::StructOpt;

use crate::db::DbContext;
use crate::dispatch::config::{Value, VerifiedChannel, VerifiedRole, VerifiedUser, FromStrWithCtx};
use crate::dispatch::Dispatch;
use crate::module::{ModInfo, Module, Sensitivity};
use crate::util::ClapExt;
use crate::util::constraints::AtMostU64;
use humantime::Duration;
use crate::db::timed::{Action, ONE_HUNDREDISH_YEARS};

pub struct ModerationModule;

pub const TIMED_ACTION_KEY: &str = "timed";


#[derive(Debug, StructOpt)]
pub struct CommonOpts {
    /// Which user the action should apply to.
    user: String,
    /// Why the action is being taken.
    reason: Option<String>,
}

#[derive(Debug, StructOpt)]
/// Command for moderating users.
pub enum ModOpt {
    /// Warn a user and make a note in the mod log about it.
    Warn(CommonOpts),
    /// Kick a user from the server.
    Kick(CommonOpts),
    /// Ban a user from the server.
    Ban {
        #[structopt(flatten)]
        common: CommonOpts,
        /// How long the user should be banned for. Specified in human format, i.e. "5d 2h 5m"
        /// Max 100 years, min 1 minute. Very large values may be interpreted as indefinite in duration.
        #[structopt(short = "d")]
        duration: Option<humantime::Duration>,
        #[structopt(short = "m")]
        /// How many days of messages from the user should be deleted.
        delete_messages: Option<AtMostU64<7>>,
    },
    /// Bans a user with max number of days for message deletion, then unbans them.
    /// Useful for deleting spam.
    SoftBan(CommonOpts),
    /// Adds the muted user role to a user.
    Mute {
        #[structopt(flatten)]
        common: CommonOpts,
        #[structopt(short = "d")]
        /// How long the user should be muted for. Specified in human format, i.e. "5d 2h 5m"
        /// Max 100 years, min 1 minute. Very large values may be interpreted as indefinite in duration.
        duration: Option<humantime::Duration>,
    },
}

impl ModOpt {
    pub fn common_args(&self) -> &CommonOpts {
        match self {
            ModOpt::Warn(c) => { c }
            ModOpt::Kick(c) => { c }
            ModOpt::Ban { common, .. } => { common }
            ModOpt::SoftBan(c) => { c }
            ModOpt::Mute { common, .. } => { common }
        }
    }

    pub fn kind(&self) -> ActionKind {
        use ActionKind::*;
        match self {
            ModOpt::Warn(_) => { Warn }
            ModOpt::Kick(_) => { Kick }
            ModOpt::Ban { .. } => { Ban }
            ModOpt::SoftBan(_) => { SoftBan }
            ModOpt::Mute { .. } => { Mute }
        }
    }

    pub fn duration(&self) -> Option<Duration> {
        let d = match self {
            ModOpt::Ban { duration, .. } => { duration.clone() }
            ModOpt::Mute { duration, .. } => { duration.clone() }
            _ => None
        };
        d
    }

    pub fn deletion_time(&self) -> Option<AtMostU64<7>> {
        match self {
            ModOpt::Ban { delete_messages, .. } => {delete_messages.clone()}
            _ => None
        }
    }
}

const MOD_CHANNEL: &str = "mod_log_channel";

const MUTE_ROLE: &str = "mute_role";

#[async_trait::async_trait]
impl Module for ModerationModule {
    fn info(&self) -> &ModInfo {
        static INFO: Lazy<ModInfo> = Lazy::new(|| ModInfo::with_name("mod")
            .with_sensitivity(Sensitivity::High)
            .with_command(true)
            .with_config_value(Value::<VerifiedChannel>::new(MOD_CHANNEL, "Channel for logging moderation actions."))
            .with_config_value(Value::<VerifiedRole>::new(MUTE_ROLE, "Role to assign to muted users."))
        );

        &INFO
    }

    async fn process(&self, dis: &Dispatch, ctx: &Context, orig: &Message, command: Vec<String>) -> crate::error::Result<()> {
        let gid = orig.guild_id.unwrap();
        let opts = ModOpt::from_iter_with_help(command)?;
        let common = opts.common_args();
        let kind = opts.kind();
        let orig_mess = orig.message_reference.as_ref().map(|m| m.message_id).flatten();
        let duration = opts.duration();
        let channel = orig.channel_id;

        let user = VerifiedUser::from_str_with_ctx(&common.user, ctx, gid).await?;
        let member = gid.member(ctx, user.into_inner()).await?;

        let mut action = ModAction::new(&member,
                                        channel,
                                        orig.author.id,
                                        kind)
            .with_duration(duration);

        if let Some(m) = orig_mess {
            action = action.with_original_message(m);
        }

        if let Some(r) = common.reason.clone() {
            action = action.with_reason(r);
        }

        action.act(dis, ctx).await?;
        action.report_action(dis, ctx).await?;
        orig.react(ctx, '✅').await?;

        Ok(())
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ActionKind {
    Warn,
    Kick,
    SoftBan,
    Ban,
    Mute,
}

impl ActionKind {
    pub const SAFETY_YELLOW: Color = Color::new(0xEED202);
    pub const SAFETY_ORANGE: Color = Color::new(0xFF6700);
    pub const TRAFFIC_RED: Color = Color::new(0xB8D113);

    pub const fn color(&self) -> Color {
        match self {
            ActionKind::Warn => Self::SAFETY_YELLOW,
            ActionKind::Kick => Self::SAFETY_ORANGE,
            ActionKind::SoftBan => Color::FABLED_PINK,
            ActionKind::Ban => Self::TRAFFIC_RED,
            ActionKind::Mute => Color::DARK_BLUE,
        }
    }

    pub const fn name(&self) -> &'static str {
        match self {
            ActionKind::Warn => { "warning" }
            ActionKind::Kick => { "kick" }
            ActionKind::SoftBan => { "soft ban" }
            ActionKind::Ban => { "ban" }
            ActionKind::Mute => { "mute" }
        }
    }

    pub const fn title_name(&self) -> &'static str {
        match self {
            ActionKind::Warn => { "Warning" }
            ActionKind::Kick => { "Kick" }
            ActionKind::SoftBan => { "Soft ban" }
            ActionKind::Ban => { "Ban" }
            ActionKind::Mute => { "Mute" }
        }
    }

    pub const fn has_duration(&self) -> bool {
        match self {
            ActionKind::Ban |
            ActionKind::Mute => { true }
            _ => false
        }
    }
}

#[derive(Debug, Clone)]
pub struct ModAction {
    user: Member,
    action: ActionKind,
    moderator: UserId,
    channel: ChannelId,
    reason: Option<Cow<'static, str>>,
    original_message: Option<MessageId>,
    duration: Option<Duration>,
    deletion_days: Option<AtMostU64<7>>
}

impl ModAction {
    pub fn deletion_days(&self) -> Option<AtMostU64<7>> {
        self.deletion_days
    }
}

impl ModAction {
    pub fn user(&self) -> &Member {
        &self.user
    }
    pub fn action(&self) -> ActionKind {
        self.action
    }
    pub fn moderator(&self) -> UserId {
        self.moderator
    }
    pub fn reason(&self) -> &str {
        self.reason.as_ref().map(|r| r.as_ref()).unwrap_or("No reason specified.")
    }
    pub fn original_message(&self) -> Option<MessageId> {
        self.original_message
    }
    pub fn duration(&self) -> Option<Duration> {
        self.duration
    }
    pub fn guild(&self) -> GuildId { self.user().guild_id }
}

impl ModAction {
    pub fn new(mem: impl Borrow<Member>, channel_id: ChannelId, moderator: UserId, action: ActionKind) -> Self {
        ModAction {
            user: mem.borrow().clone(),
            action,
            moderator,
            channel: channel_id,
            reason: None,
            original_message: None,
            duration: None,
            deletion_days: None,
        }
    }

    pub async fn act(&self, dis: &Dispatch, ctx: &Context) -> crate::error::Result<()> {
        match self.action {
            ActionKind::Warn => {}
            ActionKind::Kick => {
                self.user().kick_with_reason(ctx, self.reason()).await?;
            }
            ActionKind::SoftBan => {
                self.user().ban_with_reason(ctx, 7, self.reason()).await?;
                self.user().unban(ctx).await?;
            }
            ActionKind::Ban => {
                self.user().ban_with_reason(ctx,
                                            self.deletion_days.map(Into::into).unwrap_or(0u64) as u8,
                                            self.reason()).await?;
            }
            ActionKind::Mute => {self.mute_user(dis, ctx).await?;}
        }

        if let Some(d) = self.duration() {
            let chrono_dur = chrono::Duration::from_std(*d).unwrap_or_else(|_| (*ONE_HUNDREDISH_YEARS).clone());
            let a = match self.action {
                ActionKind::Ban => {
                    Action::unban(self.user().user.id, self.guild(), chrono_dur)
                }
                ActionKind::Mute => {
                    Action::unmute(self.user().user.id, self.guild(), chrono_dur)
                }
                _ => {warn!("Got a duration with a nonsensical attribute."); return Ok(())}
            };
            a.store_action(dis).await?;
        }
        Ok(())
    }

    pub fn with_duration(mut self, duration: Option<Duration>) -> Self {
        self.duration = duration;
        self
    }

    pub fn with_reason(mut self, reason: impl Into<Cow<'static, str>>) -> Self {
        self.reason = reason.into().into();
        self
    }

    pub fn with_original_message(mut self, message: MessageId) -> Self {
        self.original_message = Some(message);
        self
    }

    pub fn create_embed(&self, embed: &mut CreateEmbed) {
        let user = format!("{} ({})", self.user.display_name(), self.user.user.id);
        let moderator = self.moderator.mention();
        let reason = self.reason.clone().unwrap_or_else(|| "No reason specified.".into());

        embed.color(self.action.color())
            .title(self.action.title_name())
            .field("User", user, false)
            .field("Reason", reason, false)
            .field("Moderator", moderator, false)
            .field("Channel", self.channel.mention(), false);

        if self.action.has_duration() {
            let dur = self.duration.as_ref()
                .map(|d| d
                    .to_string()
                    .into())
                .unwrap_or_else(|| Cow::from("Indefinite"));

            embed.field("Duration", dur, false);
        }

        if let Some(m) = self.original_message {
            let url = format!("https://discord.com/channels/{gid}/{chan}/{mess}",
                              gid = self.user.guild_id,
                              chan = self.channel,
                              mess = m
            );
            embed.field("In response to", url, false);
        }
    }

    pub async fn mute_user(&self, dis: &Dispatch, ctx: &Context) -> crate::error::Result<()> {
        let action = self;
        let cfg_db = DbContext::new(dis.pool(), action.guild());
        let mute_role = dis.config_value_t::<VerifiedRole>(MUTE_ROLE)?
            .get(&cfg_db)
            .await?
            .ok_or(NoMuteRoleSet)?;
        let mut mem = action.user().clone();
        mem.add_role(ctx, mute_role.into_inner()).await?;
        Ok(())
    }

    pub async fn report_action(&self, dis: &Dispatch, ctx: &Context) -> crate::error::Result<()> {
        let action = self;
        let mod_channel_v = dis.config_value_t::<VerifiedChannel>(MOD_CHANNEL)?;
        let cfg_db = DbContext::new(dis.pool(), action.guild());
        let mod_channel = mod_channel_v.get(&cfg_db)
            .await?
            .ok_or(NoModChannelSet)?;
        mod_channel.into_inner().send_message(ctx, |e| {
            e.embed(|emb| {
                action.create_embed(emb);
                emb
            })
        }).await?;
        Ok(())
    }
}

impl_err!(NoModChannelSet, "No mod channel has been set for this guild (mod_log_channel).", true);
impl_err!(NoMuteRoleSet, "No mute role has been set for this guild (mute_role).", true);




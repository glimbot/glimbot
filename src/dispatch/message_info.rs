use chrono::Utc;
use serenity::model::id::{ChannelId, MessageId, UserId};
use serenity::model::prelude::Message;
use std::borrow::Borrow;

#[derive(Copy, Clone, Debug, Ord, PartialOrd, Eq, PartialEq)]
pub struct MsgInfo {
    pub timestamp: chrono::DateTime<Utc>,
    pub user: UserId,
    pub channel: ChannelId,
    pub msg: MessageId,
}

impl<BM: Borrow<Message>> From<BM> for MsgInfo {
    fn from(m: BM) -> Self {
        let m = m.borrow();
        MsgInfo {
            timestamp: m.timestamp,
            user: m.author.id,
            channel: m.channel_id,
            msg: m.id,
        }
    }
}

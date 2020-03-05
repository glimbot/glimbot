use serenity::model::prelude::*;
use std::collections::HashMap;
use crate::glimbot::BotPermissions;

pub struct GuildContext {
    guild: GuildId,
    name: String,
    owner: UserId,
    command_prefix: String,
    roles: HashMap<RoleId, BotPermissions>,
    channel_permissions: HashMap<ChannelId, BotPermissions>,
    debug_channel: Option<ChannelId>,
    log_channel: Option<ChannelId>,
    command_aliases: HashMap<String, String>,
    default_role: RoleId
}

// impl From<&Guild> for GuildContext {
//     fn from(guild: &Guild) -> Self {
//         GuildContext {
//             guild: guild.id,
//             name: guild.name.clone(),
//             owner: guild.owner_id,
//             command_prefix: "!".to_owned(),
//             roles: HashMap::new(),
//             channel_permissions: HashMap::new()
//         }
//     }
// }

use serenity::model::prelude::*;
use std::collections::HashMap;
use crate::glimbot::BotPermissions;
use parking_lot::RwLock;

pub struct GuildContext {
    guild: GuildId,
    command_info: RwLock<CommandInfo>
}

pub struct CommandInfo {
    last_executed: HashMap<String, chrono::NaiveDateTime>,
    
}


impl GuildContext {

}
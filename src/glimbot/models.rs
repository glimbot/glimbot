use diesel::*;
use diesel::sql_types::*;
use crate::glimbot::schema::*;

#[derive(PartialEq, Eq, Debug, Clone, Queryable, Insertable, Identifiable, QueryableByName)]
#[table_name = "guilds"]
pub struct GuildContext {
    pub id: i64,
    pub name: String,
}

#[derive(PartialEq, Eq, Debug, Clone, Queryable, Insertable, Identifiable, Associations)]
#[belongs_to(GuildContext)]
pub struct GuildOwner {
    pub id: i64,
    pub guild_context_id: i64,
    pub added_unix: i64,
}

#[derive(PartialEq, Eq, Debug, Clone, Queryable, Insertable, Identifiable, Associations)]
#[belongs_to(GuildContext)]
#[primary_key(guild_context_id)]
pub struct BotConfig {
    pub guild_context_id: i64,
    pub setup_done: bool,
    pub mod_role: i64,
    pub mod_channel: i64,
    pub bot_channel: i64,
    pub listen_to_bots: bool,
    pub command_prefix: String,
    pub silence_role: i64,
    pub member_role: i64
}

#[derive(PartialEq, Eq, Debug, Clone, Queryable, Insertable, Identifiable, Associations)]
#[belongs_to(BotConfig)]
#[primary_key(bot_config_id, channel)]
pub struct FreeChannel {
    bot_config_id: i64,
    channel: i32
}

#[derive(PartialEq, Eq, Debug, Clone, Queryable, Insertable, Identifiable, Associations)]
#[belongs_to(BotConfig)]
#[primary_key(bot_config_id, from)]
#[table_name = "command_aliases"]
pub struct CommandAlias {
    bot_config_id: i64,
    from: String,
    to: String
}

#[derive(PartialEq, Eq, Debug, Clone, Queryable, Insertable, Identifiable, Associations)]
#[belongs_to(BotConfig)]
#[primary_key(bot_config_id)]
pub struct ModuleConfig {
    bot_config_id: i64,

}


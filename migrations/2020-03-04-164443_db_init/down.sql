-- This file should undo anything in `up.sql`
PRAGMA foreign_keys = OFF;

DROP TABLE guilds;
DROP TABLE bot_configs;
DROP TABLE command_aliases;
DROP TABLE free_channels;
DROP TABLE guild_owners;

PRAGMA foreign_keys = ON;
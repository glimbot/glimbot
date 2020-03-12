-- This file should undo anything in `up.sql`
PRAGMA foreign_keys = OFF;

DROP TABLE guilds;
DROP TABLE incrementers;
DROP TABLE bag_configs;
DROP TABLE bag_items;

PRAGMA foreign_keys = ON;
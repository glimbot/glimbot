ALTER TABLE guild_config RENAME TO old_guild_config;

CREATE TABLE guild_config
(
    name           text not null default '',
    command_prefix text not null default '!'
);

INSERT INTO guild_config VALUES ((SELECT value FROM old_guild_config WHERE key = 'name'),
                                 (SELECT value FROM old_guild_config WHERE key = 'command_prefix'));

DROP TABLE old_guild_config;

CREATE TRIGGER IF NOT EXISTS ensure_single_row_guild_config
    BEFORE INSERT
    ON guild_config
BEGIN
    SELECT CASE
               WHEN (SELECT COUNT(*) FROM guild_config) >= 1
                   THEN RAISE(IGNORE)
               END;
END;

CREATE TRIGGER IF NOT EXISTS ensure_cmd_prefix_single_char
    BEFORE UPDATE
    ON guild_config
    WHEN LENGTH(NEW.command_prefix) <> 1
BEGIN
    SELECT RAISE (ABORT, 'New command prefix must have length 1.');
END;

CREATE TRIGGER IF NOT EXISTS ensure_cmd_prefix_single_char_ins
    BEFORE INSERT
    ON guild_config
    WHEN LENGTH(NEW.command_prefix) <> 1
BEGIN
    SELECT RAISE (ABORT, 'New command prefix must have length 1.');
END;
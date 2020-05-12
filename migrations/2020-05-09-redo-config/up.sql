CREATE TABLE new_guild_config
(
    key   text not null primary key,
    value text not null
);

INSERT OR IGNORE INTO guild_config DEFAULT
VALUES;

INSERT INTO new_guild_config
VALUES ('name', (SELECT name FROM guild_config LIMIT 1));

INSERT INTO new_guild_config
VALUES ('command_prefix', (SELECT command_prefix FROM guild_config LIMIT 1));

DROP TABLE guild_config;

ALTER TABLE new_guild_config
    RENAME TO guild_config;

CREATE TRIGGER ensure_command_prefix_one_char
    BEFORE INSERT
    ON guild_config
    WHEN NEW.key = 'command_prefix' AND length(NEW.value) <> 1
BEGIN
    SELECT RAISE(ABORT, 'New command prefix must be exactly 1 in length.');
END;

CREATE TRIGGER ensure_command_prefix_one_char_upd
    BEFORE UPDATE
    ON guild_config
    WHEN NEW.key = 'command_prefix' AND length(NEW.value) <> 1
BEGIN
    SELECT RAISE(ABORT, 'New command prefix must be exactly 1 in length.');
END;
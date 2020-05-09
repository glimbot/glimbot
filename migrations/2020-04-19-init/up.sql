CREATE TABLE IF NOT EXISTS guild_config
(
    name           text not null default '',
    command_prefix text not null default '!'
);

CREATE TABLE IF NOT EXISTS users
(
    user bigint not null primary key
);

CREATE TABLE IF NOT EXISTS messages
(
    user      bigint not null,
    message   bigint not null,
    pressure  bigint not null default 0,
    unix_time bigint not null,
    primary key (user, message),
    foreign key (user)
        references users (user)
        on delete cascade
);

CREATE INDEX IF NOT EXISTS user_freq
    ON messages (user, unix_time);

CREATE TRIGGER IF NOT EXISTS ensure_message_user
    BEFORE INSERT
    ON messages
BEGIN
    INSERT OR IGNORE INTO users VALUES (NEW.user);
END;

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
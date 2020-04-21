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
ON messages(user, unix_time);

CREATE TRIGGER IF NOT EXISTS ensure_message_user
    BEFORE INSERT
    ON messages
BEGIN
    INSERT OR IGNORE INTO users VALUES (NEW.user);
END;
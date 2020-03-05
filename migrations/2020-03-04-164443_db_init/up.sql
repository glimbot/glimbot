CREATE TABLE guilds
(
    id   bigint not null primary key,
    name text   not null
);

CREATE TABLE guild_owners
(
    id         bigint    not null primary key,
    guild_id   bigint    not null,
    when_added timestamp not null default current_timestamp,
    FOREIGN KEY (guild_id)
        REFERENCES guilds (id)
        ON DELETE CASCADE
        ON UPDATE NO ACTION
);

CREATE TABLE bot_configs
(
    guild_id       bigint  not null primary key,
    setup_done     boolean not null default false,
    mod_role       bigint,
    bot_channel    bigint,
    listen_to_bots boolean not null default false,
    command_prefix text    not null default '!',
    silence_role   bigint,
    member_role    bigint,
    FOREIGN KEY (guild_id)
        REFERENCES guilds (id)
        ON DELETE CASCADE
        ON UPDATE NO ACTION
);

CREATE TABLE free_channels
(
    bot_config_id bigint not null,
    channel_id    bigint not null,
    PRIMARY KEY (bot_config_id, channel_id),
    FOREIGN KEY (bot_config_id)
        REFERENCES bot_configs (guild_id)
        ON DELETE CASCADE
        ON UPDATE NO ACTION
);

CREATE TABLE command_aliases
(
    bot_config_id bigint not null,
    frm           text   not null,
    dest          text   not null,
    PRIMARY KEY (bot_config_id, frm),
    FOREIGN KEY (bot_config_id)
        REFERENCES bot_configs (guild_id)
        ON DELETE CASCADE
        ON UPDATE NO ACTION
);

CREATE TABLE modules
(
    id   bigint not null primary key autoincrement,
    name text    not null unique
);

CREATE TABLE disabled_modules
(
    module_id     bigint not null,
    bot_config_id bigint  not null,
    PRIMARY KEY (module_id, bot_config_id),
    FOREIGN KEY (bot_config_id)
        REFERENCES bot_configs (guild_id)
        ON DELETE CASCADE
        ON UPDATE NO ACTION,
    FOREIGN KEY (module_id)
        REFERENCES modules (id)
        ON DELETE CASCADE
        ON UPDATE NO ACTION
);

CREATE TABLE commands
(
    id        bigint not null primary key autoincrement,
    module_id bigint not null,
    name      text    not null unique,
    FOREIGN KEY (module_id)
        REFERENCES modules (id)
        ON DELETE CASCADE
        ON UPDATE NO ACTION
);

CREATE TABLE command_roles
(
    bot_config_id bigint not null,
    command_id bigint not null,
    role_id bigint not null,
    PRIMARY KEY (bot_config_id, command_id, role_id)
)





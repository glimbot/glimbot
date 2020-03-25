CREATE TABLE IF NOT EXISTS guilds
(
    id             bigint not null primary key,
    command_prefix text   not null default '!'
);

CREATE TABLE IF NOT EXISTS incrementers
(
    guild_id bigint not null,
    name     text   not null,
    count    bigint not null default 0,
    primary key (guild_id, name),
    foreign key (guild_id)
        references guilds (id)
        on delete cascade
);

CREATE TABLE IF NOT EXISTS bag_configs
(
    guild_id bigint not null primary key,
    capacity bigint not null default 10,
    foreign key (guild_id)
        references guilds (id)
        on delete cascade
);

CREATE TABLE IF NOT EXISTS bag_items
(
    id       integer not null primary key autoincrement,
    guild_id bigint  not null,
    name     text    not null,
    foreign key (guild_id)
        references guilds (id)
        on delete cascade
);

CREATE INDEX IF NOT EXISTS bag_item_guilds ON bag_items (guild_id);

CREATE TRIGGER IF NOT EXISTS bag_item_cap
    BEFORE INSERT
    ON bag_items
BEGIN
    SELECT CASE
               WHEN
                           (SELECT capacity FROM bag_configs B WHERE B.guild_id = NEW.guild_id) <
                           (SELECT COUNT(*) FROM bag_items B WHERE B.guild_id = NEW.guild_id) + 1 THEN
                   RAISE(ABORT, 'Bag is full')
               END;
END;




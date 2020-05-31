create table if not exists words
(
    id   integer primary key,
    word text not null unique
);

create table if not exists related_words
(
    word1_id integer not null,
    word2_id integer not null,
    primary key (word1_id, word2_id),
    foreign key (word1_id)
        references words (id)
        on delete cascade,
    foreign key (word2_id)
        references words (id)
        on delete cascade
);

create table if not exists synonymous_words
(
    word1_id integer not null,
    word2_id integer not null,
    primary key (word1_id, word2_id),
    foreign key (word1_id)
        references words (id)
        on delete cascade,
    foreign key (word2_id)
        references words (id)
        on delete cascade
);


create table if not exists definitions
(
    word_id    integer not null,
    pos        text    not null,
    definition text    not null,
    primary key (word_id, pos, definition),
    foreign key (word_id)
        references words (id)
        on delete cascade
);

create index if not exists def_in_order
on definitions (word_id, definition);

create index if not exists pos_in_order
on definitions (word_id, pos);

create trigger if not exists reflexive_related_words
    before insert
    on related_words
begin
    INSERT OR IGNORE INTO related_words VALUES (NEW.word2_id, NEW.word1_id);
end;

create trigger if not exists reflexive_synonymous_words
    before insert
    on synonymous_words
begin
    INSERT OR IGNORE INTO synonymous_words VALUES (NEW.word2_id, NEW.word1_id);
end;
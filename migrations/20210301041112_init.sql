CREATE TABLE known_guilds
(
    guild             BIGINT PRIMARY KEY,
    joinable_role_cnt INT NOT NULL DEFAULT 0
        CONSTRAINT reasonable_role_count CHECK (joinable_role_cnt >= 0 AND joinable_role_cnt <= 128)
);

CREATE TABLE config_values
(
    guild BIGINT,
    name  TEXT,
    value JSONB NOT NULL,
    PRIMARY KEY (guild, name),
    FOREIGN KEY (guild)
        REFERENCES known_guilds (guild)
        ON DELETE CASCADE
);

CREATE TABLE joinable_roles
(
    guild BIGINT,
    role  BIGINT,
    PRIMARY KEY (guild, role),
    FOREIGN KEY (guild)
        REFERENCES known_guilds (guild)
        ON DELETE CASCADE
);

CREATE OR REPLACE FUNCTION ensure_guild()
    RETURNS TRIGGER
    LANGUAGE plpgsql
AS
$$
BEGIN
    INSERT INTO known_guilds (guild) VALUES (NEW.guild) ON CONFLICT DO NOTHING;
    RETURN NEW;
END;
$$;

CREATE OR REPLACE FUNCTION get_or_insert_config(gid BIGINT, cfg_name TEXT, INOUT res JSONB)
    LANGUAGE plpgsql
    VOLATILE
AS
$$
BEGIN
    INSERT INTO config_values (guild, name, value) VALUES (gid, cfg_name, res)
    ON CONFLICT DO NOTHING;
    SELECT value FROM config_values WHERE guild = gid AND name = cfg_name INTO res;
END;
$$;

CREATE TRIGGER ensure_config_guild
    BEFORE INSERT OR UPDATE
    ON config_values
    FOR EACH ROW
EXECUTE PROCEDURE ensure_guild();

CREATE TRIGGER ensure_joinable_guild
    BEFORE INSERT OR UPDATE
    ON joinable_roles
    FOR EACH ROW
EXECUTE PROCEDURE ensure_guild();

CREATE OR REPLACE FUNCTION joinable_roles_limit()
    RETURNS TRIGGER
    LANGUAGE plpgsql
AS
$$
BEGIN
    UPDATE known_guilds SET joinable_role_cnt = joinable_role_cnt + 1 WHERE guild = NEW.guild;
    RETURN NEW;
END;
$$;

CREATE OR REPLACE FUNCTION update_roles_cnt()
    RETURNS TRIGGER
    LANGUAGE plpgsql
AS
$$
BEGIN
    UPDATE known_guilds SET joinable_role_cnt = joinable_role_cnt - 1 WHERE guild = NEW.guild;
    RETURN NEW;
END;
$$;

CREATE TRIGGER enforce_joinable_roles_limit
    AFTER INSERT
    ON joinable_roles
    FOR EACH ROW
EXECUTE PROCEDURE joinable_roles_limit();

CREATE TRIGGER update_joinable_roles_cnt
    BEFORE DELETE
    ON joinable_roles
    FOR EACH ROW
EXECUTE PROCEDURE update_roles_cnt();
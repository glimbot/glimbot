CREATE OR REPLACE FUNCTION update_roles_cnt()
    RETURNS TRIGGER
    LANGUAGE plpgsql
AS
$$
BEGIN
    UPDATE known_guilds SET joinable_role_cnt = joinable_role_cnt - 1 WHERE guild = OLD.guild;
    RETURN OLD;
END;
$$;

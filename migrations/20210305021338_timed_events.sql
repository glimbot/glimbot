CREATE TABLE timed_events
(
    target_user BIGINT      NOT NULL,
    guild       BIGINT      NOT NULL,
    expiry      TIMESTAMPTZ NOT NULL,
    action      JSONB        NOT NULL
);

CREATE INDEX timed_events_by_guild ON timed_events (guild);
CREATE INDEX timed_events_by_time ON timed_events (expiry);

CREATE TRIGGER ensure_timed_event_guild
    BEFORE INSERT OR UPDATE
    ON timed_events
    FOR EACH ROW
EXECUTE PROCEDURE ensure_guild();

ALTER TABLE timed_events
ADD CONSTRAINT unique_timed_event UNIQUE (target_user, guild, expiry, action);
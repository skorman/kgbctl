-- Remote sync: a local kgbctl pushes broker data into this database over the API.

-- Only users with can_sync may call the /sync/* write endpoints; browser
-- tokens stay read-only.
ALTER TABLE api_users ADD COLUMN can_sync INTEGER NOT NULL DEFAULT 0 CHECK (can_sync IN (0, 1));

-- API user that pushed the run; NULL for runs made locally against this database.
ALTER TABLE sync_runs ADD COLUMN pushed_by TEXT;

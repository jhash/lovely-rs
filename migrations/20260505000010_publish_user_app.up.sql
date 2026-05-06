-- User profile publishing: when public_published_at IS NOT NULL the
-- profile (/{username}) is reachable to anonymous viewers and lists
-- the user's published apps.
ALTER TABLE users ADD COLUMN public_published_at TIMESTAMPTZ;

-- App-level publishing flag: shows up on the user's public profile
-- list when published. Pages already had their own published_at,
-- which still gates per-page visibility.
ALTER TABLE apps ADD COLUMN published_at TIMESTAMPTZ;

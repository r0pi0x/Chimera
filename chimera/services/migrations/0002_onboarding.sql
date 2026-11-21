-- The accounts database, version 2 (docs/plans/active/onboarding.md):
-- invites carry an intent and an intended recipient instead of a
-- username, players choose and may rename their username (the last 10
-- kept), and the account holds the player's avatar.

ALTER TABLE invites ADD COLUMN intent TEXT;          -- why it was made; NULL: an older invite named for its player
ALTER TABLE invites ADD COLUMN recipient TEXT;       -- who it was meant for (the owner's words)
ALTER TABLE accounts ADD COLUMN invite_intent TEXT;  -- the redeemed invite's intent and recipient
ALTER TABLE accounts ADD COLUMN avatar INTEGER;      -- cl_avatar's look (glyph x 8 + colour), NULL: none chosen
ALTER TABLE accounts ADD COLUMN renamed_at INTEGER;  -- the last rename (one a minute)

-- Usernames an account had before, newest first by `until`.
CREATE TABLE username_history (
    account_id BLOB NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    username TEXT NOT NULL,
    until INTEGER NOT NULL
);
CREATE INDEX username_history_account ON username_history(account_id, until);

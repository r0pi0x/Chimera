-- The accounts database, version 1 (docs/plans/active/hub.md, 3.4).
-- Times are Unix seconds; ids are 16 random bytes. Plain SQL, so the
-- schema ports elsewhere if it ever has to.

-- The platform account (Chimera's).
CREATE TABLE accounts (
    id BLOB PRIMARY KEY,
    username TEXT NOT NULL UNIQUE,          -- case-folded, permanent
    display_name TEXT NOT NULL,             -- at most 20 characters
    created INTEGER NOT NULL,
    role TEXT NOT NULL DEFAULT 'tester',    -- tester | host | owner
    note TEXT NOT NULL DEFAULT '',
    content TEXT NOT NULL DEFAULT '*',      -- asset service content: '*' or a,b
    games TEXT NOT NULL DEFAULT '*',        -- hub games: '*' or a,b
    list_slots INTEGER NOT NULL DEFAULT 1,
    report INTEGER NOT NULL DEFAULT 0,
    muted INTEGER NOT NULL DEFAULT 0,
    limits TEXT,                            -- JSON, null: the role's
    recovery_hash TEXT,
    password_hash TEXT,
    state TEXT NOT NULL DEFAULT 'active',   -- active | revoked
    revoked_at INTEGER,
    last_seen INTEGER
);

CREATE TABLE devices (
    id BLOB PRIMARY KEY,
    account_id BLOB NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    kind TEXT NOT NULL,                     -- key
    public_key BLOB UNIQUE,
    token_hash TEXT UNIQUE,
    label TEXT NOT NULL DEFAULT '',
    created INTEGER NOT NULL,
    last_seen INTEGER,
    last_address TEXT,
    revoked_at INTEGER
);
CREATE INDEX devices_account ON devices(account_id);

CREATE TABLE invites (
    code_hash TEXT PRIMARY KEY,             -- SHA-256 of the normalized code
    name TEXT NOT NULL,                     -- the owner's label, the suggested username
    role TEXT NOT NULL,
    content TEXT NOT NULL,
    games TEXT NOT NULL,
    list_slots INTEGER NOT NULL,
    for_account BLOB REFERENCES accounts(id) ON DELETE CASCADE,
    keep_devices INTEGER NOT NULL DEFAULT 0,
    created INTEGER NOT NULL,
    expires INTEGER NOT NULL,
    redeemed_at INTEGER,
    redeemed_device BLOB REFERENCES devices(id) ON DELETE SET NULL,
    revoked_at INTEGER
);

CREATE TABLE link_codes (
    code_hash TEXT PRIMARY KEY,
    account_id BLOB NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    made_by_device BLOB REFERENCES devices(id) ON DELETE SET NULL,
    created INTEGER NOT NULL,
    expires INTEGER NOT NULL,
    used_at INTEGER,
    used_device BLOB REFERENCES devices(id) ON DELETE SET NULL
);

CREATE TABLE bans (
    id INTEGER PRIMARY KEY,
    account_id BLOB REFERENCES accounts(id) ON DELETE CASCADE,
    address TEXT,
    reason TEXT NOT NULL,
    created INTEGER NOT NULL,
    until INTEGER
);

-- The feed the other services poll (`GET /internal/revoked?since=`).
CREATE TABLE revocations (
    seq INTEGER PRIMARY KEY AUTOINCREMENT,
    kind TEXT NOT NULL,                     -- account | device
    target BLOB NOT NULL,
    at INTEGER NOT NULL
);

CREATE TABLE audit (
    seq INTEGER PRIMARY KEY AUTOINCREMENT,
    at INTEGER NOT NULL,
    actor TEXT NOT NULL,                    -- cli | system | <device id>
    action TEXT NOT NULL,
    target TEXT NOT NULL DEFAULT '',
    detail TEXT NOT NULL DEFAULT ''
);

-- Games on the platform (S4): each game's data shaped by its own schema.
CREATE TABLE game_schemas (
    game TEXT NOT NULL,
    version INTEGER NOT NULL,
    schema TEXT NOT NULL,
    hash TEXT NOT NULL,
    registered_by BLOB REFERENCES accounts(id) ON DELETE SET NULL,
    at INTEGER NOT NULL,
    PRIMARY KEY (game, version)
);

CREATE TABLE game_profiles (
    account_id BLOB NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    game TEXT NOT NULL,
    schema_version INTEGER NOT NULL,
    data TEXT NOT NULL DEFAULT '{}',
    created INTEGER NOT NULL,
    updated INTEGER NOT NULL,
    PRIMARY KEY (account_id, game)
);

CREATE TABLE matches (
    id BLOB PRIMARY KEY,
    game TEXT NOT NULL,
    server_account BLOB REFERENCES accounts(id) ON DELETE SET NULL,
    server_device BLOB REFERENCES devices(id) ON DELETE SET NULL,
    instance TEXT NOT NULL,
    minigame TEXT NOT NULL DEFAULT '',
    map TEXT NOT NULL DEFAULT '',
    started INTEGER,
    ended INTEGER,
    length_ms INTEGER,
    received INTEGER NOT NULL,
    ranked INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE match_players (
    match_id BLOB NOT NULL REFERENCES matches(id) ON DELETE CASCADE,
    slot INTEGER NOT NULL,
    account_id BLOB REFERENCES accounts(id) ON DELETE SET NULL,
    name TEXT NOT NULL,
    fields TEXT NOT NULL DEFAULT '{}',
    PRIMARY KEY (match_id, slot)
);

CREATE TABLE stats (
    account_id BLOB NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    game TEXT NOT NULL,
    scope TEXT NOT NULL,
    key TEXT NOT NULL,
    value INTEGER NOT NULL,
    match_id BLOB REFERENCES matches(id) ON DELETE SET NULL,
    updated INTEGER NOT NULL,
    PRIMARY KEY (account_id, game, scope, key)
);

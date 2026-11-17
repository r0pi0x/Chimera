-- The hub's listings (docs/plans/active/hub.md 3.4, 4.2): ephemeral, kept
-- so a restart doesn't empty the list. One row per listed server
-- instance; the heartbeat's fields as JSON (`hub_doc::Heartbeat`,
-- cleaned), the account that lists it and the ticket it last showed.
CREATE TABLE servers (
    instance        TEXT PRIMARY KEY,
    account_id      BLOB NOT NULL,
    device_id       BLOB NOT NULL,
    issued          INTEGER NOT NULL,
    address         TEXT NOT NULL,
    probe           TEXT NOT NULL,
    beat            TEXT NOT NULL,
    reachable       TEXT NOT NULL,
    probed_at       INTEGER NOT NULL DEFAULT 0,
    first_seen      INTEGER NOT NULL,
    last_heartbeat  INTEGER NOT NULL
);
CREATE INDEX servers_account ON servers (account_id);

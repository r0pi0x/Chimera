-- Session-only parties (docs/plans/active/parties.md): one row per live
-- party, its state as JSON (`hub::party::Party`), written on every change
-- so a restart of the hub keeps them. Member tokens are kept as hashes.
CREATE TABLE parties (
    id   TEXT PRIMARY KEY,
    doc  TEXT NOT NULL
);

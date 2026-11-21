-- The accounts database, version 3: a device is a key pair, nothing else
-- (docs/plans/active/hub.md, 5.2). Devices of any other kind go, and so do
-- accounts left with no device (they could never sign in again); both go
-- into the revocation feed first, so tickets they still hold stop at the
-- other services' next poll. `devices.token_hash` stays as an unused,
-- always-null column: SQLite can't drop a UNIQUE column in place, and
-- rebuilding the table would clear the rows that point at its devices.

INSERT INTO revocations (kind, target, at)
SELECT 'device', id,
       MAX(CAST(strftime('%s', 'now') AS INTEGER), COALESCE((SELECT MAX(at) FROM revocations), 0) + 1)
FROM devices WHERE kind != 'key';

INSERT INTO revocations (kind, target, at)
SELECT 'account', a.id,
       MAX(CAST(strftime('%s', 'now') AS INTEGER), COALESCE((SELECT MAX(at) FROM revocations), 0) + 1)
FROM accounts a
WHERE NOT EXISTS (SELECT 1 FROM devices d WHERE d.account_id = a.id AND d.kind = 'key');

DELETE FROM devices WHERE kind != 'key';
UPDATE devices SET token_hash = NULL;
DELETE FROM accounts WHERE NOT EXISTS (SELECT 1 FROM devices d WHERE d.account_id = accounts.id);

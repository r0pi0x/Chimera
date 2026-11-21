//! The database: migrations from nothing and from the previous version
//! (a copy taken first), a newer database refused untouched, a backup
//! restored with the same rows, the backups' retention.

use std::fs;

use chimera_services::db;
use rusqlite::Connection;

use super::scratch;

fn rows(path: &std::path::Path) -> Vec<(String, String)> {
    let conn = Connection::open(path).unwrap();
    let mut stmt = conn.prepare("SELECT username, display_name FROM accounts ORDER BY username").unwrap();
    stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?))).unwrap().collect::<Result<_, _>>().unwrap()
}

#[test]
fn migrations_run_from_nothing_and_from_the_previous_version() {
    let dir = scratch("migrate");
    let (path, backups) = (dir.join("accounts.db"), dir.join("backups"));
    // From nothing: every table, WAL, the version counted.
    let conn = db::open(&path, db::ACCOUNTS_MIGRATIONS, &backups).unwrap();
    assert_eq!(db::version(&conn).unwrap(), db::ACCOUNTS_MIGRATIONS.len());
    let mode: String = conn.query_row("PRAGMA journal_mode", [], |r| r.get(0)).unwrap();
    assert_eq!(mode, "wal");
    for table in ["accounts", "devices", "invites", "link_codes", "bans", "revocations", "audit", "game_profiles", "stats"] {
        let n: i64 =
            conn.query_row("SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1", [table], |r| r.get(0)).unwrap();
        assert_eq!(n, 1, "{table}");
    }
    conn.execute(
        "INSERT INTO accounts (id, username, display_name, created) VALUES (x'00112233445566778899aabbccddeeff', 'alice', 'Alice', 1)",
        [],
    )
    .unwrap();
    drop(conn);
    assert!(!backups.exists(), "nothing to copy before the first migration");
    // A newer binary's migration on this database: a copy first, the rows kept.
    let mut next: Vec<&str> = db::ACCOUNTS_MIGRATIONS.to_vec();
    next.push("ALTER TABLE accounts ADD COLUMN badge TEXT;");
    let conn = db::open(&path, &next, &backups).unwrap();
    assert_eq!(db::version(&conn).unwrap(), next.len());
    conn.query_row("SELECT badge FROM accounts", [], |r| r.get::<_, Option<String>>(0)).unwrap();
    drop(conn);
    assert_eq!(rows(&path), [("alice".to_string(), "Alice".to_string())]);
    let copies: Vec<_> = fs::read_dir(&backups).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    let prefix = format!("accounts-premigrate-v{}-", db::ACCOUNTS_MIGRATIONS.len());
    assert!(copies.len() == 1 && copies[0].starts_with(&prefix), "{copies:?}");
    // That database, back at the older binary: refused, untouched.
    let before = fs::read(&path).unwrap();
    let e = db::open(&path, db::ACCOUNTS_MIGRATIONS, &backups).unwrap_err();
    assert!(e.contains("newer than this binary"), "{e}");
    assert_eq!(fs::read(&path).unwrap(), before);
    let _ = fs::remove_dir_all(&dir);
}

/// A version 1 database (before onboarding) takes version 2's columns
/// and keeps its rows: invites without an intent, accounts without an
/// avatar.
#[test]
fn version_one_moves_to_onboardings_columns() {
    let dir = scratch("migrate-v2");
    let (path, backups) = (dir.join("accounts.db"), dir.join("backups"));
    let conn = db::open(&path, &db::ACCOUNTS_MIGRATIONS[..1], &backups).unwrap();
    conn.execute_batch(
        "INSERT INTO accounts (id, username, display_name, created) VALUES (x'00112233445566778899aabbccddeeff', 'alice', 'Alice', 1);
         INSERT INTO devices (id, account_id, kind, public_key, created) VALUES (x'a1', x'00112233445566778899aabbccddeeff', 'key', x'aa', 1);",
    )
    .unwrap();
    drop(conn);
    let conn = db::open(&path, db::ACCOUNTS_MIGRATIONS, &backups).unwrap();
    let (avatar, intent): (Option<i64>, Option<String>) =
        conn.query_row("SELECT avatar, invite_intent FROM accounts", [], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
    assert_eq!((avatar, intent), (None, None));
    let n: i64 = conn.query_row("SELECT COUNT(*) FROM username_history", [], |r| r.get(0)).unwrap();
    assert_eq!(n, 0);
    drop(conn);
    assert_eq!(rows(&path), [("alice".to_string(), "Alice".to_string())]);
    let _ = fs::remove_dir_all(&dir);
}

/// Version 3 keeps key devices only: any other device goes, and an account
/// left with none goes too; both enter the revocation feed first.
#[test]
fn version_three_keeps_key_devices_only() {
    let dir = scratch("migrate-keys");
    let (path, backups) = (dir.join("accounts.db"), dir.join("backups"));
    let conn = db::open(&path, &db::ACCOUNTS_MIGRATIONS[..2], &backups).unwrap();
    conn.execute_batch(
        "INSERT INTO accounts (id, username, display_name, created) VALUES (x'01', 'alice', 'Alice', 1), (x'02', 'bob', 'Bob', 1);
         INSERT INTO devices (id, account_id, kind, public_key, created) VALUES (x'a1', x'01', 'key', x'aa', 1);
         INSERT INTO devices (id, account_id, kind, token_hash, created) VALUES (x'a2', x'01', 'other', 'h1', 1), (x'b1', x'02', 'other', 'h2', 1);",
    )
    .unwrap();
    drop(conn);
    let conn = db::open(&path, db::ACCOUNTS_MIGRATIONS, &backups).unwrap();
    assert_eq!(rows(&path), [("alice".to_string(), "Alice".to_string())]);
    let devices: i64 = conn.query_row("SELECT COUNT(*) FROM devices WHERE kind = 'key' AND token_hash IS NULL", [], |r| r.get(0)).unwrap();
    let all: i64 = conn.query_row("SELECT COUNT(*) FROM devices", [], |r| r.get(0)).unwrap();
    assert_eq!((devices, all), (1, 1));
    let mut stmt = conn.prepare("SELECT kind, hex(target) FROM revocations ORDER BY kind, target").unwrap();
    let feed: Vec<(String, String)> = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?))).unwrap().collect::<Result<_, _>>().unwrap();
    assert_eq!(feed, [("account".into(), "02".into()), ("device".into(), "A2".into()), ("device".into(), "B1".into())]);
    drop(stmt);
    drop(conn);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_backup_restores_the_same_rows() {
    let dir = scratch("restore");
    let (path, backups) = (dir.join("accounts.db"), dir.join("backups"));
    let conn = db::open(&path, db::ACCOUNTS_MIGRATIONS, &backups).unwrap();
    conn.execute("INSERT INTO accounts (id, username, display_name, created) VALUES (x'01', 'alice', 'Alice', 1)", []).unwrap();
    conn.execute("INSERT INTO accounts (id, username, display_name, created) VALUES (x'02', 'bob', 'Bob', 2)", []).unwrap();
    let copy = backups.join("accounts-20261114.db");
    db::backup(&conn, &copy).unwrap();
    db::integrity(&copy).unwrap();
    // Changed after the backup, then lost.
    conn.execute("DELETE FROM accounts WHERE username = 'bob'", []).unwrap();
    drop(conn);
    assert_eq!(rows(&path).len(), 1);
    db::restore(&copy, &path).unwrap();
    assert_eq!(rows(&path), [("alice".into(), "Alice".into()), ("bob".into(), "Bob".into())]);
    // The restored database opens and migrates as usual.
    let conn = db::open(&path, db::ACCOUNTS_MIGRATIONS, &backups).unwrap();
    assert_eq!(db::version(&conn).unwrap(), db::ACCOUNTS_MIGRATIONS.len());
    // A damaged copy is refused before it replaces anything.
    let bad = backups.join("broken.db");
    fs::write(&bad, b"not a database at all").unwrap();
    drop(conn);
    assert!(db::restore(&bad, &path).is_err());
    assert_eq!(rows(&path).len(), 2);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn backups_keep_two_weeks_daily_then_eight_weekly() {
    let dir = scratch("prune");
    let today = db::days_from_civil(2026, 11, 14);
    assert_eq!(db::ymd(today), "20261114");
    assert_eq!(db::days_from_civil(1970, 1, 1), 0);
    for age in 0..120 {
        fs::write(db::daily_path(&dir, "accounts", today - age), b"x").unwrap();
    }
    fs::write(dir.join("accounts-premigrate-v1-123.db"), b"x").unwrap();
    db::prune(&dir, "accounts", today);
    let mut left: Vec<String> = fs::read_dir(&dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    left.sort();
    assert_eq!(left.len(), 14 + 8 + 1, "{left:?}");
    assert!(left.contains(&"accounts-premigrate-v1-123.db".to_string()));
    assert!(left.contains(&format!("accounts-{}.db", db::ymd(today - 13))));
    assert!(!left.contains(&format!("accounts-{}.db", db::ymd(today - 14 - 8 * 7))));
    let _ = fs::remove_dir_all(&dir);
}

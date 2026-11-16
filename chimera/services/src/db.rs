//! The services' SQLite databases (docs/plans/active/hub.md, 3.2-3.3):
//! opening with the pragmas (WAL, `synchronous=NORMAL`, foreign keys, a
//! 5 s busy timeout), the migrations (numbered SQL files compiled in,
//! `migrations/0001_init.sql`, ..., each applied in one transaction, `PRAGMA
//! user_version` the counter; a database newer than the binary is refused
//! untouched, and a copy is taken before any migration of an existing one),
//! online backups (SQLite's backup API, then `PRAGMA integrity_check` on the
//! copy), the backups' retention (14 daily copies, then one a week for 8
//! weeks) and a restore (the copy checked, then put in place of the
//! database while its service is stopped).
//!
//! One database per service, owned by that process alone.

use std::{
    fs,
    path::{Path, PathBuf},
};

use rusqlite::{Connection, OpenFlags};

/// The accounts database's migrations, in order.
pub const ACCOUNTS_MIGRATIONS: &[&str] = &[include_str!("../migrations/0001_init.sql")];
/// The hub's (`hub.db`).
pub const HUB_MIGRATIONS: &[&str] = &[include_str!("../migrations/hub/0001_init.sql")];
/// Daily backups kept.
pub const KEEP_DAILY: u64 = 14;
/// Weekly backups kept after the daily ones.
pub const KEEP_WEEKLY: u64 = 8;

fn err(what: impl std::fmt::Display, e: impl std::fmt::Display) -> String {
    format!("{what}: {e}")
}

/// The database's schema version.
pub fn version(conn: &Connection) -> Result<usize, String> {
    conn.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
        .map(|v| v as usize)
        .map_err(|e| err("user_version", e))
}

/// Open (or create) the database at `path` with the pragmas and bring it
/// to the newest of `migrations`; an existing database is copied into
/// `backups` before it is migrated.
pub fn open(path: &Path, migrations: &[&str], backups: &Path) -> Result<Connection, String> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| err(dir.display(), e))?;
    }
    let existed = path.exists();
    let mut conn = Connection::open(path).map_err(|e| err(path.display(), e))?;
    let mode: String =
        conn.pragma_update_and_check(None, "journal_mode", "WAL", |r| r.get(0)).map_err(|e| err("journal_mode", e))?;
    if !mode.eq_ignore_ascii_case("wal") {
        return Err(format!("{}: journal_mode is {mode}, not WAL", path.display()));
    }
    conn.execute_batch("PRAGMA synchronous=NORMAL; PRAGMA foreign_keys=ON; PRAGMA busy_timeout=5000;")
        .map_err(|e| err("pragmas", e))?;
    let at = version(&conn)?;
    if at > migrations.len() {
        return Err(format!(
            "{}: the database is version {at}, newer than this binary knows ({}): refusing to touch it; \
             run the newer binary or restore a backup",
            path.display(),
            migrations.len()
        ));
    }
    if existed && at < migrations.len() && at > 0 {
        let copy = backups.join(format!("{}-premigrate-v{at}-{}.db", stem(path), crate::now()));
        backup(&conn, &copy)?;
    }
    migrate(&mut conn, migrations)?;
    crate::restrict(path);
    Ok(conn)
}

fn stem(path: &Path) -> String {
    path.file_stem().map_or("db".into(), |s| s.to_string_lossy().into_owned())
}

/// Apply the migrations past the database's version, each in one
/// transaction.
pub fn migrate(conn: &mut Connection, migrations: &[&str]) -> Result<(), String> {
    let at = version(conn)?;
    for (i, sql) in migrations.iter().enumerate().skip(at) {
        let tx = conn.transaction().map_err(|e| err("migration", e))?;
        tx.execute_batch(sql).map_err(|e| err(format!("migration {}", i + 1), e))?;
        tx.pragma_update(None, "user_version", (i + 1) as i64).map_err(|e| err("user_version", e))?;
        tx.commit().map_err(|e| err(format!("migration {}", i + 1), e))?;
    }
    Ok(())
}

/// An online backup of `conn` to `to` (replaced), checked.
pub fn backup(conn: &Connection, to: &Path) -> Result<(), String> {
    if let Some(dir) = to.parent() {
        fs::create_dir_all(dir).map_err(|e| err(dir.display(), e))?;
    }
    let partial = to.with_extension("partial");
    let _ = fs::remove_file(&partial);
    conn.backup(rusqlite::MAIN_DB, &partial, None::<fn(rusqlite::backup::Progress)>)
        .map_err(|e| err(format!("backup to {}", partial.display()), e))?;
    // One self-contained file: the copy out of WAL mode (its WAL folded in).
    Connection::open(&partial)
        .and_then(|c| c.pragma_update_and_check(None, "journal_mode", "DELETE", |r| r.get::<_, String>(0)))
        .map_err(|e| err(partial.display(), e))?;
    integrity(&partial)?;
    fs::rename(&partial, to).map_err(|e| err(to.display(), e))?;
    crate::restrict(to);
    Ok(())
}

/// `PRAGMA integrity_check` of the database file at `path` (opened read-only).
pub fn integrity(path: &Path) -> Result<(), String> {
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX)
        .map_err(|e| err(path.display(), e))?;
    let result: String =
        conn.query_row("PRAGMA integrity_check", [], |r| r.get(0)).map_err(|e| err(path.display(), e))?;
    if result != "ok" {
        return Err(format!("{}: integrity check: {result}", path.display()));
    }
    Ok(())
}

/// Put the backup `from` in place of the database at `db` (its service
/// stopped): the copy is checked first, the old WAL files removed.
pub fn restore(from: &Path, db: &Path) -> Result<(), String> {
    integrity(from)?;
    let staged = db.with_extension("restoring");
    fs::copy(from, &staged).map_err(|e| err(staged.display(), e))?;
    for suffix in ["-wal", "-shm"] {
        let mut p = db.as_os_str().to_owned();
        p.push(suffix);
        let _ = fs::remove_file(PathBuf::from(p));
    }
    fs::rename(&staged, db).map_err(|e| err(db.display(), e))?;
    crate::restrict(db);
    Ok(())
}

/// Days since the Unix epoch of a civil date (UTC).
pub fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let m = i64::from(month);
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + i64::from(day) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// `YYYYMMDD` of a day since the epoch.
pub fn ymd(days: i64) -> String {
    let text = chimera_assets::accounts::utc_text((days.max(0) as u64) * 86_400);
    text[..10].replace('-', "")
}

/// The nightly backup's file for `day` (days since the epoch).
pub fn daily_path(backups: &Path, name: &str, day: i64) -> PathBuf {
    backups.join(format!("{name}-{}.db", ymd(day)))
}

fn day_of(file: &str, name: &str) -> Option<i64> {
    let digits = file.strip_prefix(name)?.strip_prefix('-')?.strip_suffix(".db")?;
    if digits.len() != 8 || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let (y, m, d) = (digits[..4].parse().ok()?, digits[4..6].parse().ok()?, digits[6..].parse().ok()?);
    Some(days_from_civil(y, m, d))
}

/// Delete `name`'s daily backups past retention on `today`: the last
/// `KEEP_DAILY` days all kept, then the newest of each week for
/// `KEEP_WEEKLY` weeks; pre-migration copies are never deleted. The
/// files deleted.
pub fn prune(backups: &Path, name: &str, today: i64) -> Vec<PathBuf> {
    let mut files: Vec<(i64, PathBuf)> = fs::read_dir(backups)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| Some((day_of(&e.file_name().to_string_lossy(), name)?, e.path())))
        .collect();
    files.sort_by_key(|f| std::cmp::Reverse(f.0));
    let mut weeks_kept = std::collections::BTreeSet::new();
    let mut deleted = Vec::new();
    for (day, path) in files {
        let age = today - day;
        let keep = if age < KEEP_DAILY as i64 {
            true
        } else {
            let week = (age - KEEP_DAILY as i64) / 7;
            week < KEEP_WEEKLY as i64 && weeks_kept.insert(week)
        };
        if !keep && fs::remove_file(&path).is_ok() {
            deleted.push(path);
        }
    }
    deleted
}

use crate::models::{Pm2Process, ServerMetrics};
use chrono::Utc;
use rusqlite::{params, Connection};
use std::sync::{Arc, Mutex};

pub type DbPool = Arc<Mutex<Connection>>;

pub fn init_db(path: &str) -> DbPool {
    let conn = Connection::open(path).expect("Failed to open SQLite database");

    conn.execute_batch(
        "PRAGMA journal_mode=WAL;
        PRAGMA synchronous=NORMAL;
        PRAGMA temp_store=MEMORY;
        PRAGMA busy_timeout=3000;
        CREATE TABLE IF NOT EXISTS history (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            timestamp INTEGER NOT NULL,
            data TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_timestamp ON history(timestamp);
        CREATE TABLE IF NOT EXISTS pm2_history (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            timestamp INTEGER NOT NULL,
            data TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_pm2_timestamp ON pm2_history(timestamp);",
    )
    .expect("Failed to create schema");

    Arc::new(Mutex::new(conn))
}

fn metrics_without_pm2(metrics: &ServerMetrics) -> ServerMetrics {
    let mut compact = metrics.clone();
    compact.pm2_processes.clear();
    compact
}

pub fn insert_metrics_batch(db: &DbPool, metrics_list: &[ServerMetrics]) {
    if metrics_list.is_empty() {
        return;
    }

    let conn = db.lock().expect("DB lock poisoned");
    let tx = match conn.unchecked_transaction() {
        Ok(tx) => tx,
        Err(e) => {
            eprintln!("[db] Failed to start transaction: {}", e);
            return;
        }
    };

    for metrics in metrics_list {
        let compact = metrics_without_pm2(metrics);
        let json = match serde_json::to_string(&compact) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("[db] Failed to serialize metrics: {}", e);
                continue;
            }
        };

        if let Err(e) = tx.execute(
            "INSERT INTO history (timestamp, data) VALUES (?1, ?2)",
            params![metrics.timestamp, json],
        ) {
            eprintln!("[db] Failed to insert history row: {}", e);
        }
    }

    if let Err(e) = tx.commit() {
        eprintln!("[db] Failed to commit history batch: {}", e);
    }
}

pub fn insert_pm2_snapshot(db: &DbPool, timestamp: i64, pm2: &[Pm2Process]) {
    let json = match serde_json::to_string(pm2) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("[db] Failed to serialize PM2 snapshot: {}", e);
            return;
        }
    };

    let conn = db.lock().expect("DB lock poisoned");
    if let Err(e) = conn.execute(
        "INSERT INTO pm2_history (timestamp, data) VALUES (?1, ?2)",
        params![timestamp, json],
    ) {
        eprintln!("[db] Failed to insert PM2 snapshot: {}", e);
    }
}

pub fn get_history(db: &DbPool, hours: u64) -> Vec<ServerMetrics> {
    let cutoff = Utc::now().timestamp() - (hours as i64 * 3600);
    let conn = db.lock().expect("DB lock poisoned");

    let mut stmt = conn
        .prepare("SELECT data FROM history WHERE timestamp >= ?1 ORDER BY timestamp ASC")
        .expect("Failed to prepare query");

    let rows = stmt
        .query_map(params![cutoff], |row| {
            let json_str: String = row.get(0)?;
            Ok(json_str)
        })
        .expect("Failed to query history");

    rows.filter_map(|r| {
        r.ok()
            .and_then(|json_str| serde_json::from_str::<ServerMetrics>(&json_str).ok())
    })
    .collect()
}

pub fn cleanup_old(db: &DbPool, days: u64) {
    let cutoff = Utc::now().timestamp() - (days as i64 * 86400);
    let conn = db.lock().expect("DB lock poisoned");
    let deleted_history = conn
        .execute("DELETE FROM history WHERE timestamp < ?1", params![cutoff])
        .unwrap_or(0);
    let deleted_pm2 = conn
        .execute("DELETE FROM pm2_history WHERE timestamp < ?1", params![cutoff])
        .unwrap_or(0);

    let deleted = deleted_history + deleted_pm2;
    if deleted > 0 {
        eprintln!(
            "[db] Cleaned up {} old records (>{} days)",
            deleted, days
        );
    }
}

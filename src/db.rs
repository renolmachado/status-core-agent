use crate::models::ServerMetrics;
use chrono::Utc;
use rusqlite::{params, Connection};
use std::sync::{Arc, Mutex};

pub type DbPool = Arc<Mutex<Connection>>;

pub fn init_db(path: &str) -> DbPool {
    let conn = Connection::open(path).expect("Failed to open SQLite database");

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS history (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            timestamp INTEGER NOT NULL,
            data TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_timestamp ON history(timestamp);",
    )
    .expect("Failed to create schema");

    Arc::new(Mutex::new(conn))
}

pub fn insert_metrics(db: &DbPool, metrics: &ServerMetrics) {
    let json = serde_json::to_string(metrics).expect("Failed to serialize metrics");
    let conn = db.lock().expect("DB lock poisoned");
    conn.execute(
        "INSERT INTO history (timestamp, data) VALUES (?1, ?2)",
        params![metrics.timestamp, json],
    )
    .ok();
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
    let deleted = conn
        .execute("DELETE FROM history WHERE timestamp < ?1", params![cutoff])
        .unwrap_or(0);

    if deleted > 0 {
        eprintln!("[db] Cleaned up {} old records (>{} days)", deleted, days);
    }
}

mod api;
mod collector;
mod db;
mod models;
mod pm2;

use api::AppState;
use models::ServerMetrics;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{interval, Duration};

const COLLECT_INTERVAL_SECS: u64 = 30;
const CLEANUP_INTERVAL_SECS: u64 = 86400; // 24h
const RETENTION_DAYS: u64 = 7;
const DB_PATH: &str = "metrics.db";
const LISTEN_ADDR: &str = "0.0.0.0:3456";

#[tokio::main]
async fn main() {
    eprintln!("[status-core-agent] Initializing...");

    let db_pool = db::init_db(DB_PATH);
    db::cleanup_old(&db_pool, RETENTION_DAYS);

    let current = Arc::new(RwLock::new(ServerMetrics::default_empty()));

    let state = AppState {
        current: current.clone(),
        db: db_pool.clone(),
    };

    // Collector loop: every 30s
    let collector_current = current.clone();
    let collector_db = db_pool.clone();
    tokio::spawn(async move {
        let mut tick = interval(Duration::from_secs(COLLECT_INTERVAL_SECS));
        loop {
            tick.tick().await;
            let metrics = collector::collect_metrics().await;
            {
                let mut w = collector_current.write().await;
                *w = metrics.clone();
            }
            db::insert_metrics(&collector_db, &metrics);
            eprintln!(
                "[collector] CPU: {:.1}% | RAM: {}/{} MB | Temp: {:.1}°C | PM2: {} procs",
                metrics.cpu_load_avg,
                metrics.ram_used / 1_048_576,
                metrics.ram_total / 1_048_576,
                metrics.temp_celsius,
                metrics.pm2_processes.len()
            );
        }
    });

    // Cleanup loop: every 24h
    let cleanup_db = db_pool.clone();
    tokio::spawn(async move {
        let mut tick = interval(Duration::from_secs(CLEANUP_INTERVAL_SECS));
        loop {
            tick.tick().await;
            db::cleanup_old(&cleanup_db, RETENTION_DAYS);
        }
    });

    let router = api::build_router(state);

    eprintln!("[status-core-agent] Listening on {}", LISTEN_ADDR);
    let listener = tokio::net::TcpListener::bind(LISTEN_ADDR)
        .await
        .expect("Failed to bind to address");

    axum::serve(listener, router)
        .await
        .expect("Server error");
}

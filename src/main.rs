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

const COLLECT_INTERVAL_SECS: u64 = 15;
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

    let collector_current = current.clone();
    let collector_db = db_pool.clone();
    tokio::spawn(async move {
        let mut col = collector::Collector::new();
        let mut tick = interval(Duration::from_secs(COLLECT_INTERVAL_SECS));
        loop {
            tick.tick().await;
            let m = col.collect().await;
            {
                let mut w = collector_current.write().await;
                *w = m.clone();
            }
            db::insert_metrics(&collector_db, &m);
            eprintln!(
                "[collector] HP:{} | CPU:{:.1}% avg{:.2} cores {}/{} | RAM:{}/{}M | Swap:{}/{}M | Disk:{}/{}G | {:.1}°C | PM2:{}",
                m.health_score,
                m.cpu_usage,
                m.cpu_load_avg,
                m.cpu_cores_online,
                m.cpu_cores_total,
                m.ram_used / 1_048_576,
                m.ram_total / 1_048_576,
                m.swap_used / 1_048_576,
                m.swap_total / 1_048_576,
                m.disk_available / 1_073_741_824,
                m.disk_total / 1_073_741_824,
                m.temp_celsius,
                m.pm2_processes.len()
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

mod api;
mod collector;
mod db;
mod models;
mod pm2;

use api::AppState;
use models::ServerMetrics;
use std::env;
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;
use tokio::net::TcpSocket;
use tokio::sync::RwLock;
use tokio::time::{interval, sleep, Duration};

const COLLECT_INTERVAL_SECS: u64 = 60;
const CLEANUP_INTERVAL_SECS: u64 = 86400; // 24h
const RETENTION_DAYS: u64 = 7;
const DB_PATH: &str = "metrics.db";
const DEFAULT_LISTEN_ADDR: &str = "0.0.0.0:3002";
/// Retries when the port is still in TIME_WAIT or the old PID has not released yet.
const BIND_RETRIES: u32 = 30;
const BIND_RETRY_DELAY_MS: u64 = 1000;

fn listen_addr_from_env() -> String {
    env::var("STATUS_CORE_LISTEN")
        .or_else(|_| env::var("LISTEN_ADDR"))
        .unwrap_or_else(|_| DEFAULT_LISTEN_ADDR.to_string())
}

fn bind_listener(addr: SocketAddr) -> Result<tokio::net::TcpListener, std::io::Error> {
    let socket = if addr.is_ipv4() {
        TcpSocket::new_v4()?
    } else {
        TcpSocket::new_v6()?
    };
    socket.set_reuseaddr(true)?;
    socket.bind(addr)?;
    socket.listen(1024)
}

async fn bind_with_retries(addr_str: &str) -> tokio::net::TcpListener {
    let addr = SocketAddr::from_str(addr_str).unwrap_or_else(|e| {
        eprintln!(
            "[status-core-agent] Invalid listen address {:?}: {}",
            addr_str, e
        );
        std::process::exit(1);
    });

    for attempt in 1..=BIND_RETRIES {
        match bind_listener(addr) {
            Ok(listener) => return listener,
            Err(e)
                if e.kind() == std::io::ErrorKind::AddrInUse && attempt < BIND_RETRIES =>
            {
                eprintln!(
                    "[status-core-agent] Address in use (try {}/{}), retrying in {} ms — {}",
                    attempt, BIND_RETRIES, BIND_RETRY_DELAY_MS, e
                );
                sleep(Duration::from_millis(BIND_RETRY_DELAY_MS)).await;
            }
            Err(e) => {
                eprintln!(
                    "[status-core-agent] Failed to bind {}: {}",
                    addr_str, e
                );
                eprintln!(
                    "[status-core-agent] Hint: another process holds this port, or PM2 started a second copy. Check `pm2 list`, stop duplicates, or set STATUS_CORE_LISTEN (e.g. 0.0.0.0:3003)."
                );
                std::process::exit(1);
            }
        }
    }

    unreachable!("bind_with_retries always returns or exits");
}

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
            let (m, log_now) = col.collect().await;
            {
                let mut w = collector_current.write().await;
                *w = m.clone();
            }
            let db = collector_db.clone();
            let m_db = m.clone();
            let _ = tokio::task::spawn_blocking(move || {
                db::insert_metrics(&db, &m_db);
            })
            .await;

            if log_now {
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
        }
    });

    // Cleanup loop: every 24h
    let cleanup_db = db_pool.clone();
    tokio::spawn(async move {
        let mut tick = interval(Duration::from_secs(CLEANUP_INTERVAL_SECS));
        loop {
            tick.tick().await;
            let db = cleanup_db.clone();
            let _ = tokio::task::spawn_blocking(move || {
                db::cleanup_old(&db, RETENTION_DAYS);
            })
            .await;
        }
    });

    let router = api::build_router(state);

    let listen_addr = listen_addr_from_env();
    eprintln!("[status-core-agent] Listening on {}", listen_addr);
    let listener = bind_with_retries(&listen_addr).await;

    axum::serve(listener, router)
        .await
        .expect("Server error");
}

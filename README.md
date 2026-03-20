# StatusCore Server Agent

Lightweight system monitoring agent written in Rust for Samsung Galaxy S10 (AArch64) running Debian via proot-distro.

Collects CPU, RAM, swap, disk, temperature, battery, and PM2 process metrics every 15 seconds, computes a health score, persists everything to SQLite, and exposes an HTTP API for the StatusCore frontend.

## Build

Compile directly on the S10 (inside Debian/proot):

```bash
# Install Rust if not already present
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# Build optimized binary
cargo build --release
```

The binary will be at `target/release/status-core-agent`.

## Run

```bash
./target/release/status-core-agent
```

The agent starts on `0.0.0.0:3456` by default. SQLite database is stored as `metrics.db` in the working directory.

## API Endpoints

### GET /api/v1/current

Returns the latest collected metrics.

```json
{
  "timestamp": 1711000000,
  "cpu_usage": 12.5,
  "cpu_load_avg": 2.5,
  "cpu_cores_online": 6,
  "cpu_cores_total": 8,
  "ram_used": 3221225472,
  "ram_total": 6442450944,
  "swap_used": 125829120,
  "swap_total": 2147483648,
  "disk_available": 25769803776,
  "disk_total": 59055800320,
  "temp_celsius": 38.0,
  "battery_level": 85,
  "health_score": 100,
  "pm2_processes": [
    {
      "name": "my-app",
      "status": "online",
      "cpu_percent": 12.5,
      "memory_bytes": 52428800,
      "restarts": 3
    }
  ]
}
```

### GET /api/v1/history?hours=24

Returns an array of metrics for the specified time window (default: 24 hours).

## Metrics

### Health Score (0-100)

Composite score computed from temperature and RAM pressure:

| Factor | Threshold | Penalty |
|---|---|---|
| Temperature | > 45°C | Linear ramp up to -50 at 80°C |
| RAM usage | > 85% | Linear ramp up to -50 at 100% |

A score of 100 means the system is healthy. A score below 50 signals critical conditions.

### Throttling Watchdog

`cpu_cores_online` vs `cpu_cores_total` reveals thermal/power throttling. If the S10 reports 4/8 cores online, performance has dropped to half regardless of what load average shows. Total cores are read from `/sys/devices/system/cpu/possible`.

### Swap Usage

`swap_used` / `swap_total` tracks ZRAM/swap pressure. When swap usage climbs past 50%, an OOM kill (Signal 9) is imminent. This is the best early warning for Android memory pressure.

### Disk / Storage Health

`disk_available` / `disk_total` monitors internal storage (UFS). Prioritizes the `/data` mount on Android, falls back to `/`. Catches PM2 logs or database data filling up storage before the system locks.

## Configuration

Constants are defined in `src/main.rs`:

| Constant | Default | Description |
|---|---|---|
| `COLLECT_INTERVAL_SECS` | 15 | Collection interval in seconds |
| `CLEANUP_INTERVAL_SECS` | 86400 | Cleanup interval (24h) |
| `RETENTION_DAYS` | 7 | Days to keep history |
| `DB_PATH` | `metrics.db` | SQLite database path |
| `LISTEN_ADDR` | `0.0.0.0:3456` | HTTP listen address |

## Architecture

The collector reuses a persistent `System` object across ticks, calling only targeted refreshes (`refresh_cpu_all`, `refresh_memory`) instead of rebuilding the full system snapshot. This keeps CPU usage under 2%.

```
main.rs          Spawns collector loop + cleanup loop + HTTP server
collector.rs     Collector struct (owns System, Components, Disks)
models.rs        ServerMetrics / Pm2Process structs
api.rs           Axum routes (/current, /history)
db.rs            SQLite persistence (insert, query, cleanup)
pm2.rs           PM2 jlist parser (Termux-aware path resolution)
```

## Data Sources

| Metric | Source |
|---|---|
| CPU usage | `sysinfo` crate (`global_cpu_usage`) |
| CPU load avg | `sysinfo` crate (1-min load average) |
| CPU cores online | `sysinfo` crate (refreshed each tick) |
| CPU cores total | `/sys/devices/system/cpu/possible` |
| RAM | `sysinfo` crate (`used_memory` / `total_memory`) |
| Swap | `sysinfo` crate (`used_swap` / `total_swap`) |
| Disk | `sysinfo` Disks (prefers `/data`, falls back to `/`) |
| Temperature | `sysinfo` Components, fallback to `/sys/class/thermal/thermal_zone0/temp` |
| Battery | `/sys/class/power_supply/battery/capacity` |
| Health score | Computed from temperature + RAM usage |
| PM2 processes | `pm2 jlist` command output |

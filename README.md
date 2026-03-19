# StatusCore Server Agent

Lightweight system monitoring agent written in Rust for Samsung Galaxy S10 (AArch64) running Debian via proot-distro.

Collects CPU, RAM, temperature, battery, and PM2 process metrics every 30 seconds, persists them to SQLite, and exposes an HTTP API for the StatusCore frontend.

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
  "cpu_load_avg": 2.5,
  "cpu_cores_online": 8,
  "ram_used": 3221225472,
  "ram_total": 6442450944,
  "temp_celsius": 38.0,
  "battery_level": 85,
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

## Configuration

Constants are defined in `src/main.rs`:

| Constant | Default | Description |
|---|---|---|
| `COLLECT_INTERVAL_SECS` | 30 | Collection interval in seconds |
| `CLEANUP_INTERVAL_SECS` | 86400 | Cleanup interval (24h) |
| `RETENTION_DAYS` | 7 | Days to keep history |
| `DB_PATH` | `metrics.db` | SQLite database path |
| `LISTEN_ADDR` | `0.0.0.0:3456` | HTTP listen address |

## Data Sources

| Metric | Source |
|---|---|
| CPU load | `sysinfo` crate (1-min load average) |
| CPU cores | `sysinfo` crate (online core count) |
| RAM | `sysinfo` crate |
| Temperature | `sysinfo` Components, fallback to `/sys/class/thermal/thermal_zone0/temp` |
| Battery | `/sys/class/power_supply/battery/capacity` |
| PM2 processes | `pm2 jlist` command output |

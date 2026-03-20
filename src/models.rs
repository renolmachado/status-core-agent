use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pm2Process {
    pub name: String,
    pub status: String,
    pub cpu_percent: f32,
    pub memory_bytes: u64,
    pub restarts: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerMetrics {
    pub timestamp: i64,
    pub cpu_usage: f32,
    pub cpu_load_avg: f32,
    pub cpu_cores_online: usize,
    pub cpu_cores_total: usize,
    pub ram_used: u64,
    pub ram_total: u64,
    pub swap_used: u64,
    pub swap_total: u64,
    pub disk_available: u64,
    pub disk_total: u64,
    pub temp_celsius: f32,
    pub battery_level: u8,
    pub health_score: u8,
    pub pm2_processes: Vec<Pm2Process>,
}

impl ServerMetrics {
    pub fn default_empty() -> Self {
        Self {
            timestamp: 0,
            cpu_usage: 0.0,
            cpu_load_avg: 0.0,
            cpu_cores_online: 0,
            cpu_cores_total: 0,
            ram_used: 0,
            ram_total: 0,
            swap_used: 0,
            swap_total: 0,
            disk_available: 0,
            disk_total: 0,
            temp_celsius: 0.0,
            battery_level: 0,
            health_score: 100,
            pm2_processes: Vec::new(),
        }
    }
}

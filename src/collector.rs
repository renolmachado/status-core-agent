use crate::models::ServerMetrics;
use crate::pm2;
use chrono::Utc;
use sysinfo::{Components, Disks, System};
use std::path::Path;

pub struct Collector {
    sys: System,
    components: Components,
    disks: Disks,
    cpu_cores_total: usize,
}

impl Collector {
    pub fn new() -> Self {
        let mut sys = System::new();
        sys.refresh_cpu_all();
        sys.refresh_memory();

        let components = Components::new_with_refreshed_list();
        let disks = Disks::new_with_refreshed_list();

        let cores_online = sys.cpus().len();
        let cpu_cores_total = read_cpu_total(cores_online);

        Self { sys, components, disks, cpu_cores_total }
    }

    pub async fn collect(&mut self) -> ServerMetrics {
        self.sys.refresh_cpu_all();
        self.sys.refresh_memory();
        self.components.refresh(true);
        self.disks.refresh(true);

        let cpu_usage = self.sys.global_cpu_usage();
        let cpu_load_avg = System::load_average().one as f32;
        let cpu_cores_online = self.sys.cpus().len();

        let ram_used = self.sys.used_memory();
        let ram_total = self.sys.total_memory();
        let swap_used = self.sys.used_swap();
        let swap_total = self.sys.total_swap();

        let (disk_available, disk_total) = disk_usage_from(&self.disks);

        let temp_celsius = temp_from_components(&self.components);
        let battery_level = read_battery();
        let pm2_processes = pm2::collect_pm2().await;

        let health_score = compute_health_score(temp_celsius, ram_used, ram_total);

        ServerMetrics {
            timestamp: Utc::now().timestamp(),
            cpu_usage,
            cpu_load_avg,
            cpu_cores_online,
            cpu_cores_total: self.cpu_cores_total,
            ram_used,
            ram_total,
            swap_used,
            swap_total,
            disk_available,
            disk_total,
            temp_celsius,
            battery_level,
            health_score,
            pm2_processes,
        }
    }
}

fn compute_health_score(temp: f32, ram_used: u64, ram_total: u64) -> u8 {
    let mut score: f32 = 100.0;

    // Temperature penalty: linear ramp from 45°C (0 penalty) to 80°C (−50)
    if temp > 45.0 {
        let penalty = ((temp - 45.0) / 35.0 * 50.0).min(50.0);
        score -= penalty;
    }

    // RAM penalty: linear ramp from 85% (0 penalty) to 100% (−50)
    if ram_total > 0 {
        let ram_pct = ram_used as f32 / ram_total as f32 * 100.0;
        if ram_pct > 85.0 {
            let penalty = ((ram_pct - 85.0) / 15.0 * 50.0).min(50.0);
            score -= penalty;
        }
    }

    score.clamp(0.0, 100.0) as u8
}

fn temp_from_components(components: &Components) -> f32 {
    for component in components {
        if let Some(temp) = component.temperature() {
            if temp > 0.0 {
                return temp;
            }
        }
    }
    read_thermal_zone_fallback()
}

fn read_thermal_zone_fallback() -> f32 {
    let path = Path::new("/sys/class/thermal/thermal_zone0/temp");
    match std::fs::read_to_string(path) {
        Ok(content) => content.trim().parse::<f32>().unwrap_or(0.0) / 1000.0,
        Err(_) => 0.0,
    }
}

fn read_battery() -> u8 {
    let path = Path::new("/sys/class/power_supply/battery/capacity");
    match std::fs::read_to_string(path) {
        Ok(content) => content.trim().parse::<u8>().unwrap_or(0),
        Err(_) => 0,
    }
}

fn read_cpu_total(online_fallback: usize) -> usize {
    let path = Path::new("/sys/devices/system/cpu/possible");
    match std::fs::read_to_string(path) {
        Ok(content) => parse_cpu_range(&content).unwrap_or(online_fallback),
        Err(_) => online_fallback,
    }
}

fn parse_cpu_range(s: &str) -> Option<usize> {
    let s = s.trim();
    if let Some((_start, end)) = s.split_once('-') {
        let max: usize = end.parse().ok()?;
        Some(max + 1)
    } else {
        s.parse::<usize>().ok().map(|_| 1)
    }
}

fn disk_usage_from(disks: &Disks) -> (u64, u64) {
    let preferred = ["/data", "/"];

    for target in &preferred {
        for disk in disks.list() {
            if disk.mount_point().to_str() == Some(target) {
                return (disk.available_space(), disk.total_space());
            }
        }
    }

    if let Some(disk) = disks.list().first() {
        return (disk.available_space(), disk.total_space());
    }

    (0, 0)
}

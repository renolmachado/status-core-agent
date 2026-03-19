use crate::models::ServerMetrics;
use crate::pm2;
use chrono::Utc;
use sysinfo::{Components, System};
use std::path::Path;

pub async fn collect_metrics() -> ServerMetrics {
    let mut sys = System::new_all();
    std::thread::sleep(std::time::Duration::from_millis(200));
    sys.refresh_all();

    let cpu_load_avg = {
        let load = System::load_average();
        load.one as f32
    };

    let cpu_cores_online = sys.cpus().len();

    let ram_used = sys.used_memory();
    let ram_total = sys.total_memory();

    let temp_celsius = read_temperature();
    let battery_level = read_battery();
    let pm2_processes = pm2::collect_pm2().await;

    ServerMetrics {
        timestamp: Utc::now().timestamp(),
        cpu_load_avg,
        cpu_cores_online,
        ram_used,
        ram_total,
        temp_celsius,
        battery_level,
        pm2_processes,
    }
}

fn read_temperature() -> f32 {
    let components = Components::new_with_refreshed_list();
    for component in &components {
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
        Ok(content) => {
            content
                .trim()
                .parse::<f32>()
                .unwrap_or(0.0)
                / 1000.0
        }
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

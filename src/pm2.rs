use crate::models::Pm2Process;
use std::path::Path;
use std::sync::OnceLock;
use tokio::process::Command;

const TERMUX_PREFIX: &str = "/data/data/com.termux/files/usr";

fn pm2_binary() -> &'static str {
    static BIN: OnceLock<String> = OnceLock::new();
    BIN.get_or_init(|| {
        let termux_pm2 = format!("{}/bin/pm2", TERMUX_PREFIX);
        if Path::new(&termux_pm2).exists() {
            eprintln!("[pm2] Using Termux PM2: {}", termux_pm2);
            return termux_pm2;
        }

        let nvm_dir = "/data/data/com.termux/files/home/.nvm/versions/node";
        if let Ok(entries) = std::fs::read_dir(nvm_dir) {
            for entry in entries.flatten() {
                let pm2_path = entry.path().join("bin/pm2");
                if pm2_path.exists() {
                    let p = pm2_path.to_string_lossy().to_string();
                    eprintln!("[pm2] Using Termux nvm PM2: {}", p);
                    return p;
                }
            }
        }

        "pm2".to_string()
    })
}

pub async fn collect_pm2() -> Vec<Pm2Process> {
    match run_pm2_jlist().await {
        Ok(processes) => processes,
        Err(_) => Vec::new(),
    }
}

async fn run_pm2_jlist() -> Result<Vec<Pm2Process>, Box<dyn std::error::Error>> {
    let output = Command::new(pm2_binary())
        .arg("jlist")
        .output()
        .await?;

    if !output.status.success() {
        return Ok(Vec::new());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout)?;

    let arr = parsed.as_array().ok_or("pm2 jlist did not return an array")?;

    let mut processes = Vec::with_capacity(arr.len());
    for entry in arr {
        let name = entry["name"]
            .as_str()
            .unwrap_or("unknown")
            .to_string();

        let status = entry["pm2_env"]["status"]
            .as_str()
            .unwrap_or("unknown")
            .to_string();

        let cpu_percent = entry["monit"]["cpu"]
            .as_f64()
            .unwrap_or(0.0) as f32;

        let memory_bytes = entry["monit"]["memory"]
            .as_u64()
            .unwrap_or(0);

        let restarts = entry["pm2_env"]["restart_time"]
            .as_u64()
            .unwrap_or(0) as u32;

        processes.push(Pm2Process {
            name,
            status,
            cpu_percent,
            memory_bytes,
            restarts,
        });
    }

    Ok(processes)
}

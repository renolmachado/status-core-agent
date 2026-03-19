use crate::models::Pm2Process;
use tokio::process::Command;

pub async fn collect_pm2() -> Vec<Pm2Process> {
    match run_pm2_jlist().await {
        Ok(processes) => processes,
        Err(_) => Vec::new(),
    }
}

async fn run_pm2_jlist() -> Result<Vec<Pm2Process>, Box<dyn std::error::Error>> {
    let output = Command::new("pm2")
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

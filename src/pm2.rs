use crate::models::Pm2Process;
use std::path::Path;
use std::sync::OnceLock;
use tokio::process::Command;

const TERMUX_PREFIX: &str = "/data/data/com.termux/files/usr";
const TERMUX_HOME: &str = "/data/data/com.termux/files/home";

struct Pm2Config {
    bin: String,
    home: Option<String>,
}

fn pm2_config() -> &'static Pm2Config {
    static CFG: OnceLock<Pm2Config> = OnceLock::new();
    CFG.get_or_init(|| {
        let termux_pm2_home = format!("{}/.pm2", TERMUX_HOME);

        let termux_pm2 = format!("{}/bin/pm2", TERMUX_PREFIX);
        if Path::new(&termux_pm2).exists() {
            eprintln!("[pm2] Using Termux PM2: {}", termux_pm2);
            eprintln!("[pm2] PM2_HOME: {}", termux_pm2_home);
            return Pm2Config { bin: termux_pm2, home: Some(termux_pm2_home) };
        }

        let nvm_dir = format!("{}/.nvm/versions/node", TERMUX_HOME);
        if let Ok(entries) = std::fs::read_dir(&nvm_dir) {
            for entry in entries.flatten() {
                let pm2_path = entry.path().join("bin/pm2");
                if pm2_path.exists() {
                    let p = pm2_path.to_string_lossy().to_string();
                    eprintln!("[pm2] Using Termux nvm PM2: {}", p);
                    eprintln!("[pm2] PM2_HOME: {}", termux_pm2_home);
                    return Pm2Config { bin: p, home: Some(termux_pm2_home) };
                }
            }
        }

        eprintln!("[pm2] Termux PM2 not found, falling back to system PATH");
        Pm2Config { bin: "pm2".to_string(), home: None }
    })
}

pub async fn collect_pm2() -> Vec<Pm2Process> {
    match run_pm2_jlist().await {
        Ok(processes) => processes,
        Err(e) => {
            eprintln!("[pm2] Error: {}", e);
            Vec::new()
        }
    }
}

async fn run_pm2_jlist() -> Result<Vec<Pm2Process>, Box<dyn std::error::Error>> {
    let cfg = pm2_config();
    eprintln!("[pm2] Running: {} jlist", cfg.bin);

    let mut cmd = Command::new(&cfg.bin);
    cmd.arg("jlist");
    if let Some(home) = &cfg.home {
        cmd.env("PM2_HOME", home);
    }

    let output = cmd.output().await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        eprintln!("[pm2] Command failed (exit {}): {}", output.status, stderr);
        return Ok(Vec::new());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    // PM2 can prefix stdout with non-JSON text (ANSI codes, warnings);
    // find the actual JSON array start.
    let json_str = stdout
        .find('[')
        .map(|i| &stdout[i..])
        .unwrap_or(&stdout);

    let parsed: serde_json::Value = serde_json::from_str(json_str)?;

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

/// System monitoring daemon for LED badge
use anyhow::Result;
use serde::Deserialize;
use sysinfo::{Disks, System};
use std::time::Duration;
use tokio::time::sleep;

use crate::protocol::{DisplayMode, MessageConfig, ProtocolHeader};
use crate::renderer::render_text;
use crate::usb::UsbConnection;

/// Configuration for the monitor daemon
#[derive(Clone)]
pub struct MonitorConfig {
    pub interval_secs: u64,
    pub cpu_warn: u8,
    pub cpu_crit: u8,
    pub mem_warn: u8,
    pub mem_crit: u8,
    pub disk_warn: u8,
    pub disk_crit: u8,
    pub gpu_warn: u8,
    pub gpu_crit: u8,
    pub ollama_url: String,
    pub idle_message: Option<String>,
}

impl Default for MonitorConfig {
    fn default() -> Self {
        Self {
            interval_secs: 5,
            cpu_warn: 80,
            cpu_crit: 95,
            mem_warn: 80,
            mem_crit: 95,
            disk_warn: 80,
            disk_crit: 95,
            gpu_warn: 80,
            gpu_crit: 95,
            ollama_url: "http://localhost:11434".to_string(),
            idle_message: None,
        }
    }
}

/// Alert severity level
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AlertLevel {
    Normal,
    Info,
    Warning,
    Critical,
}

/// An alert to display on the badge
#[derive(Debug, Clone)]
pub struct Alert {
    pub priority: u8,
    pub icon: &'static str,
    pub message: String,
    pub level: AlertLevel,
}

impl Alert {
    fn normal(message: String) -> Self {
        Self {
            priority: 255,
            icon: "",
            message,
            level: AlertLevel::Normal,
        }
    }

    fn info(priority: u8, icon: &'static str, message: String) -> Self {
        Self {
            priority,
            icon,
            message,
            level: AlertLevel::Info,
        }
    }

    fn warning(priority: u8, icon: &'static str, message: String) -> Self {
        Self {
            priority,
            icon,
            message,
            level: AlertLevel::Warning,
        }
    }

    fn critical(priority: u8, icon: &'static str, message: String) -> Self {
        Self {
            priority,
            icon,
            message,
            level: AlertLevel::Critical,
        }
    }

    /// Format the alert as a badge message
    pub fn to_badge_message(&self) -> String {
        if self.icon.is_empty() {
            self.message.clone()
        } else {
            format!(":{}:{}", self.icon, self.message)
        }
    }
}

/// Ollama API response for /api/ps
#[derive(Debug, Deserialize)]
struct OllamaModelsResponse {
    models: Option<Vec<OllamaModel>>,
}

#[derive(Debug, Deserialize)]
struct OllamaModel {
    name: String,
}

/// Collect CPU alert
fn collect_cpu_alert(sys: &System, config: &MonitorConfig) -> Option<Alert> {
    let cpu_info = sys.global_cpu_info();
    let usage_pct = cpu_info.cpu_usage() as u8;

    if usage_pct >= config.cpu_crit {
        Some(Alert::critical(4, "cpu", format!(" CRIT {}%", usage_pct)))
    } else if usage_pct >= config.cpu_warn {
        Some(Alert::warning(8, "cpu", format!(" {}%", usage_pct)))
    } else {
        None
    }
}

/// Collect memory alert
fn collect_memory_alert(sys: &System, config: &MonitorConfig) -> Option<Alert> {
    let total = sys.total_memory();
    let used = sys.used_memory();
    if total == 0 {
        return None;
    }
    let usage_pct = ((used as f64 / total as f64) * 100.0) as u8;

    if usage_pct >= config.mem_crit {
        Some(Alert::critical(2, "memory", format!(" CRIT {}%", usage_pct)))
    } else if usage_pct >= config.mem_warn {
        Some(Alert::warning(6, "memory", format!(" {}%", usage_pct)))
    } else {
        None
    }
}

/// Collect disk alert
fn collect_disk_alert(config: &MonitorConfig) -> Option<Alert> {
    let disks = Disks::new_with_refreshed_list();

    // Find root filesystem
    for disk in disks.list() {
        if disk.mount_point().to_str() == Some("/") {
            let total = disk.total_space();
            let available = disk.available_space();
            if total == 0 {
                continue;
            }
            let used = total - available;
            let usage_pct = ((used as f64 / total as f64) * 100.0) as u8;

            if usage_pct >= config.disk_crit {
                return Some(Alert::critical(1, "disk", " FULL!".to_string()));
            } else if usage_pct >= config.disk_warn {
                return Some(Alert::warning(9, "disk", format!(" {}%", usage_pct)));
            }
        }
    }
    None
}

/// Collect GPU alert (NVIDIA only)
#[cfg(feature = "nvidia")]
fn collect_gpu_alert(nvml: &Option<nvml_wrapper::Nvml>, config: &MonitorConfig) -> Option<Alert> {
    let nvml = nvml.as_ref()?;
    let device = nvml.device_by_index(0).ok()?;
    let utilization = device.utilization_rates().ok()?;
    let usage_pct = utilization.gpu as u8;

    if usage_pct >= config.gpu_crit {
        Some(Alert::critical(3, "gpu", format!(" CRIT {}%", usage_pct)))
    } else if usage_pct >= config.gpu_warn {
        Some(Alert::warning(7, "gpu", format!(" {}%", usage_pct)))
    } else {
        None
    }
}

#[cfg(not(feature = "nvidia"))]
fn collect_gpu_alert(_nvml: &Option<()>, _config: &MonitorConfig) -> Option<Alert> {
    None
}

/// Check ollama for loaded models
async fn collect_ollama_alert(config: &MonitorConfig) -> Option<Alert> {
    let url = format!("{}/api/ps", config.ollama_url);
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .ok()?;

    let response = client.get(&url).send().await.ok()?;
    let data: OllamaModelsResponse = response.json().await.ok()?;

    if let Some(models) = data.models {
        if let Some(model) = models.first() {
            // Extract just the model name without tag
            let name = model.name.split(':').next().unwrap_or(&model.name);
            return Some(Alert::info(5, "info", format!(" {}", name)));
        }
    }
    None
}

/// Collect all alerts from all sources
async fn collect_all_alerts(
    sys: &System,
    #[cfg(feature = "nvidia")] nvml: &Option<nvml_wrapper::Nvml>,
    #[cfg(not(feature = "nvidia"))] _nvml: &Option<()>,
    config: &MonitorConfig,
) -> Vec<Alert> {
    let mut alerts = Vec::new();

    if let Some(alert) = collect_cpu_alert(sys, config) {
        alerts.push(alert);
    }
    if let Some(alert) = collect_memory_alert(sys, config) {
        alerts.push(alert);
    }
    if let Some(alert) = collect_disk_alert(config) {
        alerts.push(alert);
    }
    #[cfg(feature = "nvidia")]
    if let Some(alert) = collect_gpu_alert(nvml, config) {
        alerts.push(alert);
    }
    #[cfg(not(feature = "nvidia"))]
    {
        let _ = collect_gpu_alert(&None, config);
    }
    if let Some(alert) = collect_ollama_alert(config).await {
        alerts.push(alert);
    }

    alerts
}

/// Send a message to the badge
fn send_to_badge(usb: &UsbConnection, message: &str) -> Result<()> {
    let bitmap = render_text(message);

    let config = MessageConfig {
        speed: 4,
        mode: DisplayMode::ScrollLeft,
        blink: false,
        animated_border: false,
    };

    let header = ProtocolHeader::new()
        .brightness(100)
        .add_message(config, bitmap.width_columns)
        .build();

    let mut payload = header.to_vec();
    payload.extend_from_slice(&bitmap.data);

    usb.write(None, &payload)?;
    Ok(())
}

/// Get the idle message (hostname or custom)
fn get_idle_message(config: &MonitorConfig) -> String {
    if let Some(ref msg) = config.idle_message {
        msg.clone()
    } else {
        gethostname::gethostname()
            .to_string_lossy()
            .to_string()
    }
}

/// Run the monitor daemon
pub async fn run_monitor(config: MonitorConfig, usb: UsbConnection) -> Result<()> {
    let mut sys = System::new();

    #[cfg(feature = "nvidia")]
    let nvml = nvml_wrapper::Nvml::init().ok();
    #[cfg(not(feature = "nvidia"))]
    let nvml: Option<()> = None;

    let mut last_alert: Option<String> = None;
    let mut showing_idle = false;

    // Initial CPU reading (first reading is inaccurate)
    sys.refresh_cpu_usage();
    sleep(Duration::from_millis(500)).await;

    println!("LED Badge Monitor started");
    println!("  Interval: {}s", config.interval_secs);
    println!("  CPU warn/crit: {}%/{}%", config.cpu_warn, config.cpu_crit);
    println!("  Memory warn/crit: {}%/{}%", config.mem_warn, config.mem_crit);
    println!("  Disk warn/crit: {}%/{}%", config.disk_warn, config.disk_crit);
    #[cfg(feature = "nvidia")]
    println!("  GPU warn/crit: {}%/{}%", config.gpu_warn, config.gpu_crit);
    println!("  Ollama URL: {}", config.ollama_url);
    if let Some(ref msg) = config.idle_message {
        println!("  Idle message: {}", msg);
    } else {
        println!("  Idle: badge default (battery/charging)");
    }
    println!();

    loop {
        // Refresh system info
        sys.refresh_cpu_usage();
        sys.refresh_memory();

        // Collect alerts
        let alerts = collect_all_alerts(&sys, &nvml, &config).await;

        // Get highest priority alert (lowest number)
        let top_alert = alerts.into_iter().min_by_key(|a| a.priority);

        match top_alert {
            Some(alert) => {
                let message = alert.to_badge_message();

                // Only update if message changed
                if last_alert.as_ref() != Some(&message) {
                    match alert.level {
                        AlertLevel::Critical => println!("[CRIT] {}", message),
                        AlertLevel::Warning => println!("[WARN] {}", message),
                        AlertLevel::Info => println!("[INFO] {}", message),
                        AlertLevel::Normal => println!("[IDLE] {}", message),
                    }

                    if let Err(e) = send_to_badge(&usb, &message) {
                        eprintln!("Failed to update badge: {}", e);
                    }
                    last_alert = Some(message);
                    showing_idle = false;
                }
            }
            None => {
                // No alerts - either show custom idle message or let badge show its default
                if let Some(ref idle_msg) = config.idle_message {
                    if last_alert.as_ref() != Some(idle_msg) {
                        println!("[IDLE] {}", idle_msg);
                        if let Err(e) = send_to_badge(&usb, idle_msg) {
                            eprintln!("Failed to update badge: {}", e);
                        }
                        last_alert = Some(idle_msg.clone());
                    }
                } else if !showing_idle {
                    // No custom idle message - let badge show its built-in battery/charging display
                    // Send empty message or just don't update
                    println!("[IDLE] (badge default - battery/charging)");
                    showing_idle = true;
                    last_alert = None;
                }
            }
        }

        sleep(Duration::from_secs(config.interval_secs)).await;
    }
}

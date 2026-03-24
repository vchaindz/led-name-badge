/// Initialization module - sets up udev rules for non-root USB access

use anyhow::{Context, Result, anyhow};
use std::fs;
use std::process::Command;

/// udev rules content
const UDEV_RULES: &str = r#"# LED Name Badge - allow non-root access
# Drop this file into /etc/udev/rules.d/ and replug your device

SUBSYSTEM=="usb", ATTRS{idVendor}=="0416", ATTRS{idProduct}=="5020", MODE="0666"
KERNEL=="hidraw*", ATTRS{idVendor}=="0416", ATTRS{idProduct}=="5020", MODE="0666"
"#;

/// Path for udev rules
const UDEV_RULES_PATH: &str = "/etc/udev/rules.d/99-led-badge.rules";

/// Check if udev rules are already installed
pub fn is_initialized() -> bool {
    fs::metadata(UDEV_RULES_PATH).is_ok()
}

/// Install udev rules (requires root)
pub fn install_udev_rules() -> Result<()> {
    // Check if running as root
    if !is_root() {
        return Err(anyhow!(
            "This command requires root privileges.\nRun: sudo led-badge init"
        ));
    }

    // Write udev rules file
    fs::write(UDEV_RULES_PATH, UDEV_RULES)
        .context("Failed to write udev rules file")?;

    println!("Installed udev rules to {}", UDEV_RULES_PATH);

    // Reload udev rules
    let status = Command::new("udevadm")
        .args(["control", "--reload-rules"])
        .status()
        .context("Failed to reload udev rules")?;

    if !status.success() {
        return Err(anyhow!("udevadm control --reload-rules failed"));
    }

    println!("Reloaded udev rules");

    // Trigger udev
    let status = Command::new("udevadm")
        .arg("trigger")
        .status()
        .context("Failed to trigger udev")?;

    if !status.success() {
        eprintln!("Warning: udevadm trigger failed");
    }

    println!();
    println!("Setup complete! Please unplug and replug your LED badge.");
    println!("After that, you can use 'led-badge' without sudo.");

    Ok(())
}

/// Check if running as root
fn is_root() -> bool {
    unsafe { libc::geteuid() == 0 }
}

/// Print init status and instructions
pub fn print_status() {
    if is_initialized() {
        println!("udev rules are installed at {}", UDEV_RULES_PATH);
        println!("If you're still having permission issues, try:");
        println!("  1. Unplug and replug your LED badge");
        println!("  2. Run: sudo udevadm control --reload-rules && sudo udevadm trigger");
    } else {
        println!("udev rules are NOT installed.");
        println!("Run 'sudo led-badge init' to set up non-root USB access.");
    }
}

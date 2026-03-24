use anyhow::Result;
use clap::{Parser, Subcommand};

mod font;
mod init;
mod monitor;
mod protocol;
mod renderer;
mod usb;

use monitor::MonitorConfig;
use protocol::{DisplayMode, MessageConfig, ProtocolHeader};
use renderer::render_text;
use usb::UsbConnection;

#[derive(Parser)]
#[command(name = "led-badge")]
#[command(about = "LED name badge programming tool", long_about = None)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Message text to display (use :icon: for built-in icons)
    #[arg(value_name = "MESSAGE")]
    messages: Vec<String>,

    /// Display mode (0=scroll-left, 1=scroll-right, 2=scroll-up, 3=scroll-down,
    /// 4=still-centered, 5=animation, 6=drop-down, 7=curtain, 8=laser)
    #[arg(short, long, default_value = "0")]
    mode: u8,

    /// Scroll speed (1-8)
    #[arg(short, long, default_value = "4")]
    speed: u8,

    /// Brightness (25, 50, 75, or 100)
    #[arg(short = 'B', long, default_value = "100")]
    brightness: u8,

    /// Enable blinking
    #[arg(short, long)]
    blink: bool,

    /// Enable animated border
    #[arg(short, long)]
    ants: bool,

    /// USB backend to use (auto, hidapi, libusb)
    #[arg(short = 'M', long, default_value = "auto")]
    method: String,

    /// Device ID (use 'list' to see available devices)
    #[arg(short = 'D', long)]
    device: Option<String>,
}

#[derive(Subcommand)]
enum Commands {
    /// Set up udev rules for non-root USB access (requires sudo)
    Init,

    /// List available built-in icons
    Icons,

    /// List connected LED badges
    Devices,

    /// Run system monitor daemon (displays alerts on badge)
    Monitor {
        /// Check interval in seconds
        #[arg(long, default_value = "5")]
        interval: u64,

        /// CPU warning threshold (%)
        #[arg(long, default_value = "80")]
        cpu_warn: u8,

        /// CPU critical threshold (%)
        #[arg(long, default_value = "95")]
        cpu_crit: u8,

        /// Memory warning threshold (%)
        #[arg(long, default_value = "80")]
        mem_warn: u8,

        /// Memory critical threshold (%)
        #[arg(long, default_value = "95")]
        mem_crit: u8,

        /// Disk warning threshold (%)
        #[arg(long, default_value = "80")]
        disk_warn: u8,

        /// Disk critical threshold (%)
        #[arg(long, default_value = "95")]
        disk_crit: u8,

        /// GPU warning threshold (%)
        #[arg(long, default_value = "80")]
        gpu_warn: u8,

        /// GPU critical threshold (%)
        #[arg(long, default_value = "95")]
        gpu_crit: u8,

        /// Ollama API URL
        #[arg(long, default_value = "http://localhost:11434")]
        ollama_url: String,

        /// Message to display when idle (default: hostname)
        #[arg(long)]
        idle_message: Option<String>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Init) => {
            init::install_udev_rules()?;
        }
        Some(Commands::Icons) => {
            println!("Available icons (use :name: in messages):");
            for name in font::list_icons() {
                println!("  :{}: ", name);
            }
            println!("\nExample: led-badge \"I :heart: you\"");
        }
        Some(Commands::Devices) => {
            let conn = match cli.method.as_str() {
                "auto" => UsbConnection::new()?,
                other => UsbConnection::with_backend(other)?,
            };

            println!("Using backend: {}", conn.backend_name());
            let devices = conn.find_devices()?;

            if devices.is_empty() {
                println!("No LED badges found.");
                println!("\nMake sure your badge is connected via USB.");
                if !init::is_initialized() {
                    println!("If you're having permission issues, run: sudo led-badge init");
                }
            } else {
                println!("Found {} device(s):", devices.len());
                for device in devices {
                    println!("  {}", device);
                }
            }
        }
        Some(Commands::Monitor {
            interval,
            cpu_warn,
            cpu_crit,
            mem_warn,
            mem_crit,
            disk_warn,
            disk_crit,
            gpu_warn,
            gpu_crit,
            ollama_url,
            idle_message,
        }) => {
            let config = MonitorConfig {
                interval_secs: interval,
                cpu_warn,
                cpu_crit,
                mem_warn,
                mem_crit,
                disk_warn,
                disk_crit,
                gpu_warn,
                gpu_crit,
                ollama_url,
                idle_message,
            };

            let conn = match cli.method.as_str() {
                "auto" => UsbConnection::new()?,
                other => UsbConnection::with_backend(other)?,
            };

            monitor::run_monitor(config, conn).await?;
        }
        None => {
            if cli.messages.is_empty() {
                eprintln!("Error: No message provided.");
                eprintln!("Usage: led-badge \"your message here\"");
                eprintln!("       led-badge init  (to set up udev rules)");
                eprintln!("       led-badge --help (for more options)");
                std::process::exit(1);
            }

            // Render messages
            let mut bitmaps = Vec::new();
            for msg in &cli.messages {
                let bitmap = render_text(msg);
                bitmaps.push(bitmap);
            }

            // Build protocol header
            let mut header_builder = ProtocolHeader::new().brightness(cli.brightness);

            for bitmap in &bitmaps {
                let config = MessageConfig {
                    speed: cli.speed,
                    mode: DisplayMode::from(cli.mode),
                    blink: cli.blink,
                    animated_border: cli.ants,
                };
                header_builder = header_builder.add_message(config, bitmap.width_columns);
            }

            let header = header_builder.build();

            // Combine header and bitmap data
            let mut payload = header.to_vec();
            for bitmap in &bitmaps {
                payload.extend_from_slice(&bitmap.data);
            }


            // Connect and write
            let conn = match cli.method.as_str() {
                "auto" => UsbConnection::new()?,
                other => UsbConnection::with_backend(other)?,
            };

            let device_id = cli.device.as_deref();

            println!("Writing to LED badge via {}...", conn.backend_name());
            conn.write(device_id, &payload)?;
            println!("Done!");
        }
    }

    Ok(())
}

# led-badge

A Rust CLI tool for programming LED name badges (USB VID `0x0416`, PID `0x5020`) — the cheap, widely available ones you find on Amazon and AliExpress.

Write messages, display built-in icons, and even use your badge as a **system monitor** showing CPU, memory, disk, GPU, and Ollama status.

## Why this exists

The [NVIDIA DGX Spark](https://www.nvidia.com/en-us/products/workstations/dgx-spark/) is a silent powerhouse — no fan noise, no blinking LEDs, no visual indication that it's even doing anything. When you're running LLM inference or heavy GPU workloads, you have zero physical feedback about what the system is up to.

This project was born out of that exact problem. I picked up a [BerryBase LED name tag](https://www.berrybase.de/) (11x44 pixels) as a cheap external indicator, and wrote this Rust CLI to drive it. It runs as a systemd service and continuously monitors:

- **Linux system health** — CPU, memory, and disk usage with configurable warning/critical thresholds
- **NVIDIA GPU load** — reads `nvidia-smi` metrics via NVML to show GPU utilization on the badge
- **Ollama status** — polls the Ollama API to display which model is currently loaded and running

The result: a tiny scrolling LED display sitting next to a silent DGX Spark that tells you at a glance whether your system is idle, running inference, or about to run out of memory.

Beyond monitoring, `led-badge` also works as a general-purpose badge programming tool — send any text or icons to the display from the command line.

## Demo

<video src="assets/demo1.mp4" autoplay loop muted playsinline></video>

<video src="assets/demo2.mp4" autoplay loop muted playsinline></video>

## Features

- Write text messages with multiple display effects
- 36+ built-in icons (hardware, status, arrows, symbols)
- Up to 8 messages with individual display settings
- System monitoring daemon with configurable alert thresholds
- NVIDIA GPU monitoring (optional)
- Ollama LLM status display
- HID API and libusb backends
- Linux udev setup for non-root access

## Installation

### From source

```bash
# Standard build
cargo build --release

# With NVIDIA GPU monitoring
cargo build --release --features nvidia

# Without hidapi (libusb-only fallback)
cargo build --release --no-default-features
```

The binary will be at `target/release/led-badge`.

### Linux setup (udev rules)

Required for non-root USB access:

```bash
sudo led-badge init
```

This installs udev rules to `/etc/udev/rules.d/99-led-badge.rules`. Unplug and replug your badge after running.

## Usage

### Basic messages

```bash
# Simple message
led-badge "Hello World"

# Multiple messages (cycles between them)
led-badge "Message 1" "Message 2" "Message 3"

# With inline icons
led-badge "I :heart: Rust"
led-badge ":cpu: Server OK :check:"

# Literal colon (use ::)
led-badge "Time:: 12::00"
```

### Display options

```bash
# Display modes (0-8)
led-badge -m 0 "Scroll left"       # default
led-badge -m 1 "Scroll right"
led-badge -m 2 "Scroll up"
led-badge -m 3 "Scroll down"
led-badge -m 4 "Static centered"
led-badge -m 5 "Animation"
led-badge -m 6 "Drop down"
led-badge -m 7 "Curtain"
led-badge -m 8 "Laser"

# Speed (1-8, default: 4)
led-badge -s 1 "Slow scroll"
led-badge -s 8 "Fast scroll"

# Brightness (25, 50, 75, 100)
led-badge -B 50 "Half brightness"

# Effects
led-badge -b "Blinking message"
led-badge -a "Animated border"
led-badge -b -a "Both effects"
```

### USB backend selection

```bash
led-badge -M auto "Auto-detect"     # default
led-badge -M hidapi "Use HID API"
led-badge -M libusb "Use libusb"
```

### Subcommands

```bash
# List connected badges
led-badge devices

# List all available icons
led-badge icons

# Setup udev rules (Linux, requires root)
sudo led-badge init

# Run system monitor daemon
led-badge monitor
```

## System Monitor

Run `led-badge monitor` to turn your badge into a system status display. It polls system metrics and shows alerts with priority ordering.

```bash
# Default settings
led-badge monitor

# Custom thresholds and interval
led-badge monitor \
  --interval 10 \
  --cpu-warn 70 --cpu-crit 90 \
  --mem-warn 75 --mem-crit 90 \
  --disk-warn 85 --disk-crit 95

# Custom idle message
led-badge monitor --idle-message "MY SERVER"

# With Ollama monitoring
led-badge monitor --ollama-url http://localhost:11434
```

### Monitor options

| Flag | Default | Description |
|------|---------|-------------|
| `--interval` | 5 | Check interval in seconds |
| `--cpu-warn` | 80 | CPU warning threshold % |
| `--cpu-crit` | 95 | CPU critical threshold % |
| `--mem-warn` | 80 | Memory warning threshold % |
| `--mem-crit` | 95 | Memory critical threshold % |
| `--disk-warn` | 80 | Disk warning threshold % |
| `--disk-crit` | 95 | Disk critical threshold % |
| `--gpu-warn` | 80 | GPU warning threshold % |
| `--gpu-crit` | 95 | GPU critical threshold % |
| `--ollama-url` | `http://localhost:11434` | Ollama API URL |
| `--idle-message` | hostname | Message when no alerts |

### Alert priority (highest first)

1. Disk critical
2. Memory critical
3. GPU critical
4. CPU critical
5. Ollama model loaded (info)
6. Memory warning
7. GPU warning
8. CPU warning
9. Disk warning

### Systemd service

A systemd service file is included at `assets/led-badge-monitor.service`:

```bash
sudo cp assets/led-badge-monitor.service /etc/systemd/system/
sudo systemctl enable --now led-badge-monitor
```

## Built-in Icons

Use icons in messages with `:name:` syntax.

| Category | Icons |
|----------|-------|
| **Hardware** | `cpu`, `memory` / `ram`, `disk` / `hdd` / `ssd`, `gpu` |
| **Status** | `check` / `ok`, `cross` / `x` / `error`, `warn` / `warning`, `info` |
| **Arrows** | `left`, `right`, `up`, `down` |
| **Symbols** | `heart` / `HEART` (filled), `heart2` / `HEART2` (filled), `star`, `lightning` / `bolt`, `music` / `note`, `sun`, `moon`, `coffee`, `thumbsup` / `like` |
| **Communication** | `mail` / `email`, `phone`, `wifi` |
| **Power** | `on` / `power_on` / `power`, `off` / `power_off` |
| **Misc** | `happy`, `happy2`, `ball`, `fablab`, `bicycle`, `bicycle_r`, `owncloud` |

Run `led-badge icons` for the full list.

## Supported Characters

```
A-Z a-z 0-9 ^ !"$%&/()=?` °\}][{@ ~ |<>,;.:-_#'+* ä ö ü Ä Ö Ü ß
```

## Protocol

The badge uses a 64-byte header with magic bytes `wang`, followed by bitmap data sent in 64-byte chunks with 100ms delays. Each character is rendered as an 11-pixel tall bitmap. See `src/protocol.rs` for full details.

## License

[MIT](LICENSE)

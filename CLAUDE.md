# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build Commands

```bash
# Build (release)
cargo build --release

# Build with NVIDIA GPU monitoring support
cargo build --release --features nvidia

# Build without hidapi (libusb-only)
cargo build --release --no-default-features

# Run tests
cargo test

# Run the tool
cargo run -- "Your message here"
cargo run -- --help
```

## Architecture

This is a Rust CLI tool for programming LED name badges (USB VID 0x0416, PID 0x5020).

### Core Modules

- **main.rs**: CLI entry point using clap. Subcommands: `init` (udev setup), `icons` (list icons), `devices` (list badges), `monitor` (system monitoring daemon).

- **protocol.rs**: Badge communication protocol. 64-byte header with magic bytes "wang", followed by bitmap data. Header encodes brightness, display modes (scroll, animation, etc.), blink/border flags, and message lengths.

- **renderer.rs**: Text-to-bitmap conversion. Characters are 11 bytes tall (11 rows, 1 byte/row). Supports `:iconname:` syntax for inline icons. Use `::` for literal colon.

- **font.rs**: Embedded 11-pixel font bitmap data. Contains CHARMAP string mapping characters to font indices. Built-in icons include hardware symbols (cpu, memory, disk, gpu), status indicators (check, cross, warn), and decorative symbols.

- **usb.rs**: USB backends abstraction. HidapiBackend (default, feature-gated) and RusbBackend (libusb fallback). Data sent in 64-byte chunks with 100ms delays between writes.

- **monitor.rs**: Async system monitoring daemon. Polls CPU, memory, disk, GPU (nvidia feature), and Ollama API. Displays alerts with priority ordering on the badge.

- **init.rs**: Linux udev rules installer for non-root USB access.

### Key Patterns

- Features: `hidapi` (default), `nvidia` (optional GPU monitoring)
- USB writes require padding to 64-byte boundaries
- Protocol supports up to 8 messages with individual display settings
- Icons are variable-width (1-3 byte columns) unlike fixed-width text characters

/// Protocol header for LED name badge
///
/// The badge uses a 64-byte header followed by bitmap data.
/// Header format:
/// - Bytes 0-3: Magic "wang" (0x77, 0x61, 0x6e, 0x67)
/// - Byte 5: Brightness (0x00=100%, 0x10=75%, 0x20=50%, 0x40=25%)
/// - Byte 6: Blink flags (bit per message)
/// - Byte 7: Animated border flags (bit per message)
/// - Bytes 8-15: Speed/Mode for each message (high nibble=speed-1, low nibble=mode)
/// - Bytes 16-31: Message lengths (2 bytes each, big-endian)
/// - Bytes 38-43: Date/time (not visible on device)

use chrono::{DateTime, Local};

/// Display modes
#[derive(Debug, Clone, Copy, Default)]
pub enum DisplayMode {
    #[default]
    ScrollLeft = 0,
    ScrollRight = 1,
    ScrollUp = 2,
    ScrollDown = 3,
    StillCentered = 4,
    Animation = 5,
    DropDown = 6,
    Curtain = 7,
    Laser = 8,
}

impl From<u8> for DisplayMode {
    fn from(value: u8) -> Self {
        match value {
            0 => DisplayMode::ScrollLeft,
            1 => DisplayMode::ScrollRight,
            2 => DisplayMode::ScrollUp,
            3 => DisplayMode::ScrollDown,
            4 => DisplayMode::StillCentered,
            5 => DisplayMode::Animation,
            6 => DisplayMode::DropDown,
            7 => DisplayMode::Curtain,
            8 => DisplayMode::Laser,
            _ => DisplayMode::ScrollLeft,
        }
    }
}

/// Message configuration
#[derive(Debug, Clone)]
pub struct MessageConfig {
    pub speed: u8,      // 1-8
    pub mode: DisplayMode,
    pub blink: bool,
    pub animated_border: bool,
}

impl Default for MessageConfig {
    fn default() -> Self {
        Self {
            speed: 4,
            mode: DisplayMode::ScrollLeft,
            blink: false,
            animated_border: false,
        }
    }
}

/// Protocol header builder
pub struct ProtocolHeader {
    brightness: u8,  // 25, 50, 75, 100
    messages: Vec<(MessageConfig, usize)>,  // (config, length in byte-columns)
}

impl ProtocolHeader {
    pub fn new() -> Self {
        Self {
            brightness: 100,
            messages: Vec::new(),
        }
    }

    pub fn brightness(mut self, brightness: u8) -> Self {
        self.brightness = brightness.clamp(25, 100);
        self
    }

    pub fn add_message(mut self, config: MessageConfig, length: usize) -> Self {
        if self.messages.len() < 8 {
            self.messages.push((config, length));
        }
        self
    }

    /// Build the 64-byte header
    pub fn build(&self) -> [u8; 64] {
        let mut header = [0u8; 64];

        // Magic bytes "wang"
        header[0] = 0x77;
        header[1] = 0x61;
        header[2] = 0x6e;
        header[3] = 0x67;

        // Brightness
        header[5] = match self.brightness {
            0..=25 => 0x40,
            26..=50 => 0x20,
            51..=75 => 0x10,
            _ => 0x00,
        };

        // Blink and animated border flags
        let mut blink_flags = 0u8;
        let mut ants_flags = 0u8;

        for (i, (config, _)) in self.messages.iter().enumerate() {
            if config.blink {
                blink_flags |= 1 << i;
            }
            if config.animated_border {
                ants_flags |= 1 << i;
            }

            // Speed and mode: high nibble = speed-1, low nibble = mode
            let speed = (config.speed.clamp(1, 8) - 1) as u8;
            let mode = config.mode as u8;
            header[8 + i] = (speed << 4) | mode;
        }

        header[6] = blink_flags;
        header[7] = ants_flags;

        // Fill remaining speed/mode slots with defaults
        for i in self.messages.len()..8 {
            header[8 + i] = 0x30; // speed 4, mode 0
        }

        // Message lengths (2 bytes each, big-endian)
        for (i, (_, length)) in self.messages.iter().enumerate() {
            header[16 + i * 2] = (*length >> 8) as u8;
            header[17 + i * 2] = *length as u8;
        }

        // Date/time (optional, not visible on device)
        let now: DateTime<Local> = Local::now();
        header[38] = (now.format("%y").to_string().parse::<u8>().unwrap_or(0)) as u8;
        header[39] = now.format("%m").to_string().parse::<u8>().unwrap_or(1);
        header[40] = now.format("%d").to_string().parse::<u8>().unwrap_or(1);
        header[41] = now.format("%H").to_string().parse::<u8>().unwrap_or(0);
        header[42] = now.format("%M").to_string().parse::<u8>().unwrap_or(0);
        header[43] = now.format("%S").to_string().parse::<u8>().unwrap_or(0);

        header
    }
}

impl Default for ProtocolHeader {
    fn default() -> Self {
        Self::new()
    }
}

/// Maximum payload size (header + bitmap data)
pub const MAX_PAYLOAD_SIZE: usize = 8192;

/// Header size
pub const HEADER_SIZE: usize = 64;

/// Chunk size for USB transmission
pub const CHUNK_SIZE: usize = 64;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_header_magic() {
        let header = ProtocolHeader::new()
            .add_message(MessageConfig::default(), 5)
            .build();

        assert_eq!(&header[0..4], &[0x77, 0x61, 0x6e, 0x67]);
    }

    #[test]
    fn test_brightness() {
        let header = ProtocolHeader::new()
            .brightness(25)
            .add_message(MessageConfig::default(), 5)
            .build();

        assert_eq!(header[5], 0x40);
    }
}

/// USB communication with LED name badge

use anyhow::{anyhow, Context, Result};
use std::thread;
use std::time::Duration;

use crate::protocol::{CHUNK_SIZE, MAX_PAYLOAD_SIZE};

/// USB Vendor ID for the LED badge
pub const VENDOR_ID: u16 = 0x0416;

/// USB Product ID for the LED badge
pub const PRODUCT_ID: u16 = 0x5020;

/// Backend trait
pub trait Backend {
    fn name(&self) -> &'static str;
    fn find_devices(&self) -> Result<Vec<String>>;
    fn write(&self, device_id: Option<&str>, data: &[u8]) -> Result<()>;
}

/// hidapi backend
#[cfg(feature = "hidapi")]
pub struct HidapiBackend;

#[cfg(feature = "hidapi")]
impl Backend for HidapiBackend {
    fn name(&self) -> &'static str {
        "hidapi"
    }

    fn find_devices(&self) -> Result<Vec<String>> {
        let api = hidapi::HidApi::new().context("Failed to initialize HID API")?;
        let mut devices = Vec::new();

        for device in api.device_list() {
            if device.vendor_id() == VENDOR_ID && device.product_id() == PRODUCT_ID {
                if let Ok(path) = device.path().to_str() {
                    devices.push(path.to_string());
                }
            }
        }

        Ok(devices)
    }

    fn write(&self, _device_id: Option<&str>, data: &[u8]) -> Result<()> {
        let api = hidapi::HidApi::new().context("Failed to initialize HID API")?;
        let device = api.open(VENDOR_ID, PRODUCT_ID).context(
            "Failed to open LED badge. Try running with sudo or set up udev rules with 'led-badge init'",
        )?;

        // Send data in 64-byte chunks with report ID prepended
        for chunk in data.chunks(CHUNK_SIZE) {
            thread::sleep(Duration::from_millis(100));

            // Prepend report ID (0x00) before each 64-byte chunk
            let mut buf = vec![0u8; CHUNK_SIZE + 1];
            buf[0] = 0x00; // Report ID
            buf[1..1 + chunk.len()].copy_from_slice(chunk);

            device.write(&buf).context("Failed to write to HID device")?;
        }

        Ok(())
    }
}

/// libusb backend using rusb
pub struct RusbBackend;

impl Backend for RusbBackend {
    fn name(&self) -> &'static str {
        "libusb"
    }

    fn find_devices(&self) -> Result<Vec<String>> {
        let mut devices = Vec::new();

        for device in rusb::devices()?.iter() {
            let desc = device.device_descriptor()?;
            if desc.vendor_id() == VENDOR_ID && desc.product_id() == PRODUCT_ID {
                let id = format!("{}:{}", device.bus_number(), device.address());
                devices.push(id);
            }
        }

        Ok(devices)
    }

    fn write(&self, device_id: Option<&str>, data: &[u8]) -> Result<()> {
        let device = if let Some(id) = device_id {
            let parts: Vec<&str> = id.split(':').collect();
            if parts.len() != 2 {
                return Err(anyhow!("Invalid device ID format. Expected bus:address"));
            }
            let bus: u8 = parts[0].parse().context("Invalid bus number")?;
            let addr: u8 = parts[1].parse().context("Invalid address")?;

            rusb::devices()?
                .iter()
                .find(|d| {
                    d.bus_number() == bus
                        && d.address() == addr
                        && d.device_descriptor()
                            .map(|desc| desc.vendor_id() == VENDOR_ID && desc.product_id() == PRODUCT_ID)
                            .unwrap_or(false)
                })
                .ok_or_else(|| anyhow!("Device not found: {}", id))?
        } else {
            rusb::devices()?
                .iter()
                .find(|d| {
                    d.device_descriptor()
                        .map(|desc| desc.vendor_id() == VENDOR_ID && desc.product_id() == PRODUCT_ID)
                        .unwrap_or(false)
                })
                .ok_or_else(|| anyhow!("No LED badge found. Is it plugged in?"))?
        };

        let mut handle = device.open().context(
            "Failed to open USB device. Try 'led-badge init' or run with sudo",
        )?;

        #[cfg(target_os = "linux")]
        {
            for i in 0..4 {
                if handle.kernel_driver_active(i).unwrap_or(false) {
                    handle.detach_kernel_driver(i).ok();
                }
            }
        }

        let config = device.active_config_descriptor()?;
        let interface = config.interfaces().next().ok_or_else(|| anyhow!("No USB interface"))?;
        let interface_desc = interface.descriptors().next().ok_or_else(|| anyhow!("No interface descriptor"))?;
        let endpoint = interface_desc
            .endpoint_descriptors()
            .find(|ep| ep.direction() == rusb::Direction::Out)
            .ok_or_else(|| anyhow!("No OUT endpoint found"))?;

        handle.set_active_configuration(1).ok();
        handle.claim_interface(0).ok();

        for chunk in data.chunks(CHUNK_SIZE) {
            thread::sleep(Duration::from_millis(100));

            let mut buf = [0u8; CHUNK_SIZE];
            buf[..chunk.len()].copy_from_slice(chunk);

            handle
                .write_interrupt(endpoint.address(), &buf, Duration::from_secs(5))
                .context("USB write failed")?;
        }

        Ok(())
    }
}

/// USB connection with selectable backend
pub enum UsbConnection {
    #[cfg(feature = "hidapi")]
    Hidapi(HidapiBackend),
    Libusb(RusbBackend),
}

impl UsbConnection {
    /// Create with hidapi backend (preferred)
    #[cfg(feature = "hidapi")]
    pub fn new() -> Result<Self> {
        Ok(Self::Hidapi(HidapiBackend))
    }

    /// Create with libusb backend (fallback)
    #[cfg(not(feature = "hidapi"))]
    pub fn new() -> Result<Self> {
        Ok(Self::Libusb(RusbBackend))
    }

    /// Create with specific backend
    pub fn with_backend(backend: &str) -> Result<Self> {
        match backend {
            #[cfg(feature = "hidapi")]
            "auto" | "hidapi" => Ok(Self::Hidapi(HidapiBackend)),
            #[cfg(not(feature = "hidapi"))]
            "auto" => Ok(Self::Libusb(RusbBackend)),
            "libusb" | "rusb" => Ok(Self::Libusb(RusbBackend)),
            #[cfg(not(feature = "hidapi"))]
            "hidapi" => Err(anyhow!("hidapi not compiled. Rebuild with: cargo build --features hidapi")),
            _ => Err(anyhow!("Unknown backend: {}", backend)),
        }
    }

    pub fn backend_name(&self) -> &'static str {
        match self {
            #[cfg(feature = "hidapi")]
            Self::Hidapi(b) => b.name(),
            Self::Libusb(b) => b.name(),
        }
    }

    pub fn find_devices(&self) -> Result<Vec<String>> {
        match self {
            #[cfg(feature = "hidapi")]
            Self::Hidapi(b) => b.find_devices(),
            Self::Libusb(b) => b.find_devices(),
        }
    }

    pub fn write(&self, device_id: Option<&str>, data: &[u8]) -> Result<()> {
        if data.len() > MAX_PAYLOAD_SIZE {
            return Err(anyhow!("Data too large: {} bytes (max {})", data.len(), MAX_PAYLOAD_SIZE));
        }

        let mut padded = data.to_vec();
        let padding = (CHUNK_SIZE - (padded.len() % CHUNK_SIZE)) % CHUNK_SIZE;
        padded.extend(vec![0u8; padding]);

        match self {
            #[cfg(feature = "hidapi")]
            Self::Hidapi(b) => b.write(device_id, &padded),
            Self::Libusb(b) => b.write(device_id, &padded),
        }
    }
}

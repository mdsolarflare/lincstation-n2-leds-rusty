use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use i2cdev::linux::LinuxI2CDevice;
use i2cdev::core::I2CDevice;

/// LED color enumeration with standard palette
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum LedColor {
    Off,
    White,
    Blue,
    Green,
    Yellow,
    Red,
}

impl Default for LedColor {
    fn default() -> Self {
        LedColor::Off
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LedState {
    pub timestamp: String,
    #[serde(rename = "device")]
    pub device_slot: String,
    pub sys_name: String,
    pub color: LedColor,
    pub last_error: Option<String>,
}

/// LED Bar modes for the chassis status LED
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LedBarMode {
    Solid = 0,
    Breath = 1,
    Loop = 2,
}

/// LED Bar control structure
#[derive(Debug, Clone)]
pub struct LedBar {
    pub mode: LedBarMode,
    pub brightness: u8,  // 0-255
    pub color: LedColor,
}

/// Current state of the LED controller
#[derive(Debug, Clone)]
pub struct LedControllerState {
    pub bus_number: i32,
    pub found: bool,
    pub registers: HashMap<u8, u8>,
}

// LED Controller Register Map
//
// LED Bar (Chassis/Status LED):
//   0x90: Mode (0=solid, 1=breath, 2=loop)
//   0x91: Brightness (0-255)
//   0x92-0x94: RGB for breath/solid (Red, Green, Blue)
//   0x95-0x97: Loop color 1 RGB
//   0x98-0x9A: Loop color 2 RGB
//
// Switch Button LED:
//   0xA0: On register (bit flags)
//   0xB0: Off register (bit flags)
//   0x50: Blinking control
//
// LED Strips (HDD/Network/NVME):
//   0xA0/0xB0: On/Off with bit masks for each strip
//   0x52, 0x54, 0x56, 0x58, 0x5A, 0x5C, 0x5E: Blink controls

/// Mapping of LED slots to their control bits
/// Updated to use network LED position for OS drive (7 drives total)
pub const LED_STRIP_CONTROLS: &[(&str, u8, u8)] = &[
    // (name, on_bit, blink_reg) - uses 0xA0/0xB0 except NVME which use 0xA1/0xB1
    ("OS",       0x40, 0x56),  // Network LED position → OS drive
    ("HDD0",     0x04, 0x52),
    ("HDD1",     0x10, 0x54),
    ("NVME1",    0x01, 0x58),  // On 0xA1/0xB1
    ("NVME2",    0x04, 0x5A),  // On 0xA1/0xB1
    ("NVME3",    0x10, 0x5C),  // On 0xA1/0xB1
    ("NVME4",    0x40, 0x5E),  // On 0xA1/0xB1
];

/// Convert LedColor to RGB values
pub fn color_to_rgb(color: LedColor) -> (u8, u8, u8) {
    match color {
        LedColor::Off => (0, 0, 0),
        LedColor::White => (225, 225, 225),
        LedColor::Blue => (0, 0, 255),
        LedColor::Green => (0, 255, 0),
        LedColor::Yellow => (255, 200, 0),
        LedColor::Red => (255, 0, 0),
    }
}

/// Find the I2C bus that has the LED controller at address 0x26
pub fn find_i2c_bus() -> i32 {
    if let Ok(entries) = fs::read_dir("/sys/class/i2c-dev") {
        let mut buses: Vec<u32> = Vec::new();
        for entry in entries.flatten() {
            if let Ok(file_name) = entry.file_name().into_string() {
                if let Some(bus_str) = file_name.strip_prefix("i2c-") {
                    if let Ok(bus_num) = bus_str.parse::<u32>() {
                        buses.push(bus_num);
                    }
                }
            }
        }
        
        buses.sort();
        for bus_num in buses {
            let dev_path = format!("/dev/i2c-{}", bus_num);
            if let Ok(mut device) = LinuxI2CDevice::new(&dev_path, 0x26) {
                // Try to read a register to verify the device exists
                if device.smbus_read_byte_data(0x50).is_ok() {
                    return bus_num as i32;
                }
            }
        }
    }
    -1
}

/// Get human-readable name for the I2C bus
pub fn get_i2c_bus_name(bus_num: i32) -> String {
    let name_path = format!("/sys/class/i2c-dev/i2c-{}/device/name", bus_num);
    if let Ok(content) = fs::read_to_string(name_path) {
        content.trim().to_string()
    } else {
        format!("I2C Bus {}", bus_num)
    }
}

/// Read registers from the LED controller via SMBus
pub fn read_led_controller_state() -> LedControllerState {
    let bus = find_i2c_bus();
    if bus < 0 {
        return LedControllerState { bus_number: -1, found: false, registers: HashMap::new() };
    }

    let mut registers = HashMap::new();
    let dev_path = format!("/dev/i2c-{}", bus);
    
    if let Ok(mut device) = LinuxI2CDevice::new(&dev_path, 0x26) {
        let reg_addrs = vec![0x50, 0x90, 0x91, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9A, 0xA0, 0xB0];
        
        for &addr in &reg_addrs {
            if let Ok(val) = device.smbus_read_byte_data(addr) {
                registers.insert(addr, val);
            }
        }
    }

    LedControllerState { bus_number: bus, found: !registers.is_empty(), registers }
}

/// Set LED bar mode (solid, breath, loop)
pub fn set_led_bar_mode(bus: i32, mode: LedBarMode) -> Result<(), String> {
    let dev_path = format!("/dev/i2c-{}", bus);
    let mut device = LinuxI2CDevice::new(&dev_path, 0x26)
        .map_err(|e| format!("Failed to open I2C device: {}", e))?;
    
    device.smbus_write_byte_data(0x90, mode as u8)
        .map_err(|e| format!("Failed to set LED bar mode: {}", e))
}

/// Set LED bar brightness (0-255)
pub fn set_led_bar_brightness(bus: i32, brightness: u8) -> Result<(), String> {
    let dev_path = format!("/dev/i2c-{}", bus);
    let mut device = LinuxI2CDevice::new(&dev_path, 0x26)
        .map_err(|e| format!("Failed to open I2C device: {}", e))?;
    
    device.smbus_write_byte_data(0x91, brightness)
        .map_err(|e| format!("Failed to set LED bar brightness: {}", e))
}

/// Set LED bar RGB color
pub fn set_led_bar_color(bus: i32, red: u8, green: u8, blue: u8) -> Result<(), String> {
    let dev_path = format!("/dev/i2c-{}", bus);
    let mut device = LinuxI2CDevice::new(&dev_path, 0x26)
        .map_err(|e| format!("Failed to open I2C device: {}", e))?;
    
    device.smbus_write_byte_data(0x92, red)
        .map_err(|e| format!("Failed to set red channel: {}", e))?;
    
    device.smbus_write_byte_data(0x93, green)
        .map_err(|e| format!("Failed to set green channel: {}", e))?;
    
    device.smbus_write_byte_data(0x94, blue)
        .map_err(|e| format!("Failed to set blue channel: {}", e))
}

/// Apply full LED bar configuration
pub fn apply_led_bar(bus: i32, bar: &LedBar) -> Result<(), String> {
    set_led_bar_mode(bus, bar.mode)?;
    set_led_bar_brightness(bus, bar.brightness)?;
    let (red, green, blue) = color_to_rgb(bar.color);
    set_led_bar_color(bus, red, green, blue)?;
    Ok(())
}

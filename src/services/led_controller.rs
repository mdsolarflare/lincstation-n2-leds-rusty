use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::process::Command;

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
            let output = Command::new("/usr/sbin/i2cget")
                .arg("-y")
                .arg(bus_num.to_string())
                .arg("0x26")
                .arg("0x50")
                .arg("b")
                .output();
            
            if let Ok(output) = output {
                if output.status.success() {
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
/// Falls back to i2cget command for reliability
pub fn read_led_controller_state() -> LedControllerState {
    let bus = find_i2c_bus();
    if bus < 0 {
        return LedControllerState { bus_number: -1, found: false, registers: HashMap::new() };
    }

    let mut registers = HashMap::new();
    let reg_addrs = vec![0x50, 0x90, 0x91, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9A, 0xA0, 0xB0];
    
    for &addr in &reg_addrs {
        if let Ok(output) = Command::new("/usr/sbin/i2cget")
            .arg("-y")
            .arg(bus.to_string())
            .arg("0x26")
            .arg(format!("0x{:02x}", addr))
            .arg("b")
            .output()
        {
            if output.status.success() {
                let trimmed = String::from_utf8_lossy(&output.stdout).trim().to_string();
                let hex_str = if trimmed.starts_with("0x") || trimmed.starts_with("0X") {
                    &trimmed[2..]
                } else {
                    &trimmed
                };
                if let Ok(val) = u8::from_str_radix(hex_str, 16) {
                    registers.insert(addr, val);
                }
            }
        }
    }

    LedControllerState { bus_number: bus, found: !registers.is_empty(), registers }
}

/// Set LED bar mode (solid, breath, loop)
pub fn set_led_bar_mode(bus: i32, mode: LedBarMode) -> Result<(), String> {
    Command::new("/usr/sbin/i2cset")
        .arg("-y")
        .arg(bus.to_string())
        .arg("0x26")
        .arg("0x90")
        .arg(format!("0x{:02x}", mode as u8))
        .arg("b")
        .status()
        .map_err(|e| e.to_string())
        .and_then(|status| {
            if status.success() { Ok(()) } else { Err("i2cset failed".into()) }
        })
}

/// Set LED bar brightness (0-255)
pub fn set_led_bar_brightness(bus: i32, brightness: u8) -> Result<(), String> {
    Command::new("/usr/sbin/i2cset")
        .arg("-y")
        .arg(bus.to_string())
        .arg("0x26")
        .arg("0x91")
        .arg(format!("0x{:02x}", brightness))
        .arg("b")
        .status()
        .map_err(|e| e.to_string())
        .and_then(|status| {
            if status.success() { Ok(()) } else { Err("i2cset failed".into()) }
        })
}

/// Set LED bar RGB color
pub fn set_led_bar_color(bus: i32, red: u8, green: u8, blue: u8) -> Result<(), String> {
    Command::new("/usr/sbin/i2cset")
        .arg("-y")
        .arg(bus.to_string())
        .arg("0x26")
        .arg("0x92")
        .arg(format!("0x{:02x}", red))
        .arg("b")
        .status()
        .map_err(|e| e.to_string())
        .and_then(|status| {
            if !status.success() { return Err("i2cset 0x92 failed".into()); }
            
            Command::new("/usr/sbin/i2cset")
                .arg("-y")
                .arg(bus.to_string())
                .arg("0x26")
                .arg("0x93")
                .arg(format!("0x{:02x}", green))
                .arg("b")
                .status()
                .map_err(|e| e.to_string())
                .and_then(|status| {
                    if !status.success() { return Err("i2cset 0x93 failed".into()); }
                    
                    Command::new("/usr/sbin/i2cset")
                        .arg("-y")
                        .arg(bus.to_string())
                        .arg("0x26")
                        .arg("0x94")
                        .arg(format!("0x{:02x}", blue))
                        .arg("b")
                        .status()
                        .map_err(|e| e.to_string())
                        .and_then(|status| {
                            if status.success() { Ok(()) } else { Err("i2cset 0x94 failed".into()) }
                        })
                })
        })
}

/// Apply full LED bar configuration
pub fn apply_led_bar(bus: i32, bar: &LedBar) -> Result<(), String> {
    set_led_bar_mode(bus, bar.mode)?;
    set_led_bar_brightness(bus, bar.brightness)?;
    let (red, green, blue) = color_to_rgb(bar.color);
    set_led_bar_color(bus, red, green, blue)?;
    Ok(())
}

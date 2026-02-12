use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use i2cdev::linux::LinuxI2CDevice;
use i2cdev::core::I2CDevice;
use std::fs;

// ============================================================================
// LED BAR HARDWARE MODEL
// ============================================================================

/// LED bar display mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LedBarMode {
    Solid = 0,
    Breath = 1,
    Loop = 2,
}

/// LED color palette - use for bar and strip LEDs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LedColor {
    Black,
    White,
    Red,
    Blue,
    Green,
    Yellow,
    Cyan,
    Magenta,
    Orange,
    Purple,
}

impl Default for LedColor {
    fn default() -> Self {
        LedColor::Black
    }
}

/// Convert LedColor to RGB triplet (0-255 each)
pub fn color_to_rgb(color: LedColor) -> (u8, u8, u8) {
    match color {
        LedColor::Black => (0, 0, 0),
        LedColor::White => (225, 225, 225),
        LedColor::Red => (255, 0, 0),
        LedColor::Blue => (0, 0, 255),
        LedColor::Green => (0, 255, 0),
        LedColor::Yellow => (255, 200, 0),
        LedColor::Cyan => (0, 255, 255),
        LedColor::Magenta => (255, 0, 255),
        LedColor::Orange => (255, 165, 0),
        LedColor::Purple => (128, 0, 128),
    }
}

/// The LED bar (chassis/status light) state
/// Includes mode, brightness, and RGB color
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedBar {
    pub mode: LedBarMode,
    pub brightness: u8, // 0-255
    pub color: LedColor,
}

impl Default for LedBar {
    fn default() -> Self {
        Self {
            mode: LedBarMode::Loop,
            brightness: 255,
            color: LedColor::Red,
        }
    }
}

// ============================================================================
// LED STRIP HARDWARE MODEL
// ============================================================================

/// Individual LED strip state (disk/drive indicators)
/// Each LED has red and white channels, plus white blinking control
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct LedStrip {
    pub white_on: bool,
    pub red_on: bool,
    pub white_blinking: bool,
}

impl Default for LedStrip {
    fn default() -> Self {
        Self {
            white_on: false,
            red_on: false,
            white_blinking: false,
        }
    }
}

impl LedStrip {
    /// Get the effective color of this LED based on channel state
    pub fn get_color(&self) -> &'static str {
        match (self.white_on, self.red_on) {
            (false, false) => "off",
            (true, false) => "white",
            (false, true) => "red",
            (true, true) => "orange", // red + white
        }
    }

    /// Get current state as human-readable string
    pub fn describe(&self) -> String {
        let color = self.get_color();
        let blink = if self.white_blinking && self.white_on {
            " (blinking)"
        } else {
            ""
        };
        format!("{}{}", color, blink)
    }
}

// ============================================================================
// LED CONTROLLER STATE
// ============================================================================

/// Names of the 8 LED strips
pub const LED_STRIP_NAMES: &[&str] = &["OS", "HDD0", "HDD1", "NVME1", "NVME2", "NVME3", "NVME4"];

/// Complete LED controller state - both bar and strips
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedControllerState {
    pub bar: LedBar,
    pub strips: HashMap<String, LedStrip>,
}

impl Default for LedControllerState {
    fn default() -> Self {
        let mut strips = HashMap::new();
        for name in LED_STRIP_NAMES {
            strips.insert(name.to_string(), LedStrip::default());
        }
        Self {
            bar: LedBar::default(),
            strips,
        }
    }
}

impl LedControllerState {
    /// Create a new controller state with all LEDs off
    pub fn new() -> Self {
        Self::default()
    }

    /// Get a mutable reference to a strip by name
    pub fn get_strip_mut(&mut self, name: &str) -> Option<&mut LedStrip> {
        self.strips.get_mut(name)
    }

    /// Get a reference to a strip by name
    pub fn get_strip(&self, name: &str) -> Option<&LedStrip> {
        self.strips.get(name)
    }
}

// ============================================================================
// COMMANDS
// ============================================================================

/// Commands that can be executed on the LED controller
/// This enum represents the complete command library
#[derive(Debug, Clone)]
#[allow(dead_code)]  // Some variants planned for future use
pub enum LedCommand {
    // LED Bar commands
    SetBarMode(LedBarMode),
    SetBarBrightness(u8),
    SetBarColor(LedColor),
    ApplyBar(LedBar),

    // Individual LED strip commands
    SetStripColor(String, bool, bool), // name, white_on, red_on
    SetStripWhiteBlinking(String, bool), // name, enabled
    ApplyStrip(String, LedStrip),

    // Batch operations
    AllStripsOff,
    AllLEDsOff,
}

impl LedCommand {
    /// Human-readable description of the command
    pub fn describe(&self) -> String {
        match self {
            Self::SetBarMode(mode) => format!("Set bar mode to {:?}", mode),
            Self::SetBarBrightness(b) => format!("Set bar brightness to {}", b),
            Self::SetBarColor(c) => format!("Set bar color to {:?}", c),
            Self::ApplyBar(bar) => format!("Apply bar: {:?} @ {}", bar.mode, bar.brightness),
            Self::SetStripColor(name, w, r) => {
                format!("Set {} to white={} red={}", name, w, r)
            }
            Self::SetStripWhiteBlinking(name, enabled) => {
                format!("Set {} white blinking to {}", name, enabled)
            }
            Self::ApplyStrip(name, strip) => {
                format!("Apply {}: {}", name, strip.describe())
            }
            Self::AllStripsOff => "Turn all strips off".to_string(),
            Self::AllLEDsOff => "Turn all LEDs off (bar and strips)".to_string(),
        }
    }
}

// ============================================================================
// TEST/DEBUG COMMANDS
// ============================================================================

/// Create sequence of commands for: all lights off
pub fn test_all_lights_off() -> Vec<LedCommand> {
    let mut cmds = vec![
        LedCommand::SetBarMode(LedBarMode::Solid),
        LedCommand::SetBarBrightness(0),
        LedCommand::SetBarColor(LedColor::Black),
    ];
    
    for name in LED_STRIP_NAMES {
        cmds.push(LedCommand::SetStripColor(name.to_string(), false, false));
        cmds.push(LedCommand::SetStripWhiteBlinking(name.to_string(), false));
    }
    
    cmds
}

/// Create sequence of commands for: all lights white with blinking
pub fn test_all_lights_white() -> Vec<LedCommand> {
    let mut cmds = vec![
        LedCommand::SetBarMode(LedBarMode::Solid),
        LedCommand::SetBarBrightness(255),
        LedCommand::SetBarColor(LedColor::White),
    ];
    
    for name in LED_STRIP_NAMES {
        cmds.push(LedCommand::SetStripColor(name.to_string(), true, false));
        cmds.push(LedCommand::SetStripWhiteBlinking(name.to_string(), true));
    }
    
    cmds
}

/// Create sequence of commands for: all lights red
pub fn test_all_lights_red() -> Vec<LedCommand> {
    let mut cmds = vec![
        LedCommand::SetBarMode(LedBarMode::Solid),
        LedCommand::SetBarBrightness(255),
        LedCommand::SetBarColor(LedColor::Red),
    ];
    
    for name in LED_STRIP_NAMES {
        cmds.push(LedCommand::SetStripColor(name.to_string(), false, true));
        cmds.push(LedCommand::SetStripWhiteBlinking(name.to_string(), false));
    }
    
    cmds
}

// ============================================================================
// HARDWARE COMMUNICATION
// ============================================================================

/// I2C address of the LED controller device
const LED_CONTROLLER_ADDR: u16 = 0x26;

/// Find the I2C bus that has the LED controller at address 0x26
pub fn find_i2c_bus() -> Option<i32> {
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
            if let Ok(mut device) = LinuxI2CDevice::new(&dev_path, LED_CONTROLLER_ADDR) {
                // Try to read a register to verify the device exists
                if device.smbus_read_byte_data(0x50).is_ok() {
                    return Some(bus_num as i32);
                }
            }
        }
    }
    None
}

/// Get human-readable name for an I2C bus
pub fn get_i2c_bus_name(bus_num: i32) -> String {
    let name_path = format!("/sys/class/i2c-dev/i2c-{}/device/name", bus_num);
    match fs::read_to_string(name_path) {
        Ok(content) => content.trim().to_string(),
        Err(_) => format!("I2C Bus {}", bus_num),
    }
}

/// LED bar register readings
#[derive(Debug, Clone)]
pub struct LedBarRegisters {
    pub mode: u8,                      // 0x90
    pub brightness: u8,                // 0x91
    pub color_red: u8,                 // 0x92 - used by Solid and Breath modes
    pub color_green: u8,               // 0x93 - used by Solid and Breath modes
    pub color_blue: u8,                // 0x94 - used by Solid and Breath modes
    pub loop_a_red: u8,                // 0x95 - Loop mode color A
    pub loop_a_green: u8,              // 0x96 - Loop mode color A
    pub loop_a_blue: u8,               // 0x97 - Loop mode color A
    pub loop_b_red: u8,                // 0x98 - Loop mode color B
    pub loop_b_green: u8,              // 0x99 - Loop mode color B
    pub loop_b_blue: u8,               // 0x9A - Loop mode color B
}

/// Read all LED bar registers (0x90-0x9A)
pub fn read_led_bar_registers(bus: i32) -> Result<LedBarRegisters, String> {
    let dev_path = format!("/dev/i2c-{}", bus);
    let mut device = LinuxI2CDevice::new(&dev_path, LED_CONTROLLER_ADDR)
        .map_err(|e| format!("Failed to open I2C device: {}", e))?;

    Ok(LedBarRegisters {
        mode: device.smbus_read_byte_data(0x90)
            .map_err(|e| format!("Failed to read mode (0x90): {}", e))?,
        brightness: device.smbus_read_byte_data(0x91)
            .map_err(|e| format!("Failed to read brightness (0x91): {}", e))?,
        color_red: device.smbus_read_byte_data(0x92)
            .map_err(|e| format!("Failed to read color red (0x92): {}", e))?,
        color_green: device.smbus_read_byte_data(0x93)
            .map_err(|e| format!("Failed to read color green (0x93): {}", e))?,
        color_blue: device.smbus_read_byte_data(0x94)
            .map_err(|e| format!("Failed to read color blue (0x94): {}", e))?,
        loop_a_red: device.smbus_read_byte_data(0x95)
            .map_err(|e| format!("Failed to read loop A red (0x95): {}", e))?,
        loop_a_green: device.smbus_read_byte_data(0x96)
            .map_err(|e| format!("Failed to read loop A green (0x96): {}", e))?,
        loop_a_blue: device.smbus_read_byte_data(0x97)
            .map_err(|e| format!("Failed to read loop A blue (0x97): {}", e))?,
        loop_b_red: device.smbus_read_byte_data(0x98)
            .map_err(|e| format!("Failed to read loop B red (0x98): {}", e))?,
        loop_b_green: device.smbus_read_byte_data(0x99)
            .map_err(|e| format!("Failed to read loop B green (0x99): {}", e))?,
        loop_b_blue: device.smbus_read_byte_data(0x9A)
            .map_err(|e| format!("Failed to read loop B blue (0x9A): {}", e))?,
    })
}

// ============================================================================
// EXECUTION LAYER
// ============================================================================

/// Execute a command on the LED controller
///
/// This function handles all command execution. It updates the state
/// and applies changes to the hardware via I2C.
pub fn execute_command(bus: i32, state: &mut LedControllerState, cmd: LedCommand) -> Result<(), String> {
    match cmd {
        // LED Bar commands
        LedCommand::SetBarMode(mode) => {
            state.bar.mode = mode;
            _write_bar_mode(bus, mode)?;
        }
        LedCommand::SetBarBrightness(brightness) => {
            state.bar.brightness = brightness;
            _write_bar_brightness(bus, brightness)?;
        }
        LedCommand::SetBarColor(color) => {
            state.bar.color = color;
            _write_bar_color(bus, color)?;
        }
        LedCommand::ApplyBar(bar) => {
            state.bar = bar.clone();
            _write_bar_mode(bus, bar.mode)?;
            _write_bar_brightness(bus, bar.brightness)?;
            _write_bar_color(bus, bar.color)?;
        }

        // LED Strip commands
        LedCommand::SetStripColor(name, white, red) => {
            if let Some(strip) = state.get_strip_mut(&name) {
                strip.white_on = white;
                strip.red_on = red;
            } else {
                return Err(format!("Unknown strip: {}", name));
            }
            _write_strip_color(bus, &name, white, red)?;
        }
        LedCommand::SetStripWhiteBlinking(name, enabled) => {
            if let Some(strip) = state.get_strip_mut(&name) {
                strip.white_blinking = enabled;
            } else {
                return Err(format!("Unknown strip: {}", name));
            }
            _write_strip_blinking(bus, &name, enabled)?;
        }
        LedCommand::ApplyStrip(name, strip) => {
            if state.strips.contains_key(&name) {
                state.strips.insert(name.clone(), strip);
                _write_strip_color(bus, &name, strip.white_on, strip.red_on)?;
                _write_strip_blinking(bus, &name, strip.white_blinking)?;
            } else {
                return Err(format!("Unknown strip: {}", name));
            }
        }

        // Batch operations
        LedCommand::AllStripsOff => {
            for name in LED_STRIP_NAMES {
                execute_command(bus, state, LedCommand::SetStripColor(name.to_string(), false, false))?;
            }
        }
        LedCommand::AllLEDsOff => {
            state.bar = LedBar::default();
            for name in LED_STRIP_NAMES {
                state.strips.insert(name.to_string(), LedStrip::default());
            }
            _write_bar_mode(bus, LedBarMode::Solid)?;
            _write_bar_brightness(bus, 0)?;
            _write_bar_color(bus, LedColor::Black)?;
            for name in LED_STRIP_NAMES {
                _write_strip_color(bus, name, false, false)?;
                _write_strip_blinking(bus, name, false)?;
            }
        }
    }
    Ok(())
}

// ============================================================================
// LED STRIP REGISTER MAP
// ============================================================================

/// Mapping of strip names to their control registers and bit positions
/// Format: (name, register_offset_for_controls, white_bit, red_bit, blink_register)
struct StripRegisterMap {
    name: &'static str,
    white_bit: u8,
    red_bit: u8,
    blink_register: u8,
}

const STRIP_REGISTERS: &[StripRegisterMap] = &[
    StripRegisterMap { name: "POWER",  white_bit: 0x01, red_bit: 0x02, blink_register: 0x50 },
    StripRegisterMap { name: "MGMT",   white_bit: 0x40, red_bit: 0x80, blink_register: 0x56 },
    StripRegisterMap { name: "SSD1",   white_bit: 0x04, red_bit: 0x08, blink_register: 0x52 },
    StripRegisterMap { name: "SSD2",   white_bit: 0x10, red_bit: 0x20, blink_register: 0x54 },
    StripRegisterMap { name: "NVME1",  white_bit: 0x01, red_bit: 0x02, blink_register: 0x58 },
    StripRegisterMap { name: "NVME2",  white_bit: 0x04, red_bit: 0x08, blink_register: 0x5A },
    StripRegisterMap { name: "NVME3",  white_bit: 0x10, red_bit: 0x20, blink_register: 0x5C },
    StripRegisterMap { name: "NVME4",  white_bit: 0x40, red_bit: 0x80, blink_register: 0x5E },
];

/// Get the register map entry for a strip name
fn _get_strip_register_map(name: &str) -> Option<&'static StripRegisterMap> {
    STRIP_REGISTERS.iter().find(|r| r.name == name)
}

/// Determine if this strip uses 0xA0/0xB0 (standard) or 0xA1/0xB1 (NVME)
fn _is_nvme_strip(name: &str) -> bool {
    name.starts_with("NVME")
}

// ============================================================================
// HARDWARE REGISTER WRITES (Private Implementation)
// ============================================================================

/// Write LED bar mode register (0x90)
fn _write_bar_mode(bus: i32, mode: LedBarMode) -> Result<(), String> {
    let dev_path = format!("/dev/i2c-{}", bus);
    let mut device = LinuxI2CDevice::new(&dev_path, LED_CONTROLLER_ADDR)
        .map_err(|e| format!("Failed to open I2C device: {}", e))?;

    device
        .smbus_write_byte_data(0x90, mode as u8)
        .map_err(|e| format!("Failed to set LED bar mode: {}", e))
}

/// Write LED bar brightness register (0x91)
fn _write_bar_brightness(bus: i32, brightness: u8) -> Result<(), String> {
    let dev_path = format!("/dev/i2c-{}", bus);
    let mut device = LinuxI2CDevice::new(&dev_path, LED_CONTROLLER_ADDR)
        .map_err(|e| format!("Failed to open I2C device: {}", e))?;

    device
        .smbus_write_byte_data(0x91, brightness)
        .map_err(|e| format!("Failed to set LED bar brightness: {}", e))
}

/// Write LED bar color registers (0x92-0x94 for all three color copies)
fn _write_bar_color(bus: i32, color: LedColor) -> Result<(), String> {
    let dev_path = format!("/dev/i2c-{}", bus);
    let mut device = LinuxI2CDevice::new(&dev_path, LED_CONTROLLER_ADDR)
        .map_err(|e| format!("Failed to open I2C device: {}", e))?;

    let (red, green, blue) = color_to_rgb(color);

    // Write to all three color register sets (solid, breath 1, breath 2)
    device
        .smbus_write_byte_data(0x92, red)
        .map_err(|e| format!("Failed to set red: {}", e))?;
    device
        .smbus_write_byte_data(0x93, green)
        .map_err(|e| format!("Failed to set green: {}", e))?;
    device
        .smbus_write_byte_data(0x94, blue)
        .map_err(|e| format!("Failed to set blue: {}", e))?;

    // Also set loop colors to same values
    device
        .smbus_write_byte_data(0x95, red)
        .map_err(|e| format!("Failed to set loop color 1 red: {}", e))?;
    device
        .smbus_write_byte_data(0x96, green)
        .map_err(|e| format!("Failed to set loop color 1 green: {}", e))?;
    device
        .smbus_write_byte_data(0x97, blue)
        .map_err(|e| format!("Failed to set loop color 1 blue: {}", e))?;

    device
        .smbus_write_byte_data(0x98, red)
        .map_err(|e| format!("Failed to set loop color 2 red: {}", e))?;
    device
        .smbus_write_byte_data(0x99, green)
        .map_err(|e| format!("Failed to set loop color 2 green: {}", e))?;
    device
        .smbus_write_byte_data(0x9A, blue)
        .map_err(|e| format!("Failed to set loop color 2 blue: {}", e))
}

/// Write LED strip color (white and red on/off)
fn _write_strip_color(bus: i32, name: &str, white_on: bool, red_on: bool) -> Result<(), String> {
    let dev_path = format!("/dev/i2c-{}", bus);
    let mut device = LinuxI2CDevice::new(&dev_path, LED_CONTROLLER_ADDR)
        .map_err(|e| format!("Failed to open I2C device: {}", e))?;

    let reg_map = _get_strip_register_map(name)
        .ok_or_else(|| format!("Unknown LED strip: {}", name))?;

    let is_nvme = _is_nvme_strip(name);
    let on_reg = if is_nvme { 0xA1 } else { 0xA0 };
    let off_reg = if is_nvme { 0xB1 } else { 0xB0 };

    // Set white channel
    if white_on {
        device
            .smbus_write_byte_data(on_reg, reg_map.white_bit)
            .map_err(|e| format!("Failed to turn on white for {}: {}", name, e))?;
    } else {
        device
            .smbus_write_byte_data(off_reg, reg_map.white_bit)
            .map_err(|e| format!("Failed to turn off white for {}: {}", name, e))?;
    }

    // Set red channel
    if red_on {
        device
            .smbus_write_byte_data(on_reg, reg_map.red_bit)
            .map_err(|e| format!("Failed to turn on red for {}: {}", name, e))?;
    } else {
        device
            .smbus_write_byte_data(off_reg, reg_map.red_bit)
            .map_err(|e| format!("Failed to turn off red for {}: {}", name, e))?;
    }

    Ok(())
}

/// Write LED strip blinking control
fn _write_strip_blinking(bus: i32, name: &str, enabled: bool) -> Result<(), String> {
    let dev_path = format!("/dev/i2c-{}", bus);
    let mut device = LinuxI2CDevice::new(&dev_path, LED_CONTROLLER_ADDR)
        .map_err(|e| format!("Failed to open I2C device: {}", e))?;

    let reg_map = _get_strip_register_map(name)
        .ok_or_else(|| format!("Unknown LED strip: {}", name))?;

    let value = if enabled { 0xFF } else { 0x00 };
    device
        .smbus_write_byte_data(reg_map.blink_register, value)
        .map_err(|e| format!("Failed to set blinking for {}: {}", name, e))
}

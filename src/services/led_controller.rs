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
    SoftWhite,
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
        LedColor::White => (255, 255, 255),
        LedColor::SoftWhite => (225, 225, 225),
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
    pub loop_color: LedColor
}

impl Default for LedBar {
    fn default() -> Self {
        Self {
            mode: LedBarMode::Loop,
            brightness: 255,
            color: LedColor::Yellow,
            loop_color: LedColor::Green
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

// ============================================================================
// LED CONTROLLER STATE
// ============================================================================

/// Names of the 8 LED strips
pub const LED_STRIP_NAMES: &[&str] = &["POWER", "MGMT", "SSD1", "SSD2", "NVME1", "NVME2", "NVME3", "NVME4"];
pub const MINIMUM_WRITE_DELAY_MS: i8 = 10;

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
    ApplyBar(LedBar),

    // Individual LED strip command
    SetSingleLed(String, bool, bool, bool),

    // Batch operations
    AllStripsWhite,     // set bar -> solid/255/white and set every strip white ON
    AllStripsRed,       // set bar -> solid/255/red and set every strip red ON
    AllLEDsOff,
}

// ============================================================================
// TEST/DEBUG COMMANDS (moved to `main.rs`)
// ----------------------------------------------------------------------------
// Helper test functions were moved into `src/main.rs` to avoid duplicate
// boilerplate and keep the service module focused on the command model and
// hardware access. See the TEST/DEBUG HELPERS section in `main.rs`.
// ============================================================================

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

/// LED strip register readings
#[derive(Debug, Clone)]
pub struct LedStripRegisters {
    pub strips: Vec<StripState>,
}

#[derive(Debug, Clone)]
pub struct StripState {
    // Identity
    pub name: String,

    // Const map (addresses & masks)
    pub white_on_reg: u8,
    pub white_on_val: u8,
    pub white_off_reg: u8,
    pub white_off_val: u8,
    pub red_on_reg: u8,
    pub red_on_val: u8,
    pub red_off_reg: u8,
    pub red_off_val: u8,
    pub blink_reg: u8,
    pub blink_val: u8
}

/// Read all LED strip registers
pub fn read_led_strip_registers(bus: i32) -> Result<LedStripRegisters, String> {
    let dev_path = format!("/dev/i2c-{}", bus);
    let mut device = LinuxI2CDevice::new(&dev_path, LED_CONTROLLER_ADDR)
        .map_err(|e| format!("Failed to open I2C device: {}", e))?;

    let mut strips = Vec::new();
    for strip_map in LED_STRIP_REGISTERS.iter() {

        let white_on_reg_read = device.smbus_read_byte_data(strip_map.white_on_reg).unwrap_or(0);
        let white_off_reg_read = device.smbus_read_byte_data(strip_map.white_off_reg).unwrap_or(0);
        let red_on_reg_read = device.smbus_read_byte_data(strip_map.red_on_reg).unwrap_or(0);
        let red_off_reg_read = device.smbus_read_byte_data(strip_map.red_off_reg).unwrap_or(0);
        let blink_read = device.smbus_read_byte_data(strip_map.blink_reg).unwrap_or(0);

        strips.push(StripState {
            name: strip_map.name.to_string(),

            // const map
            white_on_reg: strip_map.white_on_reg,
            white_on_val: white_on_reg_read,
            white_off_reg: strip_map.white_off_reg,
            white_off_val: white_off_reg_read,
            red_on_reg: strip_map.red_on_reg,
            red_on_val: red_on_reg_read,
            red_off_reg: strip_map.red_off_reg,
            red_off_val: red_off_reg_read,
            blink_reg: strip_map.blink_reg,
            blink_val: blink_read
        });
    }

    Ok(LedStripRegisters { strips })
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
        LedCommand::ApplyBar(bar) => {
            state.bar = bar.clone();
            _write_led_bar(bus, bar.mode, bar.brightness, bar.color, bar.loop_color)?;
        }
        LedCommand::SetSingleLed(name, white, red, blink) => {
            let strip = LED_STRIP_REGISTERS.iter().find(|r| r.name == name);
            let strip = strip.ok_or_else(|| format!("No strip register for '{}'", name))?;
            _write_led_strip(bus, strip, white, red, blink)?;
        }
        // Batch operations
        LedCommand::AllStripsWhite => {
            // set bar to solid / max brightness / white
            _write_led_bar(bus, LedBarMode::Solid, 255, LedColor::White, LedColor::White)?;
            state.bar.mode = LedBarMode::Solid;
            state.bar.brightness = 255;
            state.bar.color = LedColor::White;
            state.bar.loop_color = LedColor::White;

            // turn WHITE ON for every strip, disable blink and red
            for strip in LED_STRIP_REGISTERS {
                _write_led_strip(bus, strip, true, false, false)?;
            }
        }

        LedCommand::AllStripsRed => {
            // set bar to solid / max brightness / red
            _write_led_bar(bus, LedBarMode::Solid, 255, LedColor::Red, LedColor::Red)?;
            state.bar.mode = LedBarMode::Solid;
            state.bar.brightness = 255;
            state.bar.color = LedColor::Red;
            state.bar.loop_color = LedColor::Red;

            // turn RED ON for every strip, disable blink and white
            for strip in LED_STRIP_REGISTERS {
                _write_led_strip(bus, strip, false, true, false)?;
            }
        }

        LedCommand::AllLEDsOff => {
            for name in LED_STRIP_NAMES {
                state.strips.insert(name.to_string(), LedStrip::default());
            }
            _write_led_bar(bus, LedBarMode::Solid, 0, LedColor::Black, LedColor::Black)?;
            state.bar.mode = LedBarMode::Solid;
            state.bar.brightness = 0;
            state.bar.color = LedColor::Black;
            state.bar.loop_color = LedColor::Black;
            // turn everything off!
            for strip in LED_STRIP_REGISTERS {
                _write_led_strip(bus, strip, false, false, false)?;
            }
        }
    }
    Ok(())
}

// ============================================================================
// LED STRIP REGISTER MAP
// ============================================================================

/// Mapping of strip names to their I2C register and value commands
/// Each strip has separate registers and values for turning white/red on/off
/// and for controlling blinking
struct LedRegisterMap {
    name: &'static str,
    white_on_reg: u8,
    white_on_val: u8,
    white_off_reg: u8,
    white_off_val: u8,
    red_on_reg: u8,
    red_on_val: u8,
    red_off_reg: u8,
    red_off_val: u8,
    blink_reg: u8,
    blink_on_val: u8,
    blink_off_val: u8,
}

const LED_STRIP_REGISTERS: &[LedRegisterMap] = &[
    LedRegisterMap {
        name: "POWER",
        white_on_reg: 0xA0, white_on_val: 0x01,
        white_off_reg: 0xB0, white_off_val: 0x01,
        red_on_reg: 0xA0, red_on_val: 0x02,
        red_off_reg: 0xB0, red_off_val: 0x02,
        blink_reg: 0x50, blink_on_val: 0x01,
        blink_off_val: 0x00,
    },
    LedRegisterMap {
        name: "MGMT",
        white_on_reg: 0xA0, white_on_val: 0x40,
        white_off_reg: 0xB0, white_off_val: 0x40,
        red_on_reg: 0xA0, red_on_val: 0x80,
        red_off_reg: 0xB0, red_off_val: 0x80,
        blink_reg: 0x56, blink_on_val: 0x01,
        blink_off_val: 0x00,
    },
    LedRegisterMap {
        name: "SSD1",
        white_on_reg: 0xA0, white_on_val: 0x04,
        white_off_reg: 0xB0, white_off_val: 0x04,
        red_on_reg: 0xA0, red_on_val: 0x08,
        red_off_reg: 0xB0, red_off_val: 0x08,
        blink_reg: 0x52, blink_on_val: 0x01,
        blink_off_val: 0x00,
    },
    LedRegisterMap {
        name: "SSD2",
        white_on_reg: 0xA0, white_on_val: 0x10,
        white_off_reg: 0xB0, white_off_val: 0x10,
        red_on_reg: 0xA0, red_on_val: 0x20,
        red_off_reg: 0xB0, red_off_val: 0x20,
        blink_reg: 0x54, blink_on_val: 0x01,
        blink_off_val: 0x00,
    },
    LedRegisterMap {
        name: "NVME1",
        white_on_reg: 0xA1, white_on_val: 0x01,
        white_off_reg: 0xB1, white_off_val: 0x01,
        red_on_reg: 0xA1, red_on_val: 0x02,
        red_off_reg: 0xB1, red_off_val: 0x02,
        blink_reg: 0x58, blink_on_val: 0x01,
        blink_off_val: 0x00,
    },
    LedRegisterMap {
        name: "NVME2",
        white_on_reg: 0xA1, white_on_val: 0x04,
        white_off_reg: 0xB1, white_off_val: 0x04,
        red_on_reg: 0xA1, red_on_val: 0x08,
        red_off_reg: 0xB1, red_off_val: 0x08,
        blink_reg: 0x5A, blink_on_val: 0x01,
        blink_off_val: 0x00,
    },
    LedRegisterMap {
        name: "NVME3",
        white_on_reg: 0xA1, white_on_val: 0x10,
        white_off_reg: 0xB1, white_off_val: 0x10,
        red_on_reg: 0xA1, red_on_val: 0x20,
        red_off_reg: 0xB1, red_off_val: 0x20,
        blink_reg: 0x5C, blink_on_val: 0x01,
        blink_off_val: 0x00,
    },
    LedRegisterMap {
        name: "NVME4",
        white_on_reg: 0xA1, white_on_val: 0x40,
        white_off_reg: 0xB1, white_off_val: 0x40,
        red_on_reg: 0xA1, red_on_val: 0x80,
        red_off_reg: 0xB1, red_off_val: 0x80,
        blink_reg: 0x5E, blink_on_val: 0x01,
        blink_off_val: 0x00,
    },
];


// ============================================================================
// HARDWARE REGISTER WRITES (Private Implementation)
// ============================================================================

/// Write a byte to an SMBus register with safe error handling and optional delay
///
/// This helper encapsulates the common pattern of writing to SMBus registers,
/// converting I2C errors into descriptive strings, and adding a small delay
/// between consecutive writes to ensure hardware stability.
///
/// # Arguments
/// * `device` - The I2C device to write to
/// * `register` - The register address (0-255)
/// * `value` - The value to write (0-255)
/// * `error_msg_format` - Format string for error messages with one {} placeholder for the original error
///
/// # Returns
/// * `Ok(())` on success
/// * `Err(String)` with a descriptive error message on failure
fn smbus_write_with_delay<D: I2CDevice>(
    device: &mut D,
    register: u8,
    value: u8
) -> Result<(), String> {
    // Perform the write operation
    device
        .smbus_write_byte_data(register, value)
        .map_err(|e| format!("Error while writing to smbus: {}", e))?;

    // Add a small delay after successful writes to ensure hardware stability
    std::thread::sleep(std::time::Duration::from_millis(MINIMUM_WRITE_DELAY_MS as u64));

    Ok(())
}

/// Write LED bar mode register (0x90)
fn _write_led_bar(bus: i32, mode: LedBarMode, brightness: u8, color: LedColor, loop_color:LedColor) -> Result<(), String> {
    let dev_path = format!("/dev/i2c-{}", bus);
    let mut device = LinuxI2CDevice::new(&dev_path, LED_CONTROLLER_ADDR)
        .map_err(|e| format!("Failed to open I2C device: {}", e))?;

    smbus_write_with_delay(&mut device, 0x90, mode as u8)?;
    smbus_write_with_delay(&mut device, 0x91, brightness)?;

    let (red, green, blue) = color_to_rgb(color);

    // Write to all three color register sets (solid, breath, loop)
    smbus_write_with_delay(&mut device, 0x92, red)?;
    smbus_write_with_delay(&mut device, 0x93, green)?;
    smbus_write_with_delay(&mut device, 0x94, blue)?;

    // Also set loop colors to same values
    smbus_write_with_delay(&mut device, 0x95, red)?;
    smbus_write_with_delay(&mut device, 0x96, green)?;
    smbus_write_with_delay(&mut device, 0x97, blue)?;

    let (red, green, blue) = color_to_rgb(loop_color);

    smbus_write_with_delay(&mut device, 0x98, red)?;
    smbus_write_with_delay(&mut device, 0x99, green)?;
    smbus_write_with_delay(&mut device, 0x9A, blue)?;

    Ok(())
}

fn _write_led_strip(bus: i32, strip: &LedRegisterMap, white_on: bool, red_on: bool, blink_on: bool) -> Result<(), String> {

    let dev_path = format!("/dev/i2c-{}", bus);
    let mut device = LinuxI2CDevice::new(&dev_path, LED_CONTROLLER_ADDR)
        .map_err(|e| format!("Failed to open I2C device: {}", e))?;

    if white_on {
        smbus_write_with_delay(&mut device, strip.white_on_reg, strip.white_on_val)?;
    } else {
        smbus_write_with_delay(&mut device, strip.white_off_reg, strip.white_off_val)?;
    }

    if red_on {
        smbus_write_with_delay(&mut device, strip.red_on_reg, strip.red_on_val)?;
    } else {
        smbus_write_with_delay(&mut device, strip.red_off_reg, strip.red_off_val)?;
    }

    if blink_on {
        smbus_write_with_delay(&mut device, strip.blink_reg, strip.blink_on_val)?;
    } else {
        smbus_write_with_delay(&mut device, strip.blink_reg, strip.blink_off_val)?;
    }

    Ok(())
}

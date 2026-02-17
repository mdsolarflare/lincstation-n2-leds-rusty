// The packages for the data structures
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs;
use chrono::Utc;

mod services;
use services::disk_status::{check_drive_status, detect_all_devices, build_device_report_list, DriveStatus, SLOTS};
use services::led_controller::{
    LedColor, LedControllerState, find_i2c_bus, get_i2c_bus_name, LedCommand, 
    execute_command,
    read_led_bar_registers, read_led_strip_registers, LED_STRIP_NAMES,
};
use crate::services::led_controller::{LedBar, LedBarMode};

// Configuration for your specific hardware
#[allow(dead_code)]
const LOG_PATH: &str = "/var/log/lincstation_leds.json";

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
struct LedLog {
    leds: Vec<String>,  // TODO: Update when daemon loop is implemented
    last_updated: String,
}

#[allow(dead_code)]
fn write_led_log(log: &LedLog, path: &str) -> std::io::Result<()> {
    let temp_path = format!("{}.tmp", path);
    // Pretty print for readability
    let serialized = serde_json::to_string_pretty(log)?;
    fs::write(&temp_path, serialized)?;
    fs::rename(&temp_path, path)?;
    Ok(())
}

#[allow(dead_code)]
fn get_iso_timestamp() -> String {
    Utc::now().to_rfc3339()
}


fn main() {
    // Basic argument parsing for debug modes
    let args: Vec<String> = env::args().collect();
    if args.len() > 1 {
        match args[1].as_str() {
            "check-status" => debug_check_status(),
            "test-all-off" => {
                let bus = parse_bus_arg(&args);
                match run_test_all_off(bus) {
                    Ok(_) => println!("\n✓ test-all-off completed successfully"),
                    Err(e) => {
                        eprintln!("✗ test-all-off failed: {}", e);
                        std::process::exit(1);
                    }
                }
            }
            "test-all-white" => {
                let bus = parse_bus_arg(&args);
                match run_test_all_white(bus) {
                    Ok(_) => println!("\n✓ test-all-white completed successfully"),
                    Err(e) => {
                        eprintln!("✗ test-all-white failed: {}", e);
                        std::process::exit(1);
                    }
                }
            }
            "test-all-red" => {
                let bus = parse_bus_arg(&args);
                match run_test_all_red(bus) {
                    Ok(_) => println!("\n✓ test-all-red completed successfully"),
                    Err(e) => {
                        eprintln!("✗ test-all-red failed: {}", e);
                        std::process::exit(1);
                    }
                }
            }
            "test-led-bar" => {
                let bus = parse_bus_arg(&args);
                match run_test_led_bar(&args, bus) {
                    Ok(_) => println!("\n✓ test-led-bar completed successfully"),
                    Err(e) => {
                        eprintln!("✗ test-led-bar failed: {}", e);
                        std::process::exit(1);
                    }
                }
            }
            _ => {
                eprintln!("Usage: ./lincstation-leds [check-status|test-all-off|test-all-white|test-all-red] [--bus N]");
                std::process::exit(1);
            }
        }
        return; 
    }

    println!("Starting LincStation LED Daemon...");
    println!("Tip: Use 'check-status' to debug, or test-all-off/test-all-white/test-all-red to test LEDs");
    
    // TODO: Implement main daemon loop
    println!("(Daemon loop not yet implemented)");
}


/// Convert DriveStatus to LED color
fn drive_status_to_led_color(status: DriveStatus) -> (LedColor, Option<String>) {
    match status {
        DriveStatus::Healthy => (LedColor::Blue, Some("Drive present".into())),
        DriveStatus::Missing => (LedColor::White, Some("No drive detected".into())),
        DriveStatus::Degraded => (LedColor::Red, Some("Drive degraded".into())),
    }
}


// --- Debug Methods ---

fn debug_check_status() {
    println!("\n╔═══════════════════════════════════════════════════════════════════════╗");
    println!("║          LincStation N2 LED Daemon - Debug Status Report              ║");
    println!("╚═══════════════════════════════════════════════════════════════════════╝\n");
    
    // ========== DISK STATUS SERVICE ==========
    println!("┌─ Disk Status Service ─────────────────────────────────────────────────┐");
    
    // 1. Detect all devices and read their stats
    let disk_stats = match detect_all_devices() {
        Ok(stats) => stats,
        Err(e) => {
            eprintln!("Error detecting devices: {}", e);
            return;
        }
    };

    // 2. Build comprehensive report list (detected + expected)
    let detected_names: Vec<String> = disk_stats.iter().map(|ds| ds.device_name.clone()).collect();
    let report_rows = build_device_report_list(&detected_names);
    
    // 3. Map sys_name -> slot_name for quick lookup
    let mut sys_to_slot = HashMap::new();
    for slot in SLOTS {
        sys_to_slot.insert(slot.sys_name.to_string(), slot.slot_name);
    }

    println!("{:<12} | {:<12} | {:<8} | {:<12} | {}", 
        "DEVICE", "MAPPED SLOT", "COLOR", "UTIL %", "MESSAGE");
    println!("{}", "-".repeat(73)); // This should align with the surrounding column sizes.

    for sys_name in report_rows {
        // Determine Mapping
        let slot_label = match sys_to_slot.get(&sys_name) {
            Some(label) => *label,
            None => "(Not Mapped)",
        };

        // Determine Status
        let status = check_drive_status(&sys_name);
        let (color, msg) = drive_status_to_led_color(status);
        let msg_str = msg.unwrap_or_else(|| "".to_string());

        // Get disk stats if available
        let util_str = disk_stats
            .iter()
            .find(|ds| ds.device_name == sys_name)
            .map(|ds| format!("{:.1}%", ds.utilization_percent))
            .unwrap_or_else(|| "N/A".to_string());

        // Don't show LED color for unmapped drives
        let color_display = if slot_label != "(Not Mapped)" {
            format!("{:?}", color)
        } else {
            "-".to_string()
        };

        println!("{:<12} | {:<12} | {:<8} | {:<12} | {}", 
            sys_name, 
            slot_label, 
            color_display, 
            util_str,
            msg_str
        );
    }
    
    println!("└───────────────────────────────────────────────────────────────────────┘\n");

    // ========== LED CONTROLLER SERVICE ==========
    println!("┌─ LED Controller Service ──────────────────────────────────────────────┐");
    
    let bus = match find_i2c_bus() {
        Some(b) => b,
        None => {
            println!("✗ LED Controller NOT found");
            println!("  Searched /sys/class/i2c-dev for device at address 0x26");
            println!("  Tip: Check I2C connection or use --bus to specify manually");
            println!("└───────────────────────────────────────────────────────────────────────┘\n");
            std::process::exit(0);
        }
    };

    let bus_name = get_i2c_bus_name(bus);
    println!("✓ LED Controller found on I2C bus {}: {}\n", bus, bus_name);
    
    // Read LED Bar registers
    println!("LED Bar Registers (0x90-0x9A):");
    match read_led_bar_registers(bus) {
        Ok(regs) => {
            println!("  Mode (0x90):           0x{:02X} ({})", regs.mode, 
                match regs.mode {
                    0 => "Solid",
                    1 => "Breath",
                    2 => "Loop",
                    _ => "Unknown",
                });
            println!("  Brightness (0x91):          0x{:02X} ({}/255)", regs.brightness, regs.brightness);
            println!("  Color RGB (0x92-94):        R=0x{:02X} G=0x{:02X} B=0x{:02X}", 
                regs.color_red, regs.color_green, regs.color_blue);
            println!("    (used by Solid & Breath modes)");
            println!("  Loop Color A RGB (0x95-97): R=0x{:02X} G=0x{:02X} B=0x{:02X}", 
                regs.loop_a_red, regs.loop_a_green, regs.loop_a_blue);
            println!("  Loop Color B RGB (0x98-9A): R=0x{:02X} G=0x{:02X} B=0x{:02X}", 
                regs.loop_b_red, regs.loop_b_green, regs.loop_b_blue);
        }
        Err(e) => {
            println!("✗ Failed to read LED bar registers: {}", e);
        }
    }
    
    // Read LED Strip registers
    println!("\nLED Strip Registers (8 disk/device LEDs):");
    match read_led_strip_registers(bus) {
        Ok(strip_regs) => {
            println!("  Control register addresses (static):");
            println!("    Standard On/Off: 0xA0 / 0xB0");
            println!("    NVME     On/Off: 0xA1 / 0xB1");
            println!("\n  Strip States:");
            println!("    {:<8} | W_ON   | W_ON_V | W_OFF  | W_OFF_V | R_ON   | R_ON_V | R_OFF  | R_OFF_V | B_REG  | B_VAL", 
                "Name");
            println!("    {}", "-".repeat(110));
            for strip in &strip_regs.strips {
                println!("    {:<8} | 0x{:02X}   | 0x{:02X}   | 0x{:02X}   | 0x{:02X}    | 0x{:02X}   | 0x{:02X}   | 0x{:02X}   | 0x{:02X}    | 0x{:02X}   | 0x{:02X}", 
                    strip.name,
                    strip.white_on_reg, strip.white_on_val,
                    strip.white_off_reg, strip.white_off_val,
                    strip.red_on_reg, strip.red_on_val,
                    strip.red_off_reg, strip.red_off_val,
                    strip.blink_reg, strip.blink_val);
            }
        }
        Err(e) => {
            println!("✗ Failed to read LED strip registers: {}", e);
        }
    }
    
    println!("\nTest Commands:");
    println!("  ./lincstation-leds test-all-off");
    println!("  ./lincstation-leds test-all-white");
    println!("  ./lincstation-leds test-all-red");
    
    println!("└───────────────────────────────────────────────────────────────────────┘\n");

    std::process::exit(0);
}

// ============================================================================
// TEST/DEBUG HELPERS
// ============================================================================

/// Parse test-led-bar arguments and execute the command
///
/// Arguments:
/// --color <name>     Main color (required): red, blue, green, yellow, cyan, magenta, orange, purple, white, black
/// --loopcolor <name> Loop color (optional): defaults to main color
/// --breathing        Use breath mode instead of solid/loop
///
/// Examples:
/// test-led-bar --color red --loopcolor blue     → loop, 255, red, blue
/// test-led-bar --loopcolor yellow --color green → loop, 255, green, yellow
/// test-led-bar --color orange                   → solid, 255, orange, orange
/// test-led-bar --color orange --breathing       → breath, 255, orange, orange
fn run_test_led_bar(args: &[String], bus: i32) -> Result<(), String> {
    let mut color: Option<LedColor> = None;
    let mut loop_color: Option<LedColor> = None;
    let mut is_breathing = false;

    let mut args_iter = args.iter().skip(2).peekable();

    while let Some(arg) = args_iter.next() {
        match arg.as_str() {
            "--color" => {
                if let Some(color_str) = args_iter.peek() {
                    color = Some(parse_color_argument(color_str)?);
                    args_iter.next(); // consume the value
                }
            }
            "--loopcolor" => {
                if let Some(loop_color_str) = args_iter.peek() {
                    loop_color = Some(parse_color_argument(loop_color_str)?);
                    args_iter.next(); // consume the value
                }
            }
            "--breathing" => {
                is_breathing = true;
            }
            _ => {}
        }
    }

    let color = color.ok_or_else(|| "Missing required argument: --color".to_string())?;
    let loop_color = loop_color.unwrap_or(color);

    // Determine mode based on arguments
    let mode = if is_breathing {
        LedBarMode::Breath
    } else if loop_color != color {
        LedBarMode::Loop
    } else {
        LedBarMode::Solid
    };

    let bar = LedBar {
        mode,
        brightness: 255,
        color,
        loop_color,
    };

    println!("\n╔═══════════════════════════════════════════════════════════════╗");
    println!("║                    LED Bar Test Command                        ║");
    println!("╚═══════════════════════════════════════════════════════════════╝\n");

    println!("Applying bar configuration:");
    println!("  Mode:      {:?}", mode);
    println!("  Brightness: {}", bar.brightness);
    println!("  Color:     {:?}", color);
    println!("  LoopColor: {:?}", loop_color);

    let mut state = LedControllerState::new();
    execute_command(bus, &mut state, LedCommand::ApplyBar(bar))?;

    Ok(())
}

/// Parse a color argument string to LedColor enum
fn parse_color_argument(color_str: &str) -> Result<LedColor, String> {
    match color_str.to_lowercase().as_str() {
        "black" | "off" => Ok(LedColor::Black),
        "white" => Ok(LedColor::White),
        "red" => Ok(LedColor::Red),
        "blue" => Ok(LedColor::Blue),
        "green" => Ok(LedColor::Green),
        "yellow" => Ok(LedColor::Yellow),
        "cyan" => Ok(LedColor::Cyan),
        "magenta" => Ok(LedColor::Magenta),
        "orange" => Ok(LedColor::Orange),
        "purple" => Ok(LedColor::Purple),
        "softwhite" => Ok(LedColor::SoftWhite),
        _ => Err(format!("Unknown color '{}'. Valid colors: black, white, red, blue, green, yellow, cyan, magenta, orange, purple, softwhite", color_str))
    }
}

/// Run hardware test: sequentially turn OFF bar + white/red/blink per strip with 1s delay
///
/// This uses the public `execute_command` path so the same command logic
/// and state-updates are exercised during the test.
fn run_test_all_off(bus: i32) -> Result<(), String> {
    let mut state = LedControllerState::new();

    // Use the batch operation so we don't repeat the same write logic here
    execute_command(bus, &mut state, LedCommand::AllLEDsOff)?;
    println!("  All LEDs turned OFF");

    Ok(())
}

/// Run hardware test: set bar -> Solid/255/White and turn WHITE on for every strip
///
/// This mirrors `run_test_all_off` but uses the `AllStripsWhite` batch command so
/// the same execution path (and timing) is exercised as the production code.
fn run_test_all_white(bus: i32) -> Result<(), String> {
    let mut state = LedControllerState::new();

    // Delegate to the batch command which applies bar + per-strip white writes
    execute_command(bus, &mut state, LedCommand::AllStripsWhite)?;
    println!("  Bar set to WHITE and all strips WHITE turned ON");

    Ok(())
}

/// Run hardware test: set bar -> Solid/255/Red and turn RED on for every strip
///
/// Mirrors `run_test_all_white` but exercises the red-channel per-strip writes.
fn run_test_all_red(bus: i32) -> Result<(), String> {
    let mut state = LedControllerState::new();

    execute_command(bus, &mut state, LedCommand::AllStripsRed)?;
    println!("  Bar set to RED and all strips RED turned ON");

    Ok(())
}

/// Parse --bus argument from command line args
fn parse_bus_arg(args: &[String]) -> i32 {
    for (i, arg) in args.iter().enumerate() {
        if arg == "--bus" && i + 1 < args.len() {
            if let Ok(bus_num) = args[i + 1].parse::<i32>() {
                return bus_num;
            }
        }
    }
    
    // Try to find bus dynamically
    match find_i2c_bus() {
        Some(bus) => {
            println!("Auto-detected I2C bus: {}", bus);
            bus
        }
        None => {
            eprintln!("Could not find I2C LED controller. Use --bus N to specify manually.");
            std::process::exit(1);
        }
    }
}

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
    execute_command, run_test_all_off, run_test_all_white, run_test_all_red,
    read_led_bar_registers, read_led_strip_registers, LED_STRIP_NAMES,
};

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

/// Execute a test command sequence
fn test_command(test_name: &str, bus: i32, commands: Vec<LedCommand>) {
    println!("\n╔═══════════════════════════════════════════════════════════════╗");
    println!("║  Test: {}  {}", test_name, "   ".repeat((20 - test_name.len()) / 3));
    println!("║  I2C Bus: {}  {}", bus, " ".repeat(52 - test_name.len()));
    println!("╚═══════════════════════════════════════════════════════════════╝\n");

    let mut state = LedControllerState::new();
    
    for (i, cmd) in commands.iter().enumerate() {
        println!("[{}] {}", i + 1, cmd.describe());
        
        match execute_command(bus, &mut state, cmd.clone()) {
            Ok(_) => println!("     ✓ Success"),
            Err(e) => {
                eprintln!("     ✗ Error: {}", e);
                std::process::exit(1);
            }
        }
    }
    
    println!("\n✓ Test completed successfully!");
    println!("Final LED State:");
    println!("  Bar: mode={:?}, brightness={}, color={:?}", 
        state.bar.mode, state.bar.brightness, state.bar.color);
    
    println!("  Strips:");
    for name in LED_STRIP_NAMES {
        if let Some(strip) = state.get_strip(name) {
            println!("    {}: {}", name, strip.describe());
        }
    }
}
// The packages for the data structures
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs;
use chrono::Utc;

mod services;
use services::disk_status::{check_drive_status, detect_all_devices, build_device_report_list, DriveStatus, SLOTS};
use services::led_controller::{LedColor, LedState, read_led_controller_state, get_i2c_bus_name};

// Configuration for your specific hardware
const LOG_PATH: &str = "/var/log/lincstation_leds.json";

#[derive(Debug, Serialize, Deserialize)]
struct LedLog {
    leds: Vec<LedState>,
    last_updated: String,
}


fn main() {
    // Basic argument parsing for debug modes
    let args: Vec<String> = env::args().collect();
    if args.len() > 1 {
        match args[1].as_str() {
            "check-status" => debug_check_status(),
            _ => {
                eprintln!("Unknown command. Usage: ./lincstation-leds [check-status]");
                std::process::exit(1);
            }
        }
        return; 
    }

    println!("Starting LincStation LED Daemon...");
    
    // Initial loop state
    let mut led_log = LedLog {
        leds: Vec::new(),
        last_updated: get_iso_timestamp(),
    };

    loop {
        let mut new_states = Vec::new();
        for slot in SLOTS {
            let status = check_drive_status(slot.sys_name);
            let (color, msg) = drive_status_to_led_color(status);
            
            let state = LedState {
                timestamp: get_iso_timestamp(),
                device_slot: slot.slot_name.to_string(),
                sys_name: slot.sys_name.to_string(),
                color,
                last_error: msg,
            };
            
            // Optimization: check previous state
            let mut should_update = true;
            if let Some(old) = led_log.leds.iter().find(|l| l.device_slot == slot.slot_name) {
                if old.color == color {
                    should_update = false;
                }
            }

            if should_update {
                // TODO: Implement proper LED control based on drive status
                // For now, placeholder
            }

            new_states.push(state);
        }

        led_log.leds = new_states;
        led_log.last_updated = get_iso_timestamp();
        
        if let Err(e) = write_led_log(&led_log, LOG_PATH) {
            eprintln!("Failed to write log: {}", e);
        }

        std::thread::sleep(std::time::Duration::from_secs(2));
    }
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
    println!("{}", "-".repeat(100));

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
    let controller_state = read_led_controller_state();
    if controller_state.found {
        let bus_name = get_i2c_bus_name(controller_state.bus_number);
        println!("✓ LED Controller found on I2C bus {}: {}\n", controller_state.bus_number, bus_name);
        
        // Display LED Bar state
        println!("LED Bar (Chassis):");
        println!("  Mode: 0x{:02X} (0=solid, 1=breath, 2=loop)", 
            controller_state.registers.get(&0x90).unwrap_or(&0));
        println!("  Brightness: 0x{:02X}", 
            controller_state.registers.get(&0x91).unwrap_or(&0));
        println!("  Color RGB: R=0x{:02X} G=0x{:02X} B=0x{:02X}",
            controller_state.registers.get(&0x92).unwrap_or(&0),
            controller_state.registers.get(&0x93).unwrap_or(&0),
            controller_state.registers.get(&0x94).unwrap_or(&0));
        
        // Display switch LED and strips state
        println!("\nSwitch/Strip Controls:");
        println!("  On bits (0xA0):  0x{:02X}", 
            controller_state.registers.get(&0xA0).unwrap_or(&0));
        println!("  Off bits (0xB0): 0x{:02X}", 
            controller_state.registers.get(&0xB0).unwrap_or(&0));
        println!("  On bits (0xA1):  0x{:02X} (NVME strips)", 
            controller_state.registers.get(&0xA1).unwrap_or(&0));
        println!("  Off bits (0xB1): 0x{:02X} (NVME strips)", 
            controller_state.registers.get(&0xB1).unwrap_or(&0));
        
        // Display raw register values for debugging
        println!("\nRaw Register Values:");
        let mut reg_addresses: Vec<_> = controller_state.registers.keys().collect();
        reg_addresses.sort();
        for addr in reg_addresses {
            let val = controller_state.registers[addr];
            println!("  0x{:02X}: 0x{:02X}", addr, val);
        }
    } else {
        println!("✗ LED Controller NOT found on any I2C bus\n");
        println!("Debug Info:");
        println!("  - Searched /sys/class/i2c-dev for available buses");
        println!("  - Probed address 0x26 on each bus using i2cget");
        println!("  - Ensure i2c-tools is installed: apt install i2c-tools");
    }
    println!("└───────────────────────────────────────────────────────────────────────┘\n");

    std::process::exit(0);
}

// --- Status Logic ---

// Check drive status based on device state and file existence

// --- I2C LED Controller Functions ---

/// Find the I2C bus that has the LED controller at address 0x26
fn write_led_log(log: &LedLog, path: &str) -> std::io::Result<()> {
    let temp_path = format!("{}.tmp", path);
    // Pretty print for readability
    let serialized = serde_json::to_string_pretty(log)?;
    fs::write(&temp_path, serialized)?;
    fs::rename(&temp_path, path)?;
    Ok(())
}

fn get_iso_timestamp() -> String {
    Utc::now().to_rfc3339()
}

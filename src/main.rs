// The packages for the data structures
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;
use chrono::Utc;

// Configuration for your specific hardware
// NOTE: You must update these I2C constants for your specific board!
const LOG_PATH: &str = "/var/log/lincstation_leds.json";
const I2C_BUS: &str = "11";
const I2C_ADDRESS: &str = "0x26";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
enum LedColor {
    Off,
    White,
    Blue,
    Green,
    Yellow,
    Red,
} // ... (impl Default omitted for brevity, will be in full file)

impl Default for LedColor {
    fn default() -> Self {
        LedColor::Off
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct LedState {
    timestamp: String,
    #[serde(rename = "device")]
    device_slot: String,
    sys_name: String,
    color: LedColor,
    last_error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct LedLog {
    leds: Vec<LedState>,
    last_updated: String,
}

struct DriveSlot {
    slot_name: &'static str,
    sys_name: &'static str,
}

// Registry of all physical drive slots on the LincStation N2.
const SLOTS: &[DriveSlot] = &[
    // Position 1: OS Drive
    DriveSlot { slot_name: "OS",    sys_name: "mmcblk0" }, 
    // Position 2-5: NVMe Drives
    DriveSlot { slot_name: "NVME0", sys_name: "nvme0n1" },
    DriveSlot { slot_name: "NVME1", sys_name: "nvme1n1" },
    DriveSlot { slot_name: "NVME2", sys_name: "nvme2n1" },
    DriveSlot { slot_name: "NVME3", sys_name: "nvme3n1" },
    // Position 6-7: SATA Drives
    // User updated these to sdb/sdc
    DriveSlot { slot_name: "SATA0", sys_name: "sdb" },
    DriveSlot { slot_name: "SATA1", sys_name: "sdc" },
];

fn main() {
    // Basic argument parsing for debug modes
    let args: Vec<String> = env::args().collect();
    if args.len() > 1 {
        match args[1].as_str() {
            "check-status" => debug_check_status(),
            "check-leds" => debug_check_leds(),
            _ => {
                eprintln!("Unknown command. Usage: ./lincstation-leds [check-status|check-leds]");
                // Don't exit here, fallback to daemon mode or exit? 
                // "keep it simple" -> print and run daemon? Or exit?
                // Usually exit.
                std::process::exit(1);
            }
        }
        return; // Ensure we stop after debug command
    }

    println!("Starting LincStation LED Daemon...");
    
    // Initial loop state
    let mut led_log = LedLog {
        leds: Vec::new(),
        last_updated: get_iso_timestamp(),
    };

    loop {
        let active_devices = get_active_holders();

        let mut new_states = Vec::new();
        for slot in SLOTS {
            let (color, msg) = check_drive_status(slot, &active_devices);
            
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
                if let Err(e) = set_hardware_led(&state) {
                   eprintln!("Error setting LED for {}: {}", slot.slot_name, e);
                }
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

// --- Debug Methods ---

fn debug_check_status() {
    println!("DEBUG: Checking Drive Status...");
    let active_devices = get_active_holders();
    
    println!("{:<10} | {:<10} | {:<10} | {}", "SLOT", "DEVICE", "COLOR", "MESSAGE");
    println!("{}", "-".repeat(60));

    for slot in SLOTS {
        let (color, msg) = check_drive_status(slot, &active_devices);
        let msg_str = msg.unwrap_or_else(|| "".to_string());
        println!("{:<10} | {:<10} | {:<10?} | {}", 
            slot.slot_name, 
            slot.sys_name, 
            color, 
            msg_str
        );
    }
    std::process::exit(0);
}

fn debug_check_leds() {
    println!("DEBUG: Proposed LED States...");
    let active_devices = get_active_holders();

    println!("{:<10} | {:<10} | {:<8} | {:<8}", "SLOT", "COLOR", "REG", "VAL");
    println!("{}", "-".repeat(50));

    for slot in SLOTS {
        let (color, _) = check_drive_status(slot, &active_devices);
        let (reg, val) = get_i2c_codes(slot.slot_name, color);
        
        println!("{:<10} | {:<10?} | {:<8} | {:<8}", 
            slot.slot_name, 
            color, 
            reg, 
            val
        );
    }
    std::process::exit(0);
}

// --- Status Logic ---

fn check_drive_status(slot: &DriveSlot, active_devs: &HashSet<String>) -> (LedColor, Option<String>) {
    let sys_path = format!("/sys/block/{}", slot.sys_name);
    let path = Path::new(&sys_path);

    if !path.exists() {
        return (LedColor::White, Some("No drive detected".into()));
    }

    let state_path = path.join("device/state");
    if state_path.exists() {
        if let Ok(content) = fs::read_to_string(&state_path) {
            let polished = content.trim();
            if polished == "dead" || polished == "transport-offline" {
                return (LedColor::Red, Some(format!("Device state: {}", polished)));
            }
        }
    }

    if active_devs.contains(slot.sys_name) {
        return (LedColor::Green, Some("Drive is active/mounted".into()));
    }

    (LedColor::Blue, Some("Drive idle/unmounted".into()))
}

fn get_active_holders() -> HashSet<String> {
    let mut active = HashSet::new();
    for slot in SLOTS {
        let sys_path = format!("/sys/block/{}", slot.sys_name);
        if has_entries(&format!("{}/holders", sys_path)) {
            active.insert(slot.sys_name.to_string());
            continue;
        }
        if let Ok(dir) = fs::read_dir(&sys_path) {
            for entry in dir.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with(slot.sys_name) {
                    if has_entries(&format!("{}/{}/holders", sys_path, name)) {
                        active.insert(slot.sys_name.to_string());
                        break;
                    }
                }
            }
        }
    }
    active
}

fn has_entries(path: &str) -> bool {
    if let Ok(mut dir) = fs::read_dir(path) {
        return dir.next().is_some();
    }
    false
}

// --- Hardware Control ---

/// Helper to get the Register and Value for a given slot+color
fn get_i2c_codes(slot_name: &str, color: LedColor) -> (&'static str, &'static str) {
    match (slot_name, color) {
        // OS Drive (Position 1)
        ("OS", LedColor::White) => ("0xA1", "0x01"), 
        ("OS", LedColor::Blue)  => ("0xA1", "0x02"), // Example
        ("OS", LedColor::Green) => ("0xA1", "0x03"), // Example
        ("OS", LedColor::Red)   => ("0xB1", "0x01"), // Example

        // NVME0 (Position 2)
        ("NVME0", _) => ("0x00", "0x00"), // TODO: Fill in

        // NVME1 (Position 3)
        ("NVME1", _) => ("0x00", "0x00"), // TODO: Fill in
        
        // NVME2 (Position 4)
        ("NVME2", _) => ("0x00", "0x00"), // TODO: Fill in

        // NVME3 (Position 5)
        ("NVME3", _) => ("0x00", "0x00"), // TODO: Fill in

        // SATA0 (Position 6)
        ("SATA0", _) => ("0x00", "0x00"), // TODO: Fill in

        // SATA1 (Position 7)
        ("SATA1", _) => ("0x00", "0x00"), // TODO: Fill in
        
        (_, _) => ("0x00", "0x00"), // Skip unmapped
    }
}

fn set_hardware_led(state: &LedState) -> Result<(), String> {
    let (reg, val) = get_i2c_codes(&state.device_slot, state.color);

    if reg == "0x00" {
        return Ok(()); 
    }

    let status = Command::new("i2cset")
        .args(&["-y", I2C_BUS, I2C_ADDRESS, reg, val])
        .status()
        .map_err(|e| e.to_string())?;

    if !status.success() {
        return Err(format!("i2cset exited with code {:?}", status.code()));
    }
    Ok(())
}

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

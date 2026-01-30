// The packages for the data structures
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

// Configuration for your specific hardware
// NOTE: You must update these I2C constants for your specific board!
const LOG_PATH: &str = "/var/log/lincstation_leds.json";
const I2C_BUS: &str = "11";
const I2C_ADDRESS: &str = "0x26";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
enum LedColor {
    Off,
    White,  // No drive present
    Blue,   // Present but unused/unmounted
    Green,  // Healthy and In-Use (Mounted/ZFS member)
    Yellow, // Warning (Visible but verification failed - Unused currently)
    Red,    // Failure/Unresponsive
}

impl Default for LedColor {
    fn default() -> Self {
        LedColor::Off
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct LedState {
    timestamp: u64,
    #[serde(rename = "device")]
    device_slot: String, // e.g., "NVME0"
    sys_name: String,    // e.g., "nvme0n1"
    color: LedColor,
    last_error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct LedLog {
    leds: Vec<LedState>,
    last_updated: u64,
}

struct DriveSlot {
    slot_name: &'static str,
    sys_name: &'static str,
}

// Define the physical slots on the LincStation N2
const SLOTS: &[DriveSlot] = &[
    DriveSlot { slot_name: "NVME0", sys_name: "nvme0n1" },
    DriveSlot { slot_name: "NVME1", sys_name: "nvme1n1" },
    DriveSlot { slot_name: "NVME2", sys_name: "nvme2n1" },
    DriveSlot { slot_name: "NVME3", sys_name: "nvme3n1" },
    DriveSlot { slot_name: "EMMC",  sys_name: "mmcblk0" },
    DriveSlot { slot_name: "SATA0", sys_name: "sda" },
    DriveSlot { slot_name: "SATA1", sys_name: "sdb" },
];

fn main() {
    println!("Starting LincStation LED Daemon...");
    
    // Initial loop state
    let mut led_log = LedLog {
        leds: Vec::new(),
        last_updated: 0,
    };

    loop {
        // 1. Refresh System State (Mounts/Holders)
        let active_devices = get_active_holders();

        // 2. Update States for each slot
        let mut new_states = Vec::new();
        for slot in SLOTS {
            let (color, msg) = check_drive_status(slot, &active_devices);
            
            let state = LedState {
                timestamp: current_time(),
                device_slot: slot.slot_name.to_string(),
                sys_name: slot.sys_name.to_string(),
                color,
                last_error: msg,
            };
            
            // 3. Update Hardware LED (Only if state changed to save I2C bus traffic)
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

        // 4. Log Update
        led_log.leds = new_states;
        led_log.last_updated = current_time();
        
        if let Err(e) = write_led_log(&led_log, LOG_PATH) {
            eprintln!("Failed to write log: {}", e);
        }

        std::thread::sleep(std::time::Duration::from_secs(2));
    }
}

// --- Status Logic ---

fn check_drive_status(slot: &DriveSlot, active_devs: &HashSet<String>) -> (LedColor, Option<String>) {
    let sys_path = format!("/sys/block/{}", slot.sys_name);
    let path = Path::new(&sys_path);

    // 1. Check Presence (White)
    if !path.exists() {
        return (LedColor::White, Some("No drive detected".into()));
    }

    // 2. Check Device Health State (Red)
    // NVMe/SCSI devices usually expose a state file
    let state_path = path.join("device/state");
    if state_path.exists() {
        if let Ok(content) = fs::read_to_string(&state_path) {
            let polished = content.trim();
            if polished == "dead" || polished == "transport-offline" {
                return (LedColor::Red, Some(format!("Device state: {}", polished)));
            }
        }
    }

    // 3. Check Usage/Health (Green vs Blue)
    // SIMPLIFIED LOGIC: If checking active holders works, it's green.
    if active_devs.contains(slot.sys_name) {
        return (LedColor::Green, Some("Drive is active/mounted".into()));
    }

    // 4. Fallback (Blue)
    // Present, "live", but no holders found (Unused)
    (LedColor::Blue, Some("Drive idle/unmounted".into()))
}

// Scans /sys/block/*/holders to accept nested setups (like partitions holding the disk)
fn get_active_holders() -> HashSet<String> {
    let mut active = HashSet::new();
    
    for slot in SLOTS {
        let sys_path = format!("/sys/block/{}", slot.sys_name);
        
        // Check main holders (direct usage)
        if has_entries(&format!("{}/holders", sys_path)) {
            active.insert(slot.sys_name.to_string());
            continue;
        }

        // Check partition holders
        if let Ok(dir) = fs::read_dir(&sys_path) {
            for entry in dir.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with(slot.sys_name) {
                    // It's a partition (e.g. nvme0n1p1)
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

fn set_hardware_led(state: &LedState) -> Result<(), String> {
    // PROTOCOL NOTE: Map your specific hardware bytes here.
    // This is a placeholder mapping based on your request.
    
    let (reg, val) = match (state.device_slot.as_str(), state.color) {
        // NVME0 Examples
        ("NVME0", LedColor::White) => ("0xA1", "0x01"), 
        ("NVME0", LedColor::Blue)  => ("0xA1", "0x02"), // Example value
        ("NVME0", LedColor::Green) => ("0xA1", "0x03"), // Example value
        ("NVME0", LedColor::Red)   => ("0xB1", "0x01"), // Example value
        
        // ... Add mappings for NVME1, NVME2, etc ...
        
        (_, _) => return Ok(()), // Skip unmapped
    };

    // We use standard i2cset command for simplicity and reliability on Arch
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

fn current_time() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

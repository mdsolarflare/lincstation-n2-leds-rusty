// The packages for the data structures
use serde::{Deserialize, Serialize};
use std::collections::{HashSet, HashMap};
use std::env;
use std::fs;
use std::io::{self, BufRead, BufReader};
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
} 

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

#[derive(Clone)]
struct DriveSlot {
    slot_name: &'static str,
    sys_name: &'static str,
}

#[derive(Debug, Clone)]
pub struct DiskStats {
    pub device_name: String,
    pub prev_read_sectors: u64,
    pub prev_write_sectors: u64,
    pub prev_write_time: u64,
    pub utilization_percent: f64,
    pub is_active: bool,
}

const ACTIVITY_SAMPLE_INTERVAL_MS: u64 = 1000; // milliseconds

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
    DriveSlot { slot_name: "SATA0", sys_name: "sdb" },
    DriveSlot { slot_name: "SATA1", sys_name: "sdc" },
];

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
            let (color, msg) = check_drive_status_by_name(slot.sys_name);
            
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

pub fn read_disk_stats(disks: &mut [DiskStats]) -> io::Result<()> {
    // Read disk statistics from /proc/diskstats
    // See spec at https://www.kernel.org/doc/html/latest/admin-guide/iostats.html
    let file = fs::File::open("/proc/diskstats")?;
    let reader = BufReader::new(file);

    for line in reader.lines() {
        let line = line?;
        let parts: Vec<&str> = line.split_whitespace().collect();

        if parts.len() < 14 {
            continue;
        }

        // Parse the fields according to kernel documentation
        let _major = parts[0].parse::<u32>().unwrap_or(0);
        let _minor = parts[1].parse::<u32>().unwrap_or(0);
        let device_name = parts[2];

        // Skip partition stats (minor > 0) for simplicity
        if _minor > 0 {
            continue;
        }

        // Parse the I/O statistics fields from /proc/diskstats
        let _reads = parts[3].parse::<u64>().unwrap_or(0);
        let _reads_merged = parts[4].parse::<u64>().unwrap_or(0);
        let read_sectors = parts[5].parse::<u64>().unwrap_or(0);
        let _read_time = parts[6].parse::<u64>().unwrap_or(0);
        let _writes = parts[7].parse::<u64>().unwrap_or(0);
        let _writes_merged = parts[8].parse::<u64>().unwrap_or(0);
        let write_sectors = parts[9].parse::<u64>().unwrap_or(0);
        let _write_time = parts[10].parse::<u64>().unwrap_or(0);
        let _io_in_progress = parts[11].parse::<u64>().unwrap_or(0);
        let io_time = parts[12].parse::<u64>().unwrap_or(0);
        let _weighted_io_time = parts[13].parse::<u64>().unwrap_or(0);

        // Find matching disk in our array
        for disk in disks.iter_mut() {
            if disk.device_name == device_name {
                // Calculate utilization based on I/O time
                let mut time_diff = io_time as i64 - disk.prev_write_time as i64;
                
                if time_diff < 0 {
                    // overflow: handle wrapping of u64
                    time_diff += u64::MAX as i64;
                }
                
                if time_diff >= 0 {
                    let time_diff_f64 = time_diff as f64;
                    // time_diff is in milliseconds, convert to microseconds and calculate percentage
                    disk.utilization_percent = time_diff_f64
                        * 1000.0  // convert from milliseconds to microseconds
                        / ACTIVITY_SAMPLE_INTERVAL_MS as f64
                        * 100.0;  // convert to percentage
                    
                    if disk.utilization_percent > 100.0 {
                        disk.utilization_percent = 100.0;
                    }
                } else if time_diff <= 0 {
                    // overflow case - leave utilization as is or set to 0
                }

                // Check for activity (sectors read/written changed)
                disk.is_active = read_sectors != disk.prev_read_sectors ||
                                write_sectors != disk.prev_write_sectors;

                // Update previous values
                disk.prev_read_sectors = read_sectors;
                disk.prev_write_sectors = write_sectors;
                disk.prev_write_time = io_time;

                break;
            }
        }
    }

    Ok(())
}

// --- Debug Methods ---

fn debug_check_status() {
    println!("DEBUG: System Disk Audit & Configuration Map");
    
    // 1. Gather all physically detected block devices
    let mut detected_devices: HashSet<String> = HashSet::new();
    if let Ok(entries) = fs::read_dir("/sys/block") {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if !name.starts_with("loop") {
                detected_devices.insert(name);
            }
        }
    }

    // 2. Initialize DiskStats for ALL detected devices (for comprehensive debug info)
    let mut disk_stats: Vec<DiskStats> = detected_devices
        .iter()
        .map(|device| DiskStats {
            device_name: device.clone(),
            prev_read_sectors: 0,
            prev_write_sectors: 0,
            prev_write_time: 0,
            utilization_percent: 0.0,
            is_active: false,
        })
        .collect();

    // 3. Read disk stats from /proc/diskstats
    if let Err(e) = read_disk_stats(&mut disk_stats) {
        eprintln!("Warning: Failed to read disk stats: {}", e);
    }

    // 4. Identify Expected Slots and Map them
    let mut report_rows = Vec::new();
    
    // Map sys_name -> slot_name for quick lookup
    let mut sys_to_slot = HashMap::new();
    for slot in SLOTS {
        sys_to_slot.insert(slot.sys_name.to_string(), slot.slot_name);
    }

    // Add all DETECTED devices to list
    for sys_name in &detected_devices {
        report_rows.push(sys_name.clone());
    }
    
    // Add MISSING expected devices to list
    for slot in SLOTS {
        if !detected_devices.contains(slot.sys_name) {
            report_rows.push(slot.sys_name.to_string());
        }
    }
    
    // Deduplicate and sort (since detected might match expected)
    report_rows.sort();
    report_rows.dedup();

    println!("{:<12} | {:<12} | {:<8} | {:<8} | {:<8} | {:<12} | {:<10} | {}", 
        "DEVICE", "MAPPED SLOT", "COLOR", "REG", "VAL", "UTIL %", "I/O Active", "MESSAGE");
    println!("{}", "-".repeat(120));

    for sys_name in report_rows {
        // Determine Mapping
        let slot_label = match sys_to_slot.get(&sys_name) {
            Some(label) => *label,
            None => "(Not Mapped)",
        };

        // Determine Status
        let (color, msg) = check_drive_status_by_name(&sys_name);
        let msg_str = msg.unwrap_or_else(|| "".to_string());

        // Determine Hardware Codes (Only if mapped, otherwise N/A)
        let (reg, val) = if slot_label != "(Not Mapped)" {
            get_i2c_codes(slot_label, color)
        } else {
            ("-", "-")
        };

        // Get disk stats if available (includes all detected devices)
        let (util_str, io_active_str) = disk_stats
            .iter()
            .find(|ds| ds.device_name == sys_name)
            .map(|ds| (
                format!("{:.1}%", ds.utilization_percent),
                if ds.is_active { "Yes" } else { "No" }.to_string(),
            ))
            .unwrap_or_else(|| ("N/A".to_string(), "N/A".to_string()));

        // UI Fix: Don't show an LED color for unmapped drives, it's confusing.
        let color_display = if slot_label != "(Not Mapped)" {
            format!("{:?}", color)
        } else {
            "-".to_string()
        };

        println!("{:<12} | {:<12} | {:<8} | {:<8} | {:<8} | {:<12} | {:<10} | {}", 
            sys_name, 
            slot_label, 
            color_display, 
            reg,
            val,
            util_str,
            io_active_str,
            msg_str
        );
    }
    std::process::exit(0);
}

// --- Status Logic ---

// Check drive status based on device state and file existence
fn check_drive_status_by_name(sys_name: &str) -> (LedColor, Option<String>) {
    let sys_path = format!("/sys/block/{}", sys_name);
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

    (LedColor::Blue, Some("Drive present".into()))
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

    if reg == "0x00" || reg == "-" {
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

use std::io;
use std::fs;
use std::fs::File;
use std::path::Path;
use std::io::{BufRead, BufReader};

/// LincStation N2 Drive Status Monitor
///
/// This module reads the status of storage devices connected to the LincStation N2
/// (SATA and NVMe) and the internal eMMC.
///
/// Reference for Linux Block Devices:
/// https://docs.kernel.org/admin-guide/blockdev/index.html

/// The physical model of the disk drives in the LincStation N2
/// 8 network/storage locations
pub const SLOTS: &[DriveSlot] = &[
    // Management/OS slot
    DriveSlot { slot_name: "MGMT",  sys_name: "TBD", sys_block_path: "TBD", driver_parsing_method: DriveParsingMethod::NETWORK },
    DriveSlot { slot_name: "OS", sys_name: "mmcblk0", sys_block_path: "/device/life_time", driver_parsing_method: DriveParsingMethod::EMMC },
    // SATA Drives
    DriveSlot { slot_name: "SATA1", sys_name: "sda", sys_block_path: "/device/state", driver_parsing_method: DriveParsingMethod::SATA },
    DriveSlot { slot_name: "SATA2", sys_name: "sdb", sys_block_path: "/device/state", driver_parsing_method: DriveParsingMethod::SATA },
    // NVMe Drives
    DriveSlot { slot_name: "NVME1", sys_name: "nvme0n1", sys_block_path: "/device/state", driver_parsing_method: DriveParsingMethod::NVME },
    DriveSlot { slot_name: "NVME2", sys_name: "nvme1n1", sys_block_path: "/device/state", driver_parsing_method: DriveParsingMethod::NVME },
    DriveSlot { slot_name: "NVME3", sys_name: "nvme2n1", sys_block_path: "/device/state", driver_parsing_method: DriveParsingMethod::NVME },
    DriveSlot { slot_name: "NVME4", sys_name: "nvme3n1", sys_block_path: "/device/state", driver_parsing_method: DriveParsingMethod::NVME },
];

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DriveParsingMethod {
    NETWORK,
    EMMC,
    SATA,
    NVME,
}

#[derive(Debug, Clone, Copy)]
pub struct DriveSlot {
    pub slot_name: &'static str,
    pub sys_name: &'static str,
    pub sys_block_path: &'static str,
    pub driver_parsing_method: DriveParsingMethod,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DriveStatus {
    Healthy,
    Missing,
    Degraded,
}

/// Attempts to read the status of all defined slots.
/// Returns a Vec of tuples containing slot name and its status.
pub fn check_all_drives() -> Vec<(String, DriveStatus)> {
    SLOTS.iter().map(|slot| check_single_drive(slot)).collect()
}

/// Reads and parses a single drive slot based on its physical model and driver type.
fn check_single_drive(slot: &DriveSlot) -> (String, DriveStatus) {
    let slot_name = slot.slot_name.to_string();

    // Construct the full sysfs path
    // /sys/block/<device>/<block_path>
    let full_path = format!("/sys/block/{}{}", slot.sys_name, slot.sys_block_path);

    // Attempt to read the file content
    match read_sysfs_file(&full_path) {
        Some(content) => {
            let status = match slot.driver_parsing_method {
                DriveParsingMethod::EMMC => parse_emmc(&content),
                DriveParsingMethod::SATA => parse_sata(&content),
                DriveParsingMethod::NVME => parse_nvme(&content),
                DriveParsingMethod::NETWORK => DriveStatus::Healthy, // Network doesn't use these files in this model
            };
            (slot_name, status)
        }
        None => {
            // If we fail to look it up, we get DriveStatus::Missing
            (slot_name, DriveStatus::Missing)
        }
    }
}

/// Reads a string from the filesystem.
fn read_sysfs_file(path: &str) -> Option<String> {
    fs::read_to_string(path).ok()
}

/// Parses EMMC life_time.
///
/// The two values represent the estimated "wear and tear" on the internal NAND flash.
/// The values correspond to different types of memory blocks:
/// * **Value 1 (Type A):** Usually represents SLC/Enhanced memory.
/// * **Value 2 (Type B):** Usually represents MLC/TLC memory blocks.
///
/// Hex Value to Life Used Percentage Mapping:
/// * 0x01 (0-10%) -> Brand New
/// * 0x0A (90-100%) -> Critical
/// * 0x0B (>110%) -> Danger
///
/// Logic: If < 60% (Hex < 0x3B) -> Healthy, else -> Degraded.
fn parse_emmc(life_time: &str) -> DriveStatus {
    // Assuming the file contains a hex string like "0xXX"
    match u8::from_str_radix(life_time.trim_start_matches("0x"), 16).ok() {
        Some(hex_val) if hex_val < 0x3B => DriveStatus::Healthy, // < 60%
        _ => DriveStatus::Degraded,
    }
}

/// Parses SATA device state.
///
/// | State | Meaning |
/// |---|---|
/// | `running` | Normal. The device is active. |
/// | `offline` | Bad. The kernel has disabled the device. |
/// | `blocked` | Commands are being held back (error recovery). |
/// | `created` | Initial state. |
///
/// Logic: If 'running' -> Healthy, else -> Degraded.
fn parse_sata(state: &str) -> DriveStatus {
    match state.trim() {
        "running" => DriveStatus::Healthy,
        _ => DriveStatus::Degraded,
    }
}

/// Parses NVMe device state.
///
/// | State | Meaning |
/// |---|---|
/// | `live` | Normal. The drive is ready for I/O. |
/// | `new` | Device detected but setup not complete. |
/// | `resetting` | Driver is resetting the controller. |
/// | `dead` | Fatal Error. Controller failed to initialize. |
///
/// Logic: If 'live' -> Healthy, else -> Degraded.
fn parse_nvme(state: &str) -> DriveStatus {
    match state.trim() {
        "live" => DriveStatus::Healthy,
        _ => DriveStatus::Degraded,
    }
}


// TODO this is the disk stats model, document better
#[derive(Debug, Clone, Default)]
pub struct DiskStats {
    // Metadata fields
    pub major: u32,
    pub minor: u32,
    pub device_name: String,

    // Field 1: # of reads completed [1]
    pub reads_completed: u64,
    // Field 2: # of reads merged, Field 6: # of writes merged [1]
    pub reads_merged: u64,
    // Field 3: # of sectors read [1]
    pub sectors_read: u64,
    // Field 4: # milliseconds spent reading [1]
    pub ms_reading: u64,
    // Field 5: # writes completed [1]
    pub writes_completed: u64,
    // Field 6: # writes merged [1]
    pub writes_merged: u64,
    // Field 7: # sectors written [1]
    pub sectors_written: u64,
    // Field 8: # milliseconds spent writing [1]
    pub ms_writing: u64,
    // Field 9: # I/Os currently in progress [1]
    pub io_in_progress: u64,
    // Field 10: # milliseconds spent doing I/Os [1]
    pub ms_doing_io: u64,
    // Field 11: weighted # milliseconds spent doing I/Os [1]
    pub weighted_ms_doing_io: u64,
    // Field 12: # discards completed [1]
    pub discards_completed: u64,
    // Field 13: # discards merged [1]
    pub discards_merged: u64,
    // Field 14: # sectors discarded [1]
    pub sectors_discarded: u64,
    // Field 15: # milliseconds spent discarding [1]
    pub ms_discarding: u64,
    // Field 16: # flush requests completed [1]
    pub flush_requests_completed: u64,
    // Field 17: # milliseconds spent flushing [1]
    pub ms_flushing: u64,
}

impl From<String> for DiskStats {
    fn from(line: String) -> Self {
        let parts: Vec<&str> = line.split_whitespace().collect();

        // The kernel documentation defines 17 statistics fields starting at index 3 [1].
        // We iterate through indices 0-19 to cover Major, Minor, Name, and Fields 1-17.

        // Helper to safely parse values, defaulting to 0 if index is out of bounds or parsing fails.
        // Note: parse() is generic, so we specify ::&lt;u64&gt; to tell the compiler the target type.
        let safe_parse_u64 = |idx: usize| -> u64 {
            parts.get(idx)
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(0)
        };
        let safe_parse_u32 = |idx: usize| -> u32 {
            parts.get(idx)
                .and_then(|s| s.parse::<u32>().ok())
                .unwrap_or(0)
        };

        Self {
            major: safe_parse_u32(0),
            minor: safe_parse_u32(1),
            device_name: parts.get(2).unwrap_or(&"").to_string(),

            // Fields 1-17
            reads_completed: safe_parse_u64(3),
            reads_merged: safe_parse_u64(4),
            sectors_read: safe_parse_u64(5),
            ms_reading: safe_parse_u64(6),
            writes_completed: safe_parse_u64(7),
            writes_merged: safe_parse_u64(8),
            sectors_written: safe_parse_u64(9),
            ms_writing: safe_parse_u64(10),
            io_in_progress: safe_parse_u64(11),
            ms_doing_io: safe_parse_u64(12),
            weighted_ms_doing_io: safe_parse_u64(13),
            discards_completed: safe_parse_u64(14),
            discards_merged: safe_parse_u64(15),
            sectors_discarded: safe_parse_u64(16),
            ms_discarding: safe_parse_u64(17),
            flush_requests_completed: safe_parse_u64(18),
            ms_flushing: safe_parse_u64(19),
        }
    }
}

/// Read disk statistics from /proc/diskstats
/// See spec at https://www.kernel.org/doc/html/latest/admin-guide/iostats.html
pub fn read_disk_stats() -> io::Result<Vec<DiskStats>> {
    let file = File::open("/proc/diskstats")?;
    let reader = BufReader::new(file);
    let mut stats_list = Vec::new();

    for line in reader.lines() {
        let line = line?;

        // Parse the raw line using the From trait
        let stats = DiskStats::from(line);

        // Filter out partitions (minor > 0) to focus on physical devices
        if stats.minor > 0 {
            continue;
        }

        stats_list.push(stats);
    }

    Ok(stats_list)
}

fn get_disk_stats_over_interval() -> Option<Vec<DiskStats>> {
    // 1. Read initial disk stats
    let initial_disk_stats = match read_disk_stats() {
        Ok(stats) => stats,
        Err(e) => {
            eprintln!("Error reading initial disk stats: {}", e);
            return None;
        }
    };

    // 2. Wait 500 ms
    std::thread::sleep(std::time::Duration::from_millis(500));

    // 3. Read disk stats again
    let final_disk_stats = match read_disk_stats() {
        Ok(stats) => stats,
        Err(e) => {
            eprintln!("Error reading final disk stats: {}", e);
            return None;
        }
    };
    Some(initial_disk_stats)
}
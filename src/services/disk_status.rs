use std::io;
use std::fs;
use std::fs::File;
use std::path::Path;
use std::io::{BufRead, BufReader};

#[derive(Clone)]
pub struct DriveSlot {
    pub slot_name: &'static str,
    pub sys_name: &'static str,
}

/// Registry of all physical drive slots on the LincStation N2.
/// 8 storage locations
pub const SLOTS: &[DriveSlot] = &[
    // Management/OS slot
    DriveSlot { slot_name: "MGMT",  sys_name: "TBD" },
    DriveSlot { slot_name: "OS", sys_name: "mmcblk0" },
    // SATA Drives
    DriveSlot { slot_name: "SATA1", sys_name: "sda" },
    DriveSlot { slot_name: "SATA2", sys_name: "sdb" },
    // NVME Drives
    DriveSlot { slot_name: "NVME1", sys_name: "nvme0n1" },
    DriveSlot { slot_name: "NVME2", sys_name: "nvme1n1" },
    DriveSlot { slot_name: "NVME3", sys_name: "nvme2n1" },
    DriveSlot { slot_name: "NVME4", sys_name: "nvme3n1" },
];

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DriveStatus {
    Healthy,
    Missing,
    Degraded,
}

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

/// Check drive health status based on device state and file existence
pub fn check_drive_status(sys_name: &str) -> DriveStatus {
    let sys_path = format!("/sys/block/{}", sys_name);
    let path = Path::new(&sys_path);

    if !path.exists() {
        return DriveStatus::Missing;
    }

    let state_path = path.join("device/state");
    if state_path.exists() {
        if let Ok(content) = fs::read_to_string(&state_path) {
            let polished = content.trim();
            if polished == "dead" || polished == "transport-offline" {
                return DriveStatus::Degraded;
            }
        }
    }

    DriveStatus::Healthy
}

/// Build a sorted list of all devices to report on (detected + expected slots)
pub fn build_device_report_list(detected_devices: &[String]) -> Vec<String> {
    use std::collections::HashSet;
    
    let mut report_rows: HashSet<String> = HashSet::new();
    
    // Add all detected devices
    for device in detected_devices {
        report_rows.insert(device.clone());
    }
    
    // Add all expected slots that weren't detected
    for slot in SLOTS {
        report_rows.insert(slot.sys_name.to_string());
    }
    
    let mut result: Vec<String> = report_rows.into_iter().collect();
    result.sort();
    result
}

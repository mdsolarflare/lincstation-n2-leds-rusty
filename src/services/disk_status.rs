use std::io;
use std::fs;
use std::path::Path;
use std::io::{BufRead, BufReader};

const ACTIVITY_SAMPLE_INTERVAL_MS: u64 = 1000; // milliseconds

#[derive(Clone)]
pub struct DriveSlot {
    pub slot_name: &'static str,
    pub sys_name: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DriveStatus {
    Healthy,
    Missing,
    Degraded,
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

/// Registry of all physical drive slots on the LincStation N2.
/// 7 storage locations
pub const SLOTS: &[DriveSlot] = &[
    // Position 1: OS Drive (maps to network LED)
    DriveSlot { slot_name: "OS",    sys_name: "mmcblk0" }, 
    // Position 2-3: SATA Drives (HDD0, HDD1)
    DriveSlot { slot_name: "SATA1", sys_name: "sda" },
    DriveSlot { slot_name: "SATA2", sys_name: "sdb" },
    // Position 4-7: NVME Drives
    DriveSlot { slot_name: "NVME1", sys_name: "nvme0n1" },
    DriveSlot { slot_name: "NVME2", sys_name: "nvme1n1" },
    DriveSlot { slot_name: "NVME3", sys_name: "nvme2n1" },
    DriveSlot { slot_name: "NVME4", sys_name: "nvme3n1" },
];

/// Read disk statistics from /proc/diskstats
/// See spec at https://www.kernel.org/doc/html/latest/admin-guide/iostats.html
pub fn read_disk_stats(disks: &mut [DiskStats]) -> io::Result<()> {
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

/// Detect all block devices and initialize DiskStats for them
/// Excludes loop devices for cleaner output
pub fn detect_all_devices() -> io::Result<Vec<DiskStats>> {
    use std::collections::HashSet;
    
    let mut detected_devices: HashSet<String> = HashSet::new();
    if let Ok(entries) = fs::read_dir("/sys/block") {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            // Skip loop devices for cleaner debug output
            if !name.starts_with("loop") {
                detected_devices.insert(name);
            }
        }
    }

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

    // Read actual stats from /proc/diskstats
    read_disk_stats(&mut disk_stats)?;
    
    Ok(disk_stats)
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

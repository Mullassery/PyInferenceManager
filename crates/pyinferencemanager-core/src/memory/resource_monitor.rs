use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VramInfo {
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub shared_with_system: bool,
}

impl VramInfo {
    pub fn available_bytes(&self) -> u64 {
        self.total_bytes.saturating_sub(self.used_bytes)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceSnapshot {
    pub cpu_core_count: usize,
    pub available_memory_bytes: u64,
    pub total_memory_bytes: u64,
    pub vram_info: Option<VramInfo>,
    pub available_disk_bytes: Option<u64>,
}

impl ResourceSnapshot {
    pub fn available_memory_gb(&self) -> f32 {
        self.available_memory_bytes as f32 / 1_073_741_824.0
    }

    pub fn total_memory_gb(&self) -> f32 {
        self.total_memory_bytes as f32 / 1_073_741_824.0
    }

    pub fn memory_utilization(&self) -> f32 {
        if self.total_memory_bytes == 0 {
            0.0
        } else {
            (self.total_memory_bytes - self.available_memory_bytes) as f32
                / self.total_memory_bytes as f32
        }
    }

    pub fn available_vram_bytes(&self) -> Option<u64> {
        self.vram_info.as_ref().map(|v| v.available_bytes())
    }

    pub fn available_vram_gb(&self) -> Option<f32> {
        self.available_vram_bytes()
            .map(|b| b as f32 / 1_073_741_824.0)
    }
}

pub struct ResourceMonitor;

impl ResourceMonitor {
    pub fn cpu_core_count() -> usize {
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1)
    }

    pub fn detect_available_memory() -> crate::Result<u64> {
        #[cfg(target_os = "macos")]
        {
            Self::detect_available_memory_macos()
        }

        #[cfg(target_os = "linux")]
        {
            Self::detect_available_memory_linux()
        }

        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        {
            Err(crate::Error::HardwareError(
                "Unsupported platform for memory detection".to_string(),
            ))
        }
    }

    #[cfg(target_os = "macos")]
    fn detect_available_memory_macos() -> crate::Result<u64> {
        let output = Command::new("vm_stat")
            .output()
            .map_err(|e| crate::Error::HardwareError(format!("vm_stat failed: {}", e)))?;

        let stdout = String::from_utf8(output.stdout)
            .map_err(|e| crate::Error::HardwareError(format!("UTF-8 error: {}", e)))?;

        for line in stdout.lines() {
            if line.contains("Pages free:") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 3 {
                    let pages = parts[2]
                        .trim_end_matches('.')
                        .parse::<u64>()
                        .map_err(|e| crate::Error::HardwareError(format!("parse error: {}", e)))?;
                    return Ok(pages * 4096);
                }
            }
        }

        Err(crate::Error::HardwareError(
            "Could not find 'Pages free' in vm_stat output".to_string(),
        ))
    }

    #[cfg(target_os = "linux")]
    fn detect_available_memory_linux() -> crate::Result<u64> {
        let output = Command::new("grep")
            .args(["MemAvailable:", "/proc/meminfo"])
            .output()
            .map_err(|e| crate::Error::HardwareError(format!("grep failed: {}", e)))?;

        let stdout = String::from_utf8(output.stdout)
            .map_err(|e| crate::Error::HardwareError(format!("UTF-8 error: {}", e)))?;

        let parts: Vec<&str> = stdout.split_whitespace().collect();
        if parts.len() >= 2 {
            let kb = parts[1]
                .parse::<u64>()
                .map_err(|e| crate::Error::HardwareError(format!("parse error: {}", e)))?;
            Ok(kb * 1024)
        } else {
            Err(crate::Error::HardwareError(
                "unexpected /proc/meminfo format".to_string(),
            ))
        }
    }

    pub fn detect_vram() -> Option<VramInfo> {
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        {
            Self::detect_apple_silicon_vram()
        }

        #[cfg(target_os = "linux")]
        {
            Self::detect_nvidia_vram()
        }

        #[cfg(not(any(all(target_os = "macos", target_arch = "aarch64"), target_os = "linux")))]
        {
            None
        }
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn detect_apple_silicon_vram() -> Option<VramInfo> {
        if let Ok(output) = Command::new("sysctl").args(["hw.memsize"]).output() {
            if let Ok(stdout) = String::from_utf8(output.stdout) {
                if let Ok(total) = stdout.trim().parse::<u64>() {
                    if let Ok(available) = Self::detect_available_memory() {
                        return Some(VramInfo {
                            total_bytes: total,
                            used_bytes: total.saturating_sub(available),
                            shared_with_system: true,
                        });
                    }
                }
            }
        }
        None
    }

    #[cfg(target_os = "linux")]
    fn detect_nvidia_vram() -> Option<VramInfo> {
        if let Ok(output) = Command::new("nvidia-smi")
            .args([
                "--query-gpu=memory.total,memory.used",
                "--format=csv,noheader,nounits",
            ])
            .output()
        {
            if output.status.success() {
                if let Ok(stdout) = String::from_utf8(output.stdout) {
                    let parts: Vec<&str> = stdout.trim().split(',').collect();
                    if parts.len() >= 2 {
                        if let (Ok(total_mb), Ok(used_mb)) = (
                            parts[0].trim().parse::<u64>(),
                            parts[1].trim().parse::<u64>(),
                        ) {
                            return Some(VramInfo {
                                total_bytes: total_mb * 1_048_576,
                                used_bytes: used_mb * 1_048_576,
                                shared_with_system: false,
                            });
                        }
                    }
                }
            }
        }
        None
    }

    pub fn detect_available_disk(path: &str) -> crate::Result<u64> {
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        {
            let output = Command::new("df")
                .arg(path)
                .output()
                .map_err(|e| crate::Error::HardwareError(format!("df failed: {}", e)))?;

            let stdout = String::from_utf8(output.stdout)
                .map_err(|e| crate::Error::HardwareError(format!("UTF-8 error: {}", e)))?;

            for line in stdout.lines().skip(1) {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 4 {
                    let available = parts[3]
                        .parse::<u64>()
                        .map_err(|e| crate::Error::HardwareError(format!("parse error: {}", e)))?;
                    return Ok(available * 1024);
                }
            }
            Err(crate::Error::HardwareError(
                "Could not parse df output".to_string(),
            ))
        }

        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        {
            Err(crate::Error::HardwareError(
                "Unsupported platform for disk detection".to_string(),
            ))
        }
    }

    pub async fn snapshot(total_memory: u64) -> crate::Result<ResourceSnapshot> {
        let cpu_core_count = Self::cpu_core_count();
        let available_memory_bytes = Self::detect_available_memory()?;
        let vram_info = Self::detect_vram();
        let available_disk_bytes = Self::detect_available_disk("/").ok();

        Ok(ResourceSnapshot {
            cpu_core_count,
            available_memory_bytes,
            total_memory_bytes: total_memory,
            vram_info,
            available_disk_bytes,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cpu_core_count() {
        let cores = ResourceMonitor::cpu_core_count();
        assert!(cores > 0);
    }

    #[test]
    fn test_detect_available_memory() {
        let result = ResourceMonitor::detect_available_memory();
        assert!(result.is_ok());
        let available = result.unwrap();
        assert!(available > 0);
    }

    #[test]
    fn test_detect_vram_does_not_panic() {
        let _vram = ResourceMonitor::detect_vram();
    }

    #[test]
    fn test_detect_available_disk() {
        let result = ResourceMonitor::detect_available_disk("/");
        assert!(result.is_ok());
        assert!(result.unwrap() > 0);
    }

    #[test]
    fn test_resource_snapshot_creation() {
        let snapshot = ResourceSnapshot {
            cpu_core_count: 8,
            available_memory_bytes: 8 * 1_073_741_824,
            total_memory_bytes: 16 * 1_073_741_824,
            vram_info: None,
            available_disk_bytes: Some(100 * 1_073_741_824),
        };

        assert_eq!(snapshot.available_memory_gb(), 8.0);
        assert_eq!(snapshot.total_memory_gb(), 16.0);
        assert!(snapshot.memory_utilization() > 0.4);
    }

    #[test]
    fn test_vram_info_available() {
        let vram = VramInfo {
            total_bytes: 8 * 1_073_741_824,
            used_bytes: 2 * 1_073_741_824,
            shared_with_system: false,
        };

        assert_eq!(vram.available_bytes(), 6 * 1_073_741_824);
    }
}

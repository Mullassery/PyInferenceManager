use crate::types::HardwareProfile;
use crate::Result;
use std::process::Command;

pub struct HardwareProfiler;

impl HardwareProfiler {
    pub fn detect_total_memory() -> Result<u64> {
        #[cfg(target_os = "macos")]
        {
            let output = Command::new("sysctl")
                .args(["-n", "hw.memsize"])
                .output()
                .map_err(|e| crate::Error::HardwareError(format!("sysctl failed: {}", e)))?;

            let stdout = String::from_utf8(output.stdout)
                .map_err(|e| crate::Error::HardwareError(format!("UTF-8 error: {}", e)))?;
            let bytes = stdout
                .trim()
                .parse::<u64>()
                .map_err(|e| crate::Error::HardwareError(format!("parse error: {}", e)))?;

            Ok(bytes)
        }

        #[cfg(target_os = "linux")]
        {
            let output = Command::new("grep")
                .args(["MemTotal:", "/proc/meminfo"])
                .output()
                .map_err(|e| crate::Error::HardwareError(format!("grep failed: {}", e)))?;

            let stdout = String::from_utf8(output.stdout)
                .map_err(|e| crate::Error::HardwareError(format!("UTF-8 error: {}", e)))?;

            let parts: Vec<&str> = stdout.split_whitespace().collect();
            if parts.len() < 2 {
                return Err(crate::Error::HardwareError(
                    "unexpected /proc/meminfo format".to_string(),
                ));
            }

            let kb = parts[1]
                .parse::<u64>()
                .map_err(|e| crate::Error::HardwareError(format!("parse error: {}", e)))?;
            Ok(kb * 1024)
        }

        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        {
            Err(crate::Error::HardwareError(
                "Unsupported platform for hardware detection".to_string(),
            ))
        }
    }

    pub fn detect_apple_silicon() -> bool {
        #[cfg(target_os = "macos")]
        {
            if let Ok(output) = Command::new("sysctl").args(["hw.optional.arm64"]).output() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                return stdout.contains("1");
            }
            false
        }

        #[cfg(not(target_os = "macos"))]
        false
    }

    pub fn detect_metal() -> bool {
        #[cfg(target_os = "macos")]
        {
            std::path::Path::new("/System/Library/Frameworks/Metal.framework").exists()
        }

        #[cfg(not(target_os = "macos"))]
        false
    }

    pub async fn profile() -> Result<HardwareProfile> {
        Self::profile_with_ollama("http://localhost:11434").await
    }

    pub async fn profile_with_ollama(ollama_url: &str) -> Result<HardwareProfile> {
        let total_memory = Self::detect_total_memory()?;
        let is_apple_silicon = Self::detect_apple_silicon();
        let has_metal = Self::detect_metal();

        let mut profile = HardwareProfile::new(total_memory)
            .with_apple_silicon(is_apple_silicon)
            .with_metal(has_metal);

        let recommended_tier = profile.recommended_model_tier.clone();
        let (models, best_model, embedding_model) =
            crate::hardware::ollama_probe::OllamaProbe::probe_models(ollama_url, &recommended_tier)
                .await
                .unwrap_or_default();

        profile = profile.with_models(models, best_model, embedding_model);

        Ok(profile)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profiler_new() {
        let _profiler = HardwareProfiler;
    }

    #[tokio::test]
    async fn test_profile_async() {
        let profile = HardwareProfiler::profile().await;
        assert!(profile.is_ok());
        let p = profile.unwrap();
        assert!(p.total_memory_bytes > 0);
    }

    #[test]
    fn test_detect_apple_silicon() {
        let is_apple = HardwareProfiler::detect_apple_silicon();
        #[cfg(target_os = "macos")]
        {
            assert!(is_apple);
        }
        #[cfg(not(target_os = "macos"))]
        {
            assert!(!is_apple);
        }
    }

    #[test]
    fn test_detect_metal() {
        let has_metal = HardwareProfiler::detect_metal();
        #[cfg(target_os = "macos")]
        {
            assert!(has_metal);
        }
        #[cfg(not(target_os = "macos"))]
        {
            assert!(!has_metal);
        }
    }
}

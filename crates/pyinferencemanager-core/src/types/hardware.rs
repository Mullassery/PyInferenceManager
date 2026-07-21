use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum MemoryTier {
    Gb8,
    Gb16,
    Gb24,
    Gb32,
    Gb64,
    Gb96,
    Gb192,
    Unknown(u64),
}

impl MemoryTier {
    pub fn from_bytes(bytes: u64) -> Self {
        let gb = bytes / 1_073_741_824;
        match gb {
            0..=8 => MemoryTier::Gb8,
            9..=16 => MemoryTier::Gb16,
            17..=24 => MemoryTier::Gb24,
            25..=32 => MemoryTier::Gb32,
            33..=64 => MemoryTier::Gb64,
            65..=96 => MemoryTier::Gb96,
            97..=192 => MemoryTier::Gb192,
            n => MemoryTier::Unknown(n),
        }
    }

    pub fn to_string(&self) -> String {
        match self {
            MemoryTier::Gb8 => "8gb".to_string(),
            MemoryTier::Gb16 => "16gb".to_string(),
            MemoryTier::Gb24 => "24gb".to_string(),
            MemoryTier::Gb32 => "32gb".to_string(),
            MemoryTier::Gb64 => "64gb".to_string(),
            MemoryTier::Gb96 => "96gb".to_string(),
            MemoryTier::Gb192 => "192gb".to_string(),
            MemoryTier::Unknown(n) => format!("{}gb", n),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ModelTier {
    Tiny,
    Small,
    Medium,
    Large,
    XLarge,
}

impl ModelTier {
    pub fn for_memory(mem: &MemoryTier) -> Self {
        match mem {
            MemoryTier::Gb8 => ModelTier::Tiny,
            MemoryTier::Gb16 => ModelTier::Small,
            MemoryTier::Gb24 => ModelTier::Medium,
            MemoryTier::Gb32 => ModelTier::Medium,
            MemoryTier::Gb64 | MemoryTier::Gb96 => ModelTier::Large,
            MemoryTier::Gb192 => ModelTier::XLarge,
            MemoryTier::Unknown(_) => ModelTier::Tiny,
        }
    }

    pub fn default_model(&self) -> &'static str {
        match self {
            ModelTier::Tiny => "qwen2.5:1.5b",
            ModelTier::Small => "llama3.2:latest",
            ModelTier::Medium => "qwen2.5:14b",
            ModelTier::Large => "qwen2.5:32b",
            ModelTier::XLarge => "llama3.3:70b",
        }
    }

    pub fn to_string(&self) -> String {
        match self {
            ModelTier::Tiny => "tiny".to_string(),
            ModelTier::Small => "small".to_string(),
            ModelTier::Medium => "medium".to_string(),
            ModelTier::Large => "large".to_string(),
            ModelTier::XLarge => "xlarge".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareProfile {
    pub total_memory_bytes: u64,
    pub memory_tier: MemoryTier,
    pub recommended_model_tier: ModelTier,
    pub is_apple_silicon: bool,
    pub has_metal: bool,
    pub available_ollama_models: Vec<String>,
    pub best_available_model: Option<String>,
    pub best_embedding_model: Option<String>,
}

impl HardwareProfile {
    pub fn new(total_memory_bytes: u64) -> Self {
        let memory_tier = MemoryTier::from_bytes(total_memory_bytes);
        let recommended_model_tier = ModelTier::for_memory(&memory_tier);

        HardwareProfile {
            total_memory_bytes,
            memory_tier,
            recommended_model_tier,
            is_apple_silicon: false,
            has_metal: false,
            available_ollama_models: Vec::new(),
            best_available_model: None,
            best_embedding_model: None,
        }
    }

    pub fn with_apple_silicon(mut self, is_apple_silicon: bool) -> Self {
        self.is_apple_silicon = is_apple_silicon;
        self
    }

    pub fn with_metal(mut self, has_metal: bool) -> Self {
        self.has_metal = has_metal;
        self
    }

    pub fn with_models(
        mut self,
        available: Vec<String>,
        best: Option<String>,
        best_embedding: Option<String>,
    ) -> Self {
        self.available_ollama_models = available;
        self.best_available_model = best;
        self.best_embedding_model = best_embedding;
        self
    }

    pub fn total_memory_gb(&self) -> u64 {
        self.total_memory_bytes / 1_073_741_824
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_tier_from_bytes() {
        assert_eq!(MemoryTier::from_bytes(8 * 1_073_741_824), MemoryTier::Gb8);
        assert_eq!(MemoryTier::from_bytes(16 * 1_073_741_824), MemoryTier::Gb16);
        assert_eq!(MemoryTier::from_bytes(24 * 1_073_741_824), MemoryTier::Gb24);
        assert_eq!(MemoryTier::from_bytes(32 * 1_073_741_824), MemoryTier::Gb32);
        assert_eq!(MemoryTier::from_bytes(64 * 1_073_741_824), MemoryTier::Gb64);
        assert_eq!(MemoryTier::from_bytes(96 * 1_073_741_824), MemoryTier::Gb96);
        assert_eq!(MemoryTier::from_bytes(192 * 1_073_741_824), MemoryTier::Gb192);
    }

    #[test]
    fn test_memory_tier_boundaries() {
        // Just below 9GB should be Gb8
        assert_eq!(
            MemoryTier::from_bytes(8_999_999_999),
            MemoryTier::Gb8
        );
        // 9GB should be Gb16
        assert_eq!(
            MemoryTier::from_bytes(9 * 1_073_741_824),
            MemoryTier::Gb16
        );
    }

    #[test]
    fn test_model_tier_for_memory() {
        assert_eq!(ModelTier::for_memory(&MemoryTier::Gb8), ModelTier::Tiny);
        assert_eq!(ModelTier::for_memory(&MemoryTier::Gb16), ModelTier::Small);
        assert_eq!(ModelTier::for_memory(&MemoryTier::Gb24), ModelTier::Medium);
        assert_eq!(ModelTier::for_memory(&MemoryTier::Gb32), ModelTier::Medium);
        assert_eq!(ModelTier::for_memory(&MemoryTier::Gb64), ModelTier::Large);
        assert_eq!(ModelTier::for_memory(&MemoryTier::Gb96), ModelTier::Large);
        assert_eq!(ModelTier::for_memory(&MemoryTier::Gb192), ModelTier::XLarge);
    }

    #[test]
    fn test_model_tier_default_models() {
        assert_eq!(ModelTier::Tiny.default_model(), "qwen2.5:1.5b");
        assert_eq!(ModelTier::Small.default_model(), "llama3.2:latest");
        assert_eq!(ModelTier::Medium.default_model(), "qwen2.5:14b");
        assert_eq!(ModelTier::Large.default_model(), "qwen2.5:32b");
        assert_eq!(ModelTier::XLarge.default_model(), "llama3.3:70b");
    }

    #[test]
    fn test_hardware_profile_new() {
        let profile = HardwareProfile::new(16 * 1_073_741_824);
        assert_eq!(profile.total_memory_bytes, 16 * 1_073_741_824);
        assert_eq!(profile.memory_tier, MemoryTier::Gb16);
        assert_eq!(profile.recommended_model_tier, ModelTier::Small);
        assert!(!profile.is_apple_silicon);
    }

    #[test]
    fn test_hardware_profile_builders() {
        let profile = HardwareProfile::new(16 * 1_073_741_824)
            .with_apple_silicon(true)
            .with_metal(true)
            .with_models(
                vec!["llama3.2:latest".to_string()],
                Some("llama3.2:latest".to_string()),
                Some("nomic-embed-text".to_string()),
            );

        assert!(profile.is_apple_silicon);
        assert!(profile.has_metal);
        assert_eq!(
            profile.best_available_model,
            Some("llama3.2:latest".to_string())
        );
        assert_eq!(
            profile.best_embedding_model,
            Some("nomic-embed-text".to_string())
        );
    }

    #[test]
    fn test_total_memory_gb() {
        let profile = HardwareProfile::new(24 * 1_073_741_824);
        assert_eq!(profile.total_memory_gb(), 24);
    }

    #[test]
    fn test_memory_tier_to_string() {
        assert_eq!(MemoryTier::Gb8.to_string(), "8gb");
        assert_eq!(MemoryTier::Gb16.to_string(), "16gb");
        assert_eq!(MemoryTier::Unknown(256).to_string(), "256gb");
    }

    #[test]
    fn test_model_tier_to_string() {
        assert_eq!(ModelTier::Tiny.to_string(), "tiny");
        assert_eq!(ModelTier::Small.to_string(), "small");
        assert_eq!(ModelTier::Medium.to_string(), "medium");
        assert_eq!(ModelTier::Large.to_string(), "large");
        assert_eq!(ModelTier::XLarge.to_string(), "xlarge");
    }
}

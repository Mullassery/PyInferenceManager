use crate::types::{MemoryTier, ModelTier};

pub struct MemoryMap;

impl MemoryMap {
    pub fn tier_for_bytes(bytes: u64) -> MemoryTier {
        MemoryTier::from_bytes(bytes)
    }

    pub fn model_tier_for_memory(memory_tier: &MemoryTier) -> ModelTier {
        ModelTier::for_memory(memory_tier)
    }

    pub fn default_model_for_tier(tier: &ModelTier) -> &'static str {
        tier.default_model()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tier_for_bytes() {
        let tier = MemoryMap::tier_for_bytes(16 * 1_073_741_824);
        assert_eq!(tier, MemoryTier::Gb16);
    }

    #[test]
    fn test_model_tier_for_memory() {
        let memory_tier = MemoryTier::Gb32;
        let model_tier = MemoryMap::model_tier_for_memory(&memory_tier);
        assert_eq!(model_tier, ModelTier::Medium);
    }

    #[test]
    fn test_default_model_for_tier() {
        let model = MemoryMap::default_model_for_tier(&ModelTier::Small);
        assert_eq!(model, "llama3.2:latest");
    }
}

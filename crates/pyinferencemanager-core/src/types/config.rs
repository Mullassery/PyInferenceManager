use serde::{Deserialize, Serialize};

use super::dag::CloudProvider;
use super::hardware::ModelTier;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ExecutionMode {
    LocalFirst,
    CloudFirst,
}

impl Default for ExecutionMode {
    fn default() -> Self {
        ExecutionMode::LocalFirst
    }
}

impl ExecutionMode {
    pub fn as_str(&self) -> &str {
        match self {
            ExecutionMode::LocalFirst => "local_first",
            ExecutionMode::CloudFirst => "cloud_first",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "local_first" => Some(ExecutionMode::LocalFirst),
            "cloud_first" => Some(ExecutionMode::CloudFirst),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalModelEntry {
    pub name: String,
    pub tier: ModelTier,
    pub context_length: u32,
    pub is_embedding_model: bool,
}

impl LocalModelEntry {
    pub fn new(name: String, tier: ModelTier, context_length: u32) -> Self {
        LocalModelEntry {
            name,
            tier,
            context_length,
            is_embedding_model: false,
        }
    }

    pub fn with_embedding(mut self, is_embedding: bool) -> Self {
        self.is_embedding_model = is_embedding;
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudModelEntry {
    pub provider: CloudProvider,
    pub model_id: String,
    pub cost_per_1k_input: f32,
    pub cost_per_1k_output: f32,
    pub context_length: u32,
    pub priority: u32,  // 1=highest (primary), 10=lowest (fallback)
}

impl CloudModelEntry {
    pub fn new(
        provider: CloudProvider,
        model_id: String,
        cost_per_1k_input: f32,
        cost_per_1k_output: f32,
        context_length: u32,
    ) -> Self {
        CloudModelEntry {
            provider,
            model_id,
            cost_per_1k_input,
            cost_per_1k_output,
            context_length,
            priority: 5,
        }
    }

    pub fn with_priority(mut self, priority: u32) -> Self {
        self.priority = priority.clamp(1, 10);
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModelRegistry {
    pub local: Vec<LocalModelEntry>,
    pub cloud: Vec<CloudModelEntry>,
}

impl ModelRegistry {
    pub fn new() -> Self {
        ModelRegistry {
            local: Vec::new(),
            cloud: Vec::new(),
        }
    }

    pub fn add_local(&mut self, entry: LocalModelEntry) {
        self.local.push(entry);
    }

    pub fn add_cloud(&mut self, entry: CloudModelEntry) {
        self.cloud.push(entry);
    }

    pub fn best_local_for_tier(&self, tier: &ModelTier) -> Option<&LocalModelEntry> {
        self.local.iter().find(|e| &e.tier == tier)
    }

    pub fn embedding_model(&self) -> Option<&LocalModelEntry> {
        self.local.iter().find(|e| e.is_embedding_model)
    }

    pub fn primary_cloud(&self) -> Option<&CloudModelEntry> {
        self.cloud.first()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchestratorConfig {
    pub execution_mode: ExecutionMode,
    pub models: ModelRegistry,
    pub cloud_complexity_threshold: f32,
    pub local_complexity_threshold: f32,
    pub cache_similarity_threshold: f32,
    pub cache_ttl_seconds: u64,
    pub auto_pull_missing_models: bool,
    pub ollama_base_url: String,
    pub db_path: String,
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        OrchestratorConfig {
            execution_mode: ExecutionMode::LocalFirst,
            models: ModelRegistry::new(),
            cloud_complexity_threshold: 0.7,
            local_complexity_threshold: 0.3,
            cache_similarity_threshold: 0.88,
            cache_ttl_seconds: 3600,
            auto_pull_missing_models: true,
            ollama_base_url: "http://localhost:11434".to_string(),
            db_path: "~/.pyinferencemanager/cache.db".to_string(),
        }
    }
}

impl OrchestratorConfig {
    pub fn new() -> Self {
        OrchestratorConfig::default()
    }

    pub fn with_execution_mode(mut self, mode: ExecutionMode) -> Self {
        self.execution_mode = mode;
        self
    }

    pub fn with_models(mut self, models: ModelRegistry) -> Self {
        self.models = models;
        self
    }

    pub fn with_thresholds(
        mut self,
        cloud_threshold: f32,
        local_threshold: f32,
        cache_threshold: f32,
    ) -> Self {
        self.cloud_complexity_threshold = cloud_threshold;
        self.local_complexity_threshold = local_threshold;
        self.cache_similarity_threshold = cache_threshold;
        self
    }

    pub fn with_cache_ttl(mut self, ttl_seconds: u64) -> Self {
        self.cache_ttl_seconds = ttl_seconds;
        self
    }

    pub fn with_ollama_url(mut self, url: String) -> Self {
        self.ollama_base_url = url;
        self
    }

    pub fn with_db_path(mut self, path: String) -> Self {
        self.db_path = path;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execution_mode_default() {
        assert_eq!(ExecutionMode::default(), ExecutionMode::LocalFirst);
    }

    #[test]
    fn test_execution_mode_as_str() {
        assert_eq!(ExecutionMode::LocalFirst.as_str(), "local_first");
        assert_eq!(ExecutionMode::CloudFirst.as_str(), "cloud_first");
    }

    #[test]
    fn test_execution_mode_from_str() {
        assert_eq!(
            ExecutionMode::from_str("local_first"),
            Some(ExecutionMode::LocalFirst)
        );
        assert_eq!(
            ExecutionMode::from_str("cloud_first"),
            Some(ExecutionMode::CloudFirst)
        );
        assert_eq!(ExecutionMode::from_str("invalid"), None);
    }

    #[test]
    fn test_local_model_entry_new() {
        let entry = LocalModelEntry::new(
            "llama3.2:latest".to_string(),
            ModelTier::Small,
            4096,
        );
        assert_eq!(entry.name, "llama3.2:latest");
        assert_eq!(entry.tier, ModelTier::Small);
        assert!(!entry.is_embedding_model);
    }

    #[test]
    fn test_local_model_entry_with_embedding() {
        let entry = LocalModelEntry::new(
            "nomic-embed-text".to_string(),
            ModelTier::Tiny,
            512,
        )
        .with_embedding(true);

        assert!(entry.is_embedding_model);
    }

    #[test]
    fn test_cloud_model_entry_new() {
        let entry = CloudModelEntry::new(
            CloudProvider::Anthropic {
                model: "claude-haiku-4-5".to_string(),
            },
            "claude-haiku-4-5".to_string(),
            0.00025,
            0.00125,
            200_000,
        );

        assert_eq!(entry.model_id, "claude-haiku-4-5");
        assert_eq!(entry.cost_per_1k_input, 0.00025);
    }

    #[test]
    fn test_model_registry_new() {
        let registry = ModelRegistry::new();
        assert!(registry.local.is_empty());
        assert!(registry.cloud.is_empty());
    }

    #[test]
    fn test_model_registry_add_local() {
        let mut registry = ModelRegistry::new();
        registry.add_local(LocalModelEntry::new(
            "llama3.2".to_string(),
            ModelTier::Small,
            4096,
        ));

        assert_eq!(registry.local.len(), 1);
    }

    #[test]
    fn test_model_registry_best_local_for_tier() {
        let mut registry = ModelRegistry::new();
        registry.add_local(LocalModelEntry::new(
            "llama3.2".to_string(),
            ModelTier::Small,
            4096,
        ));
        registry.add_local(LocalModelEntry::new(
            "qwen2.5:14b".to_string(),
            ModelTier::Medium,
            4096,
        ));

        let small = registry.best_local_for_tier(&ModelTier::Small);
        assert!(small.is_some());
        assert_eq!(small.unwrap().name, "llama3.2");
    }

    #[test]
    fn test_model_registry_embedding_model() {
        let mut registry = ModelRegistry::new();
        registry.add_local(
            LocalModelEntry::new("llama3.2".to_string(), ModelTier::Small, 4096)
                .with_embedding(false),
        );
        registry.add_local(
            LocalModelEntry::new("nomic-embed-text".to_string(), ModelTier::Tiny, 512)
                .with_embedding(true),
        );

        let embedding = registry.embedding_model();
        assert!(embedding.is_some());
        assert_eq!(embedding.unwrap().name, "nomic-embed-text");
    }

    #[test]
    fn test_orchestrator_config_default() {
        let config = OrchestratorConfig::default();
        assert_eq!(config.execution_mode, ExecutionMode::LocalFirst);
        assert_eq!(config.cloud_complexity_threshold, 0.7);
        assert_eq!(config.cache_ttl_seconds, 3600);
        assert!(config.auto_pull_missing_models);
    }

    #[test]
    fn test_orchestrator_config_builders() {
        let config = OrchestratorConfig::new()
            .with_execution_mode(ExecutionMode::CloudFirst)
            .with_cache_ttl(7200)
            .with_ollama_url("http://localhost:12345".to_string());

        assert_eq!(config.execution_mode, ExecutionMode::CloudFirst);
        assert_eq!(config.cache_ttl_seconds, 7200);
        assert_eq!(config.ollama_base_url, "http://localhost:12345");
    }
}

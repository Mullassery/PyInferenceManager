use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub mod anthropic_backend;
pub mod colibri_backend;
pub mod ollama_backend;
pub mod openai_backend;
pub mod stub_backend;

pub use anthropic_backend::AnthropicBackend;
pub use colibri_backend::ColibriBackend;
pub use ollama_backend::OllamaBackend;
pub use openai_backend::OpenAiBackend;
pub use stub_backend::StubBackend;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum BackendKind {
    Anthropic,
    OpenAi,
    Ollama,
    VLlm,
    TensorRtLlm,
    MlcLlm,
    Colibri,
}

impl BackendKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            BackendKind::Anthropic => "anthropic",
            BackendKind::OpenAi => "openai",
            BackendKind::Ollama => "ollama",
            BackendKind::VLlm => "vllm",
            BackendKind::TensorRtLlm => "tensorrt_llm",
            BackendKind::MlcLlm => "mlc_llm",
            BackendKind::Colibri => "colibri",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelProfile {
    pub size_bytes: u64,
    pub is_moe: bool,
    pub context_length: u32,
    pub architecture: String,
}

impl ModelProfile {
    pub fn new(size_bytes: u64, architecture: String) -> Self {
        ModelProfile {
            size_bytes,
            is_moe: false,
            context_length: 4096,
            architecture,
        }
    }

    pub fn with_moe(mut self, is_moe: bool) -> Self {
        self.is_moe = is_moe;
        self
    }

    pub fn with_context_length(mut self, context_length: u32) -> Self {
        self.context_length = context_length;
        self
    }

    pub fn size_gb(&self) -> f32 {
        self.size_bytes as f32 / 1_073_741_824.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceRequest {
    pub model: String,
    pub prompt: String,
    pub max_tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceResult {
    pub output: String,
    pub tokens_used: u32,
    pub latency_ms: u64,
}

#[async_trait::async_trait]
pub trait RuntimeBackend: Send + Sync {
    async fn infer(&self, request: InferenceRequest) -> crate::Result<InferenceResult>;

    fn estimate_cost(&self, profile: &ModelProfile) -> f32;

    fn estimate_latency(&self, profile: &ModelProfile) -> u64;

    fn kind(&self) -> BackendKind;
}

pub enum AnyBackend {
    Anthropic(AnthropicBackend),
    OpenAi(OpenAiBackend),
    Ollama(OllamaBackend),
    VLlm(StubBackend),
    TensorRtLlm(StubBackend),
    MlcLlm(StubBackend),
    Colibri(ColibriBackend),
}

impl AnyBackend {
    pub async fn infer(&self, request: InferenceRequest) -> crate::Result<InferenceResult> {
        match self {
            AnyBackend::Anthropic(b) => b.infer(request).await,
            AnyBackend::OpenAi(b) => b.infer(request).await,
            AnyBackend::Ollama(b) => b.infer(request).await,
            AnyBackend::VLlm(b) => b.infer(request).await,
            AnyBackend::TensorRtLlm(b) => b.infer(request).await,
            AnyBackend::MlcLlm(b) => b.infer(request).await,
            AnyBackend::Colibri(b) => b.infer(request).await,
        }
    }

    pub fn estimate_cost(&self, profile: &ModelProfile) -> f32 {
        match self {
            AnyBackend::Anthropic(b) => b.estimate_cost(profile),
            AnyBackend::OpenAi(b) => b.estimate_cost(profile),
            AnyBackend::Ollama(b) => b.estimate_cost(profile),
            AnyBackend::VLlm(b) => b.estimate_cost(profile),
            AnyBackend::TensorRtLlm(b) => b.estimate_cost(profile),
            AnyBackend::MlcLlm(b) => b.estimate_cost(profile),
            AnyBackend::Colibri(b) => b.estimate_cost(profile),
        }
    }

    pub fn estimate_latency(&self, profile: &ModelProfile) -> u64 {
        match self {
            AnyBackend::Anthropic(b) => b.estimate_latency(profile),
            AnyBackend::OpenAi(b) => b.estimate_latency(profile),
            AnyBackend::Ollama(b) => b.estimate_latency(profile),
            AnyBackend::VLlm(b) => b.estimate_latency(profile),
            AnyBackend::TensorRtLlm(b) => b.estimate_latency(profile),
            AnyBackend::MlcLlm(b) => b.estimate_latency(profile),
            AnyBackend::Colibri(b) => b.estimate_latency(profile),
        }
    }

    pub fn kind(&self) -> BackendKind {
        match self {
            AnyBackend::Anthropic(b) => b.kind(),
            AnyBackend::OpenAi(b) => b.kind(),
            AnyBackend::Ollama(b) => b.kind(),
            AnyBackend::VLlm(b) => b.kind(),
            AnyBackend::TensorRtLlm(b) => b.kind(),
            AnyBackend::MlcLlm(b) => b.kind(),
            AnyBackend::Colibri(b) => b.kind(),
        }
    }
}

pub struct BackendRegistry {
    backends: HashMap<BackendKind, AnyBackend>,
}

impl BackendRegistry {
    pub fn new() -> Self {
        BackendRegistry {
            backends: HashMap::new(),
        }
    }

    pub fn register(&mut self, kind: BackendKind, backend: AnyBackend) {
        self.backends.insert(kind, backend);
    }

    pub fn get(&self, kind: BackendKind) -> Option<&AnyBackend> {
        self.backends.get(&kind)
    }

    pub fn all(&self) -> Vec<BackendKind> {
        self.backends.keys().copied().collect()
    }
}

impl Default for BackendRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backend_kind_as_str() {
        assert_eq!(BackendKind::Anthropic.as_str(), "anthropic");
        assert_eq!(BackendKind::OpenAi.as_str(), "openai");
        assert_eq!(BackendKind::Ollama.as_str(), "ollama");
        assert_eq!(BackendKind::Colibri.as_str(), "colibri");
    }

    #[test]
    fn test_model_profile_new() {
        let profile = ModelProfile::new(7_000_000_000, "llama".to_string());
        assert_eq!(profile.size_bytes, 7_000_000_000);
        assert!(!profile.is_moe);
        assert_eq!(profile.context_length, 4096);
    }

    #[test]
    fn test_model_profile_size_gb() {
        let profile = ModelProfile::new(8 * 1_073_741_824, "llama".to_string());
        assert_eq!(profile.size_gb(), 8.0);
    }

    #[test]
    fn test_model_profile_builders() {
        let profile = ModelProfile::new(7_000_000_000, "mixtral".to_string())
            .with_moe(true)
            .with_context_length(8192);

        assert!(profile.is_moe);
        assert_eq!(profile.context_length, 8192);
    }

    #[test]
    fn test_backend_registry_new() {
        let registry = BackendRegistry::new();
        assert_eq!(registry.all().len(), 0);
    }
}

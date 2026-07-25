use super::{BackendKind, InferenceRequest, InferenceResult, ModelProfile, RuntimeBackend};
use crate::engines::OllamaClient;

pub struct OllamaBackend {
    base_url: String,
}

impl OllamaBackend {
    pub fn new(base_url: String) -> Self {
        OllamaBackend { base_url }
    }
}

#[async_trait::async_trait]
impl RuntimeBackend for OllamaBackend {
    async fn infer(&self, request: InferenceRequest) -> crate::Result<InferenceResult> {
        let start = std::time::Instant::now();
        let client = OllamaClient::new(&self.base_url);

        let response = client.generate(&request.model, &request.prompt).await?;

        let latency_ms = start.elapsed().as_millis() as u64;

        Ok(InferenceResult {
            output: response.response,
            tokens_used: response.eval_count,
            latency_ms,
        })
    }

    fn estimate_cost(&self, _profile: &ModelProfile) -> f32 {
        0.0
    }

    fn estimate_latency(&self, profile: &ModelProfile) -> u64 {
        let size_gb = profile.size_gb();
        let base = (size_gb * 50.0) as u64;
        base.max(500).min(60000)
    }

    fn kind(&self) -> BackendKind {
        BackendKind::Ollama
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ollama_backend_kind() {
        let backend = OllamaBackend::new("http://localhost:11434".to_string());
        assert_eq!(backend.kind(), BackendKind::Ollama);
    }

    #[test]
    fn test_ollama_estimate_cost_is_free() {
        let backend = OllamaBackend::new("http://localhost:11434".to_string());
        let profile = ModelProfile::new(7 * 1_073_741_824, "llama".to_string());
        let cost = backend.estimate_cost(&profile);
        assert_eq!(cost, 0.0);
    }

    #[test]
    fn test_ollama_estimate_latency() {
        let backend = OllamaBackend::new("http://localhost:11434".to_string());
        let small_profile = ModelProfile::new(7 * 1_073_741_824, "llama".to_string());
        let large_profile = ModelProfile::new(70 * 1_073_741_824, "llama".to_string());

        let small_latency = backend.estimate_latency(&small_profile);
        let large_latency = backend.estimate_latency(&large_profile);

        assert!(small_latency < large_latency);
        assert!(small_latency >= 500);
    }
}

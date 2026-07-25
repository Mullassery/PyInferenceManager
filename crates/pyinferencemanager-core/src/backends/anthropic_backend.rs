use super::{BackendKind, InferenceRequest, InferenceResult, ModelProfile, RuntimeBackend};
use crate::engines::CloudClient;

pub struct AnthropicBackend {
    api_key: String,
}

impl AnthropicBackend {
    pub fn new(api_key: String) -> Self {
        AnthropicBackend { api_key }
    }
}

#[async_trait::async_trait]
impl RuntimeBackend for AnthropicBackend {
    async fn infer(&self, request: InferenceRequest) -> crate::Result<InferenceResult> {
        let start = std::time::Instant::now();
        let client = CloudClient::new(self.api_key.clone(), request.model.clone());

        let response = client.complete(&request.prompt, request.max_tokens).await?;

        let latency_ms = start.elapsed().as_millis() as u64;

        Ok(InferenceResult {
            output: response.text,
            tokens_used: response.tokens_used,
            latency_ms,
        })
    }

    fn estimate_cost(&self, profile: &ModelProfile) -> f32 {
        let size_gb = profile.size_gb();
        if size_gb < 16.0 {
            0.0005
        } else if size_gb < 50.0 {
            0.001
        } else {
            0.003
        }
    }

    fn estimate_latency(&self, profile: &ModelProfile) -> u64 {
        let size_gb = profile.size_gb();
        let base = (size_gb * 10.0) as u64;
        base.max(100).min(5000)
    }

    fn kind(&self) -> BackendKind {
        BackendKind::Anthropic
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_anthropic_backend_kind() {
        let backend = AnthropicBackend::new("test-key".to_string());
        assert_eq!(backend.kind(), BackendKind::Anthropic);
    }

    #[test]
    fn test_anthropic_estimate_cost() {
        let backend = AnthropicBackend::new("test-key".to_string());
        let small_profile = ModelProfile::new(8 * 1_073_741_824, "llama".to_string());
        let medium_profile = ModelProfile::new(30 * 1_073_741_824, "llama".to_string());
        let large_profile = ModelProfile::new(70 * 1_073_741_824, "llama".to_string());

        let small_cost = backend.estimate_cost(&small_profile);
        let medium_cost = backend.estimate_cost(&medium_profile);
        let large_cost = backend.estimate_cost(&large_profile);

        assert!(small_cost < medium_cost);
        assert!(medium_cost < large_cost);
    }

    #[test]
    fn test_anthropic_estimate_latency() {
        let backend = AnthropicBackend::new("test-key".to_string());
        let profile = ModelProfile::new(16 * 1_073_741_824, "llama".to_string());
        let latency = backend.estimate_latency(&profile);

        assert!(latency >= 100);
        assert!(latency <= 5000);
    }
}

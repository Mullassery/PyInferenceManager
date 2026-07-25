use super::{BackendKind, InferenceRequest, InferenceResult, ModelProfile, RuntimeBackend};
use crate::engines::OpenAIClient;

pub struct OpenAiBackend {
    api_key: String,
}

impl OpenAiBackend {
    pub fn new(api_key: String) -> Self {
        OpenAiBackend { api_key }
    }
}

#[async_trait::async_trait]
impl RuntimeBackend for OpenAiBackend {
    async fn infer(&self, request: InferenceRequest) -> crate::Result<InferenceResult> {
        let start = std::time::Instant::now();
        let client = OpenAIClient::new(self.api_key.clone(), request.model.clone());

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
        if size_gb < 10.0 {
            0.00015
        } else if size_gb < 30.0 {
            0.0003
        } else {
            0.0006
        }
    }

    fn estimate_latency(&self, profile: &ModelProfile) -> u64 {
        let size_gb = profile.size_gb();
        let base = (size_gb * 8.0) as u64;
        base.max(80).min(4000)
    }

    fn kind(&self) -> BackendKind {
        BackendKind::OpenAi
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_openai_backend_kind() {
        let backend = OpenAiBackend::new("test-key".to_string());
        assert_eq!(backend.kind(), BackendKind::OpenAi);
    }

    #[test]
    fn test_openai_estimate_cost() {
        let backend = OpenAiBackend::new("test-key".to_string());
        let small_profile = ModelProfile::new(4 * 1_073_741_824, "gpt".to_string());
        let large_profile = ModelProfile::new(50 * 1_073_741_824, "gpt".to_string());

        let small_cost = backend.estimate_cost(&small_profile);
        let large_cost = backend.estimate_cost(&large_profile);

        assert!(small_cost < large_cost);
    }

    #[test]
    fn test_openai_estimate_latency() {
        let backend = OpenAiBackend::new("test-key".to_string());
        let profile = ModelProfile::new(8 * 1_073_741_824, "gpt".to_string());
        let latency = backend.estimate_latency(&profile);

        assert!(latency >= 80);
        assert!(latency <= 4000);
    }
}

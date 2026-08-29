use super::{BackendKind, InferenceRequest, InferenceResult, ModelProfile, RuntimeBackend};
use crate::engines::VLlmClient;

/// Real `RuntimeBackend` for a self-hosted vLLM server, talking to its
/// OpenAI-compatible HTTP API (`POST /v1/completions`). Follows the same
/// shape as `OllamaBackend`: a thin adapter that owns just enough config
/// (here, the base URL and optional API key) to construct a fresh client
/// per call and translate `InferenceRequest`/`InferenceResult` to and from
/// the wire format.
pub struct VLlmBackend {
    base_url: String,
    api_key: Option<String>,
}

impl VLlmBackend {
    pub fn new(base_url: String) -> Self {
        VLlmBackend {
            base_url,
            api_key: None,
        }
    }

    pub fn with_api_key(mut self, api_key: Option<String>) -> Self {
        self.api_key = api_key;
        self
    }
}

#[async_trait::async_trait]
impl RuntimeBackend for VLlmBackend {
    async fn infer(&self, request: InferenceRequest) -> crate::Result<InferenceResult> {
        let start = std::time::Instant::now();
        let client = VLlmClient::new(&self.base_url).with_api_key(self.api_key.clone());

        let response = client
            .complete(&request.model, &request.prompt, request.max_tokens)
            .await?;

        let latency_ms = start.elapsed().as_millis() as u64;

        Ok(InferenceResult {
            output: response.text,
            tokens_used: response.tokens_used,
            latency_ms,
        })
    }

    // vLLM is a self-hosted runtime like Ollama, not a metered cloud API --
    // there's no per-token bill from vLLM itself. What scales with model
    // size is the GPU/compute cost of running it, so this mirrors the
    // original stub's tiered estimate (kept identical to avoid silently
    // changing cost-comparison behavior elsewhere, e.g. CostEstimator)
    // rather than collapsing to a flat 0.0 like OllamaBackend, since vLLM is
    // typically deployed for larger models where that compute cost is
    // material.
    fn estimate_cost(&self, profile: &ModelProfile) -> f32 {
        let size_gb = profile.size_gb();
        if size_gb < 20.0 {
            0.0
        } else if size_gb < 50.0 {
            0.00001
        } else {
            0.00002
        }
    }

    fn estimate_latency(&self, profile: &ModelProfile) -> u64 {
        let size_gb = profile.size_gb();
        let base = (size_gb * 30.0) as u64;
        base.max(200).min(30000)
    }

    fn kind(&self) -> BackendKind {
        BackendKind::VLlm
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vllm_backend_kind() {
        let backend = VLlmBackend::new("http://localhost:8000".to_string());
        assert_eq!(backend.kind(), BackendKind::VLlm);
    }

    #[test]
    fn test_vllm_estimate_cost_tiers_by_model_size() {
        let backend = VLlmBackend::new("http://localhost:8000".to_string());
        let small = ModelProfile::new(10 * 1_073_741_824, "model".to_string());
        let medium = ModelProfile::new(30 * 1_073_741_824, "model".to_string());
        let large = ModelProfile::new(60 * 1_073_741_824, "model".to_string());

        assert_eq!(backend.estimate_cost(&small), 0.0);
        assert!(backend.estimate_cost(&medium) < backend.estimate_cost(&large));
    }

    #[test]
    fn test_vllm_estimate_latency_bounds() {
        let backend = VLlmBackend::new("http://localhost:8000".to_string());
        let small_profile = ModelProfile::new(7 * 1_073_741_824, "llama".to_string());
        let large_profile = ModelProfile::new(70 * 1_073_741_824, "llama".to_string());

        let small_latency = backend.estimate_latency(&small_profile);
        let large_latency = backend.estimate_latency(&large_profile);

        assert!(small_latency < large_latency);
        assert!(small_latency >= 200);
    }

    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn test_infer_real_http_dispatch_through_backend() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/v1/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "choices": [{"text": "Hello from vLLM", "finish_reason": "stop"}],
                "usage": {"prompt_tokens": 3, "completion_tokens": 4, "total_tokens": 7}
            })))
            .mount(&mock_server)
            .await;

        let backend = VLlmBackend::new(mock_server.uri());
        let request = InferenceRequest {
            model: "meta-llama/Llama-3-8b".to_string(),
            prompt: "Hi".to_string(),
            max_tokens: 50,
        };

        let result = backend.infer(request).await.expect("mocked infer should succeed");
        assert_eq!(result.output, "Hello from vLLM");
        assert_eq!(result.tokens_used, 4);
    }

    #[tokio::test]
    async fn test_infer_propagates_backend_unreachable_error() {
        // Nothing listening -- must return a real error, not silently
        // succeed or hang, since this is meant to replace the old stub that
        // always errored with "not configured".
        let backend = VLlmBackend::new("http://127.0.0.1:1".to_string());
        let request = InferenceRequest {
            model: "model".to_string(),
            prompt: "Hi".to_string(),
            max_tokens: 10,
        };

        let result = backend.infer(request).await;
        assert!(result.is_err());
    }
}

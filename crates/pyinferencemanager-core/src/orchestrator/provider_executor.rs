use crate::backends::{
    AnthropicBackend, AnyBackend, BackendKind, BackendRegistry, GeminiBackend, InferenceRequest,
    OpenAiBackend,
};
use crate::error_classifier::ErrorClassifier;
use crate::types::CloudProvider;
use crate::Result;

#[derive(Debug, Clone)]
pub struct ProviderExecutionRequest {
    pub provider: CloudProvider,
    pub prompt: String,
    pub max_tokens: u32,
}

#[derive(Debug, Clone)]
pub struct ProviderExecutionResult {
    pub output: String,
    pub tokens_used: u32,
    pub provider_name: String,
}

pub struct ProviderExecutor;

impl ProviderExecutor {
    /// Execute request on a specific provider.
    ///
    /// Dispatches through `crate::backends::BackendRegistry` /
    /// `RuntimeBackend` instead of hand-matching `CloudProvider`, so the only
    /// place that needs to know how to build a client for a given
    /// `BackendKind` is `register_backend` below -- adding a new cloud
    /// `CloudProvider` variant only requires a `CloudProvider::kind()` arm
    /// (`types/dag.rs`) and, once there's a real backend for it, a
    /// `register_backend` arm here.
    pub async fn execute(request: ProviderExecutionRequest) -> Result<ProviderExecutionResult> {
        let kind = request.provider.kind();
        let model = request.provider.model().to_string();

        let mut registry = BackendRegistry::new();
        Self::register_backend(&mut registry, kind)?;
        let backend = registry
            .get(kind)
            .expect("register_backend just registered this exact kind");

        let inference_request = InferenceRequest {
            model: model.clone(),
            prompt: request.prompt,
            max_tokens: request.max_tokens,
        };

        let result = backend.infer(inference_request).await?;

        Ok(ProviderExecutionResult {
            output: result.output,
            tokens_used: result.tokens_used,
            provider_name: format!("{}:{}", kind.as_str(), model),
        })
    }

    /// Register the `RuntimeBackend` for a given `BackendKind` into the
    /// registry, sourcing its API key from the same environment variables
    /// the old per-provider functions used. Non-exhaustive on purpose: kinds
    /// with no cloud client yet (Ollama, vLLM, TensorRT-LLM, MLC-LLM,
    /// Colibri) fall through to the `other` arm rather than forcing an edit
    /// here just to compile.
    fn register_backend(registry: &mut BackendRegistry, kind: BackendKind) -> Result<()> {
        match kind {
            BackendKind::Anthropic => {
                let api_key = std::env::var("ANTHROPIC_API_KEY").map_err(|_| {
                    crate::Error::CloudError("ANTHROPIC_API_KEY not set".to_string())
                })?;
                registry.register(kind, AnyBackend::Anthropic(AnthropicBackend::new(api_key)));
                Ok(())
            }
            BackendKind::OpenAi => {
                let api_key = std::env::var("OPENAI_API_KEY")
                    .map_err(|_| crate::Error::CloudError("OPENAI_API_KEY not set".to_string()))?;
                registry.register(kind, AnyBackend::OpenAi(OpenAiBackend::new(api_key)));
                Ok(())
            }
            BackendKind::Gemini => {
                let api_key = std::env::var("GEMINI_API_KEY")
                    .or_else(|_| std::env::var("GOOGLE_API_KEY"))
                    .map_err(|_| {
                        crate::Error::CloudError(
                            "GEMINI_API_KEY (or GOOGLE_API_KEY) not set".to_string(),
                        )
                    })?;
                registry.register(kind, AnyBackend::Gemini(GeminiBackend::new(api_key)));
                Ok(())
            }
            other => Err(crate::Error::CloudError(format!(
                "no cloud backend registered for {:?}",
                other
            ))),
        }
    }

    /// Check if error from provider execution is retryable
    pub fn is_error_retryable(error: &crate::Error) -> bool {
        match error {
            crate::Error::CloudError(msg) => {
                let status_code = ErrorClassifier::extract_status_code(msg);
                ErrorClassifier::classify(status_code, msg)
                    == crate::error_classifier::ErrorCategory::Retryable
            }
            _ => false,
        }
    }

    /// Extract provider name from error for logging
    pub fn extract_provider_from_error(_error: &crate::Error) -> Option<String> {
        // In a real implementation, would extract from error context
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_execution_request_creation() {
        let request = ProviderExecutionRequest {
            provider: CloudProvider::Anthropic {
                model: "claude-haiku-4-5".to_string(),
            },
            prompt: "Hello".to_string(),
            max_tokens: 100,
        };

        assert_eq!(request.max_tokens, 100);
    }

    #[test]
    fn test_provider_execution_result_creation() {
        let result = ProviderExecutionResult {
            output: "Response".to_string(),
            tokens_used: 50,
            provider_name: "anthropic:claude-haiku-4-5".to_string(),
        };

        assert_eq!(result.tokens_used, 50);
        assert_eq!(result.provider_name, "anthropic:claude-haiku-4-5");
    }

    #[test]
    fn test_is_error_retryable_cloud_error() {
        let error = crate::Error::CloudError("HTTP 429: Rate limit exceeded".to_string());
        assert!(ProviderExecutor::is_error_retryable(&error));

        let error = crate::Error::CloudError("HTTP 401: Unauthorized".to_string());
        assert!(!ProviderExecutor::is_error_retryable(&error));
    }

    #[test]
    fn test_is_error_retryable_non_cloud_error() {
        let error = crate::Error::CacheError("Some error".to_string());
        assert!(!ProviderExecutor::is_error_retryable(&error));
    }

    // These tests mutate process-wide *_API_KEY env vars, so they're
    // serialized against each other and against the engines::*_client
    // tests (which also mutate process-wide env vars) via serial_test's
    // shared default lock.
    use serial_test::serial;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    #[serial]
    fn test_register_backend_missing_anthropic_key_matches_original_message() {
        std::env::remove_var("ANTHROPIC_API_KEY");
        let mut registry = BackendRegistry::new();
        let err = match ProviderExecutor::register_backend(&mut registry, BackendKind::Anthropic) {
            Ok(_) => panic!("expected missing-key error"),
            Err(e) => e,
        };
        assert_eq!(err.to_string(), "Cloud error: ANTHROPIC_API_KEY not set");
    }

    #[test]
    #[serial]
    fn test_register_backend_missing_openai_key_matches_original_message() {
        std::env::remove_var("OPENAI_API_KEY");
        let mut registry = BackendRegistry::new();
        let err = match ProviderExecutor::register_backend(&mut registry, BackendKind::OpenAi) {
            Ok(_) => panic!("expected missing-key error"),
            Err(e) => e,
        };
        assert_eq!(err.to_string(), "Cloud error: OPENAI_API_KEY not set");
    }

    #[test]
    #[serial]
    fn test_register_backend_missing_gemini_key_matches_original_message() {
        std::env::remove_var("GEMINI_API_KEY");
        std::env::remove_var("GOOGLE_API_KEY");
        let mut registry = BackendRegistry::new();
        let err = match ProviderExecutor::register_backend(&mut registry, BackendKind::Gemini) {
            Ok(_) => panic!("expected missing-key error"),
            Err(e) => e,
        };
        assert_eq!(
            err.to_string(),
            "Cloud error: GEMINI_API_KEY (or GOOGLE_API_KEY) not set"
        );
    }

    #[test]
    #[serial]
    fn test_register_backend_gemini_falls_back_to_google_api_key() {
        std::env::remove_var("GEMINI_API_KEY");
        std::env::set_var("GOOGLE_API_KEY", "test-key");
        let mut registry = BackendRegistry::new();
        let result = ProviderExecutor::register_backend(&mut registry, BackendKind::Gemini);
        std::env::remove_var("GOOGLE_API_KEY");
        assert!(result.is_ok());
        assert!(registry.get(BackendKind::Gemini).is_some());
    }

    #[test]
    fn test_register_backend_unsupported_kind_returns_descriptive_error() {
        let mut registry = BackendRegistry::new();
        let err = match ProviderExecutor::register_backend(&mut registry, BackendKind::Ollama) {
            Ok(_) => panic!("expected unsupported-kind error"),
            Err(e) => e,
        };
        assert!(err.to_string().contains("no cloud backend registered"));
    }

    #[tokio::test]
    #[serial]
    async fn test_execute_dispatches_via_registry_to_real_anthropic_backend() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "content": [{"type": "text", "text": "Hello from the registry-backed executor"}],
                "usage": {"input_tokens": 5, "output_tokens": 6},
                "stop_reason": "end_turn"
            })))
            .mount(&mock_server)
            .await;

        std::env::set_var("ANTHROPIC_API_KEY", "test-key");
        std::env::set_var("ANTHROPIC_BASE_URL", mock_server.uri());

        let request = ProviderExecutionRequest {
            provider: CloudProvider::Anthropic {
                model: "claude-haiku-4-5".to_string(),
            },
            prompt: "Hi".to_string(),
            max_tokens: 50,
        };
        let result = ProviderExecutor::execute(request).await;

        std::env::remove_var("ANTHROPIC_API_KEY");
        std::env::remove_var("ANTHROPIC_BASE_URL");

        let result = result.expect("registry-dispatched request should succeed");
        assert_eq!(result.output, "Hello from the registry-backed executor");
        assert_eq!(result.tokens_used, 6);
        assert_eq!(result.provider_name, "anthropic:claude-haiku-4-5");
    }
}

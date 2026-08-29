use serde::{Deserialize, Serialize};

/// vLLM's OpenAI-compatible `/v1/completions` response shape. Only the
/// fields this client actually consumes are modeled -- vLLM (like the real
/// OpenAI API it mirrors) includes several more (`id`, `object`, `created`,
/// `model`, per-choice `logprobs`, etc.) that callers here don't need.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionChoice {
    pub text: String,
    #[serde(default)]
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionUsage {
    #[serde(default)]
    pub prompt_tokens: u32,
    #[serde(default)]
    pub completion_tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionResponse {
    pub choices: Vec<CompletionChoice>,
    #[serde(default)]
    pub usage: Option<CompletionUsage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelEntry {
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelsResponse {
    #[serde(default)]
    pub data: Vec<ModelEntry>,
}

/// Result of a completed vLLM `/v1/completions` call.
#[derive(Debug, Clone)]
pub struct CompletionResult {
    pub text: String,
    pub tokens_used: u32,
    pub finish_reason: String,
}

/// HTTP client for a vLLM server's OpenAI-compatible API
/// (https://docs.vllm.ai/en/latest/serving/openai_compatible_server.html).
/// vLLM is typically self-hosted (like Ollama) rather than a metered cloud
/// API, so unlike `CloudClient`/`OpenAIClient` an API key is optional --
/// only sent as a `Bearer` header when one is configured, matching how
/// `vllm serve --api-key ...` gates access.
pub struct VLlmClient {
    base_url: String,
    api_key: Option<String>,
    // Reused across every request instead of constructing a fresh
    // reqwest::Client (and paying a new connection-pool/TLS setup cost) per
    // call -- see the identical fix in cloud_client.rs/openai_client.rs.
    client: reqwest::Client,
}

impl VLlmClient {
    pub fn new(base_url: &str) -> Self {
        VLlmClient {
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key: None,
            // Mirrors OllamaClient's split timeouts: fail fast if nothing is
            // even listening on the port, but allow a generous window for
            // real generation against a reachable server.
            client: reqwest::Client::builder()
                .connect_timeout(std::time::Duration::from_secs(3))
                .timeout(std::time::Duration::from_secs(120))
                .build()
                .unwrap_or_default(),
        }
    }

    pub fn with_api_key(mut self, api_key: Option<String>) -> Self {
        self.api_key = api_key;
        self
    }

    /// Default localhost:8000 endpoint (vLLM's default `vllm serve` port),
    /// overridable via VLLM_BASE_URL; optional VLLM_API_KEY for deployments
    /// started with `--api-key`.
    pub fn with_defaults() -> Self {
        let base_url = std::env::var("VLLM_BASE_URL")
            .unwrap_or_else(|_| "http://localhost:8000".to_string());
        Self::new(&base_url).with_api_key(std::env::var("VLLM_API_KEY").ok())
    }

    fn authorize(&self, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        match &self.api_key {
            Some(key) if !key.is_empty() => builder.header("Authorization", format!("Bearer {}", key)),
            _ => builder,
        }
    }

    /// Health/reachability probe against `/v1/models` -- doesn't require an
    /// actual model to be loaded, just that the server is up and speaking
    /// the OpenAI-compatible API.
    pub async fn is_available(&self) -> bool {
        let url = format!("{}/v1/models", self.base_url);
        self.authorize(self.client.get(&url))
            .send()
            .await
            .map(|resp| resp.status().is_success())
            .unwrap_or(false)
    }

    pub async fn list_models(&self) -> crate::Result<Vec<String>> {
        let url = format!("{}/v1/models", self.base_url);

        let response = self
            .authorize(self.client.get(&url))
            .send()
            .await
            .map_err(|e| crate::Error::VLlmError(format!("Request failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(crate::Error::VLlmError(format!(
                "HTTP {}: {}",
                response.status(),
                response.text().await.unwrap_or_default()
            )));
        }

        let result = response
            .json::<ModelsResponse>()
            .await
            .map_err(|e| crate::Error::VLlmError(format!("JSON parse error: {}", e)))?;

        Ok(result.data.into_iter().map(|m| m.id).collect())
    }

    /// Real inference call against vLLM's OpenAI-compatible text-completion
    /// endpoint (`POST /v1/completions`). `model` is the model name vLLM
    /// was launched with (`vllm serve <model>`).
    pub async fn complete(
        &self,
        model: &str,
        prompt: &str,
        max_tokens: u32,
    ) -> crate::Result<CompletionResult> {
        let url = format!("{}/v1/completions", self.base_url);

        let body = serde_json::json!({
            "model": model,
            "prompt": prompt,
            "max_tokens": max_tokens,
        });

        let response = self
            .authorize(self.client.post(&url).json(&body))
            .send()
            .await
            .map_err(|e| crate::Error::VLlmError(format!("Request failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(crate::Error::VLlmError(format!(
                "HTTP {}: {}",
                response.status(),
                response.text().await.unwrap_or_default()
            )));
        }

        let parsed = response
            .json::<CompletionResponse>()
            .await
            .map_err(|e| crate::Error::VLlmError(format!("JSON parse error: {}", e)))?;

        let choice = parsed
            .choices
            .into_iter()
            .next()
            .ok_or_else(|| crate::Error::VLlmError("Missing completion choice".to_string()))?;

        let tokens_used = parsed.usage.map(|u| u.completion_tokens).unwrap_or(0);

        Ok(CompletionResult {
            text: choice.text,
            tokens_used,
            finish_reason: choice.finish_reason.unwrap_or_else(|| "unknown".to_string()),
        })
    }
}

impl Default for VLlmClient {
    fn default() -> Self {
        Self::with_defaults()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vllm_client_new_defaults_no_api_key() {
        let client = VLlmClient::new("http://localhost:8000");
        assert_eq!(client.base_url, "http://localhost:8000");
        assert!(client.api_key.is_none());
    }

    #[test]
    fn test_vllm_client_new_trims_trailing_slash() {
        let client = VLlmClient::new("http://localhost:8000/");
        assert_eq!(client.base_url, "http://localhost:8000");
    }

    #[test]
    fn test_vllm_client_with_api_key() {
        let client = VLlmClient::new("http://localhost:8000").with_api_key(Some("secret".to_string()));
        assert_eq!(client.api_key, Some("secret".to_string()));
    }

    #[test]
    fn test_completion_response_deserialization() {
        let json = r#"{
            "id": "cmpl-1",
            "object": "text_completion",
            "created": 1,
            "model": "meta-llama/Llama-3-8b",
            "choices": [{"text": "Paris.", "index": 0, "finish_reason": "stop"}],
            "usage": {"prompt_tokens": 5, "completion_tokens": 3, "total_tokens": 8}
        }"#;

        let parsed: CompletionResponse =
            serde_json::from_str(json).expect("must deserialize a real vLLM completions response");
        assert_eq!(parsed.choices[0].text, "Paris.");
        assert_eq!(parsed.usage.unwrap().completion_tokens, 3);
    }

    use serial_test::serial;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn test_complete_real_http_against_vllm_shaped_mock() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/v1/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "cmpl-123",
                "object": "text_completion",
                "created": 1710000000,
                "model": "meta-llama/Llama-3-8b",
                "choices": [{"text": " Paris.", "index": 0, "finish_reason": "stop"}],
                "usage": {"prompt_tokens": 6, "completion_tokens": 2, "total_tokens": 8}
            })))
            .mount(&mock_server)
            .await;

        let client = VLlmClient::new(&mock_server.uri());
        let result = client
            .complete("meta-llama/Llama-3-8b", "What is the capital of France?", 50)
            .await
            .expect("mocked vLLM completion should succeed");

        assert_eq!(result.text, " Paris.");
        assert_eq!(result.tokens_used, 2);
        assert_eq!(result.finish_reason, "stop");
    }

    #[tokio::test]
    async fn test_complete_propagates_error_status() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/v1/completions"))
            .respond_with(ResponseTemplate::new(500).set_body_string("model not loaded"))
            .mount(&mock_server)
            .await;

        let client = VLlmClient::new(&mock_server.uri());
        let err = client
            .complete("meta-llama/Llama-3-8b", "Hi", 50)
            .await
            .expect_err("500 should surface as an error");

        assert!(err.to_string().contains("500"));
    }

    #[tokio::test]
    #[serial]
    async fn test_complete_sends_bearer_auth_header_when_api_key_configured() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/v1/completions"))
            .and(header("Authorization", "Bearer test-vllm-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "choices": [{"text": "ok", "finish_reason": "stop"}],
                "usage": {"prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2}
            })))
            .mount(&mock_server)
            .await;

        let client = VLlmClient::new(&mock_server.uri()).with_api_key(Some("test-vllm-key".to_string()));
        let result = client.complete("model", "Hi", 10).await;

        assert!(result.is_ok(), "request with matching auth header should succeed");
    }

    #[tokio::test]
    async fn test_list_models_real_http_against_vllm_shaped_mock() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/v1/models"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "object": "list",
                "data": [{"id": "meta-llama/Llama-3-8b", "object": "model"}]
            })))
            .mount(&mock_server)
            .await;

        let client = VLlmClient::new(&mock_server.uri());
        let models = client.list_models().await.expect("mocked list_models should succeed");

        assert_eq!(models, vec!["meta-llama/Llama-3-8b".to_string()]);
    }

    #[tokio::test]
    async fn test_is_available_true_when_server_responds_success() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/v1/models"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"data": []})))
            .mount(&mock_server)
            .await;

        let client = VLlmClient::new(&mock_server.uri());
        assert!(client.is_available().await);
    }

    #[tokio::test]
    async fn test_is_available_false_when_nothing_listening() {
        // Nothing bound to this port -- connection should fail fast rather
        // than hang, and is_available() must report false, not panic.
        let client = VLlmClient::new("http://127.0.0.1:1");
        assert!(!client.is_available().await);
    }
}

use serde::{Deserialize, Serialize};

use crate::engines::openai_client::OpenAIResponse;

pub struct OpenRouterClient {
    api_key: String,
    model: String,
    client: reqwest::Client,
}

impl OpenRouterClient {
    pub fn new(api_key: String, model: String) -> Self {
        OpenRouterClient {
            api_key,
            model,
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(60))
                .build()
                .unwrap_or_default(),
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(
            std::env::var("OPENROUTER_API_KEY").unwrap_or_default(),
            "openrouter/auto".to_string(),
        )
    }

    /// Base URL for the OpenRouter OpenAI-compatible API. Overridable via
    /// OPENROUTER_BASE_URL — defaults to the public OpenRouter endpoint.
    fn base_url() -> String {
        std::env::var("OPENROUTER_BASE_URL")
            .unwrap_or_else(|_| "https://openrouter.ai/api/v1".to_string())
    }

    pub async fn complete(&self, prompt: &str, max_tokens: u32) -> crate::Result<OpenAIResponse> {
        if self.api_key.is_empty() {
            return Err(crate::Error::CloudError(
                "OPENROUTER_API_KEY not set".to_string(),
            ));
        }

        self.complete_via_http(prompt, max_tokens).await
    }

    async fn complete_via_http(
        &self,
        prompt: &str,
        max_tokens: u32,
    ) -> crate::Result<OpenAIResponse> {
        let url = format!("{}/chat/completions", Self::base_url());

        let request_body = serde_json::json!({
            "model": self.model,
            "max_tokens": max_tokens,
            "messages": [
                {
                    "role": "user",
                    "content": prompt
                }
            ]
        });

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header(
                "HTTP-Referer",
                "https://github.com/Mullassery/PyInferenceManager",
            )
            .header("X-Title", "PyInferenceManager")
            .header("content-type", "application/json")
            .json(&request_body)
            .send()
            .await
            .map_err(|e| crate::Error::CloudError(format!("HTTP request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(crate::Error::CloudError(format!(
                "HTTP {}: {}",
                status, text
            )));
        }

        let json = response
            .json::<serde_json::Value>()
            .await
            .map_err(|e| crate::Error::CloudError(format!("JSON parse error: {}", e)))?;

        let content = json
            .get("choices")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("message"))
            .and_then(|m| m.get("content"))
            .and_then(|t| t.as_str())
            .ok_or_else(|| crate::Error::CloudError("Missing response text".to_string()))?;

        let tokens_used = json
            .get("usage")
            .and_then(|u| u.get("completion_tokens"))
            .and_then(|t| t.as_u64())
            .unwrap_or(0) as u32;

        let finish_reason = json
            .get("choices")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("finish_reason"))
            .and_then(|s| s.as_str())
            .unwrap_or("unknown")
            .to_string();

        Ok(OpenAIResponse {
            text: content.to_string(),
            tokens_used,
            finish_reason,
        })
    }
}

impl Default for OpenRouterClient {
    fn default() -> Self {
        Self::with_defaults()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_openrouter_client_new() {
        let client = OpenRouterClient::new("key123".to_string(), "openrouter/auto".to_string());
        assert_eq!(client.api_key, "key123");
        assert_eq!(client.model, "openrouter/auto");
    }

    #[test]
    fn test_openrouter_base_url_default() {
        assert_eq!(OpenRouterClient::base_url(), "https://openrouter.ai/api/v1");
    }

    #[tokio::test]
    async fn test_complete_without_api_key() {
        let client = OpenRouterClient::new("".to_string(), "openrouter/auto".to_string());
        let result = client.complete("test", 100).await;
        assert!(result.is_err());
    }

    use serial_test::serial;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    #[serial]
    async fn test_complete_via_http_real_request_response_shape() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .and(header("Authorization", "Bearer test-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "choices": [{
                    "message": {"role": "assistant", "content": "Hello from OpenRouter"},
                    "finish_reason": "stop"
                }],
                "usage": {"prompt_tokens": 5, "completion_tokens": 4, "total_tokens": 9}
            })))
            .mount(&mock_server)
            .await;

        std::env::set_var("OPENROUTER_BASE_URL", mock_server.uri());
        let client = OpenRouterClient::new("test-key".to_string(), "openrouter/auto".to_string());
        let result = client.complete("Hi", 50).await;
        std::env::remove_var("OPENROUTER_BASE_URL");

        let response = result.expect("mocked request should succeed");
        assert_eq!(response.text, "Hello from OpenRouter");
        assert_eq!(response.tokens_used, 4);
        assert_eq!(response.finish_reason, "stop");
    }
}

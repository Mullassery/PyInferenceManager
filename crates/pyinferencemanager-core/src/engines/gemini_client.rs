use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiResponse {
    pub text: String,
    pub tokens_used: u32,
    pub finish_reason: String,
}

pub struct GeminiClient {
    api_key: String,
    model: String,
    // One reqwest::Client reused for every request from this instance —
    // see the identical fix in cloud_client.rs / openai_client.rs.
    client: reqwest::Client,
}

impl GeminiClient {
    pub fn new(api_key: String, model: String) -> Self {
        GeminiClient {
            api_key,
            model,
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(60))
                .build()
                .unwrap_or_default(),
        }
    }

    pub fn with_defaults() -> Self {
        let api_key = std::env::var("GEMINI_API_KEY")
            .or_else(|_| std::env::var("GOOGLE_API_KEY"))
            .unwrap_or_default();
        Self::new(api_key, "gemini-1.5-flash".to_string())
    }

    /// Base URL for the Gemini generateContent API. Overridable via
    /// GEMINI_BASE_URL (e.g. a local mock server in tests) — defaults to
    /// the real public API.
    fn base_url() -> String {
        std::env::var("GEMINI_BASE_URL")
            .unwrap_or_else(|_| "https://generativelanguage.googleapis.com".to_string())
    }

    pub async fn complete(&self, prompt: &str, max_tokens: u32) -> crate::Result<GeminiResponse> {
        if self.api_key.is_empty() {
            return Err(crate::Error::CloudError(
                "GEMINI_API_KEY (or GOOGLE_API_KEY) not set".to_string(),
            ));
        }

        self.complete_via_http(prompt, max_tokens).await
    }

    async fn complete_via_http(
        &self,
        prompt: &str,
        max_tokens: u32,
    ) -> crate::Result<GeminiResponse> {
        let url = format!(
            "{}/v1beta/models/{}:generateContent?key={}",
            Self::base_url(),
            self.model,
            self.api_key
        );

        let request_body = serde_json::json!({
            "contents": [
                {
                    "role": "user",
                    "parts": [{ "text": prompt }]
                }
            ],
            "generationConfig": {
                "maxOutputTokens": max_tokens
            }
        });

        let response = self
            .client
            .post(&url)
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

        Self::parse_response(&json)
    }

    /// Extracted from complete_via_http so the real Gemini response shape
    /// can be exercised with a static JSON fixture in tests without needing
    /// a live network call.
    fn parse_response(json: &serde_json::Value) -> crate::Result<GeminiResponse> {
        let text = json
            .get("candidates")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("content"))
            .and_then(|c| c.get("parts"))
            .and_then(|p| p.get(0))
            .and_then(|p| p.get("text"))
            .and_then(|t| t.as_str())
            .ok_or_else(|| crate::Error::CloudError("Missing response text".to_string()))?;

        let tokens_used = json
            .get("usageMetadata")
            .and_then(|u| u.get("candidatesTokenCount"))
            .and_then(|t| t.as_u64())
            .unwrap_or(0) as u32;

        let finish_reason = json
            .get("candidates")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("finishReason"))
            .and_then(|s| s.as_str())
            .unwrap_or("unknown")
            .to_string();

        Ok(GeminiResponse {
            text: text.to_string(),
            tokens_used,
            finish_reason,
        })
    }
}

impl Default for GeminiClient {
    fn default() -> Self {
        Self::with_defaults()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gemini_client_new() {
        let client = GeminiClient::new("key123".to_string(), "gemini-1.5-flash".to_string());
        assert_eq!(client.api_key, "key123");
        assert_eq!(client.model, "gemini-1.5-flash");
    }

    #[test]
    fn test_gemini_response_serialization() {
        let resp = GeminiResponse {
            text: "Hello".to_string(),
            tokens_used: 12,
            finish_reason: "STOP".to_string(),
        };

        let json = serde_json::to_string(&resp).unwrap();
        let deserialized: GeminiResponse = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.text, "Hello");
        assert_eq!(deserialized.tokens_used, 12);
    }

    #[tokio::test]
    async fn test_complete_without_api_key() {
        let client = GeminiClient::new("".to_string(), "gemini-1.5-flash".to_string());
        let result = client.complete("test", 100).await;
        assert!(result.is_err());
    }

    /// Real request/response handling, exercised against a realistic Gemini
    /// API response payload (per Google's documented generateContent
    /// response shape) without needing a live key or network call.
    #[test]
    fn test_parse_response_real_shape() {
        let body = serde_json::json!({
            "candidates": [
                {
                    "content": {
                        "parts": [{ "text": "Paris is the capital of France." }],
                        "role": "model"
                    },
                    "finishReason": "STOP",
                    "index": 0
                }
            ],
            "usageMetadata": {
                "promptTokenCount": 8,
                "candidatesTokenCount": 7,
                "totalTokenCount": 15
            }
        });

        let parsed = GeminiClient::parse_response(&body).unwrap();
        assert_eq!(parsed.text, "Paris is the capital of France.");
        assert_eq!(parsed.tokens_used, 7);
        assert_eq!(parsed.finish_reason, "STOP");
    }

    #[test]
    fn test_parse_response_missing_text_errors() {
        let body = serde_json::json!({ "candidates": [] });
        let parsed = GeminiClient::parse_response(&body);
        assert!(parsed.is_err());
    }

    // Mutates the process-wide GEMINI_BASE_URL env var — serialized
    // relative to itself and any other test doing the same (there's only
    // one here, but kept consistent with cloud_client.rs / openai_client.rs).
    use serial_test::serial;
    use wiremock::matchers::{method, path_regex};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    #[serial]
    async fn test_complete_via_http_real_request_response_shape() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path_regex(r"^/v1beta/models/.*:generateContent$"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "candidates": [{
                    "content": {"parts": [{"text": "Hello from Gemini"}], "role": "model"},
                    "finishReason": "STOP"
                }],
                "usageMetadata": {"promptTokenCount": 5, "candidatesTokenCount": 3, "totalTokenCount": 8}
            })))
            .mount(&mock_server)
            .await;

        std::env::set_var("GEMINI_BASE_URL", mock_server.uri());
        let client = GeminiClient::new("test-key".to_string(), "gemini-1.5-flash".to_string());
        let result = client.complete("Hi", 50).await;
        std::env::remove_var("GEMINI_BASE_URL");

        let response = result.expect("mocked request should succeed");
        assert_eq!(response.text, "Hello from Gemini");
        assert_eq!(response.tokens_used, 3);
        assert_eq!(response.finish_reason, "STOP");
    }
}

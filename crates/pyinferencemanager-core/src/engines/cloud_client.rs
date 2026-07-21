use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudResponse {
    pub text: String,
    pub tokens_used: u32,
    pub stop_reason: String,
}

pub struct CloudClient {
    api_key: String,
    model: String,
}

impl CloudClient {
    pub fn new(api_key: String, model: String) -> Self {
        CloudClient { api_key, model }
    }

    pub fn with_defaults() -> Self {
        CloudClient {
            api_key: std::env::var("ANTHROPIC_API_KEY").unwrap_or_default(),
            model: "claude-haiku-4-5".to_string(),
        }
    }

    pub async fn complete(&self, prompt: &str, max_tokens: u32) -> crate::Result<CloudResponse> {
        if self.api_key.is_empty() {
            return Err(crate::Error::CloudError(
                "ANTHROPIC_API_KEY not set".to_string(),
            ));
        }

        self.complete_via_http(prompt, max_tokens).await
    }

    async fn complete_via_http(&self, prompt: &str, max_tokens: u32) -> crate::Result<CloudResponse> {
        let url = "https://api.anthropic.com/v1/messages";

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

        let response = reqwest::Client::new()
            .post(url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
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
            .get("content")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("text"))
            .and_then(|t| t.as_str())
            .ok_or_else(|| crate::Error::CloudError("Missing response text".to_string()))?;

        let output_tokens = json
            .get("usage")
            .and_then(|u| u.get("output_tokens"))
            .and_then(|t| t.as_u64())
            .unwrap_or(0) as u32;

        let stop_reason = json
            .get("stop_reason")
            .and_then(|s| s.as_str())
            .unwrap_or("unknown")
            .to_string();

        Ok(CloudResponse {
            text: content.to_string(),
            tokens_used: output_tokens,
            stop_reason,
        })
    }
}

impl Default for CloudClient {
    fn default() -> Self {
        Self::with_defaults()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cloud_client_new() {
        let client = CloudClient::new("key123".to_string(), "claude-haiku-4-5".to_string());
        assert_eq!(client.api_key, "key123");
        assert_eq!(client.model, "claude-haiku-4-5");
    }

    #[test]
    fn test_cloud_client_with_defaults() {
        let _client = CloudClient::with_defaults();
    }

    #[test]
    fn test_cloud_response_serialization() {
        let resp = CloudResponse {
            text: "Hello".to_string(),
            tokens_used: 10,
            stop_reason: "end_turn".to_string(),
        };

        let json = serde_json::to_string(&resp).unwrap();
        let deserialized: CloudResponse = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.text, "Hello");
        assert_eq!(deserialized.tokens_used, 10);
    }

    #[tokio::test]
    async fn test_complete_without_api_key() {
        let client = CloudClient::new("".to_string(), "claude-haiku-4-5".to_string());
        let result = client.complete("test", 100).await;
        assert!(result.is_err());
    }
}

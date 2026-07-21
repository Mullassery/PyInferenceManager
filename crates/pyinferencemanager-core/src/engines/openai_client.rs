use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIResponse {
    pub text: String,
    pub tokens_used: u32,
    pub finish_reason: String,
}

pub struct OpenAIClient {
    api_key: String,
    model: String,
}

impl OpenAIClient {
    pub fn new(api_key: String, model: String) -> Self {
        OpenAIClient { api_key, model }
    }

    pub fn with_defaults() -> Self {
        OpenAIClient {
            api_key: std::env::var("OPENAI_API_KEY").unwrap_or_default(),
            model: "gpt-4o-mini".to_string(),
        }
    }

    pub async fn complete(&self, prompt: &str, max_tokens: u32) -> crate::Result<OpenAIResponse> {
        if self.api_key.is_empty() {
            return Err(crate::Error::CloudError(
                "OPENAI_API_KEY not set".to_string(),
            ));
        }

        self.complete_via_http(prompt, max_tokens).await
    }

    async fn complete_via_http(&self, prompt: &str, max_tokens: u32) -> crate::Result<OpenAIResponse> {
        let url = "https://api.openai.com/v1/chat/completions";

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
            .header("Authorization", format!("Bearer {}", self.api_key))
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

impl Default for OpenAIClient {
    fn default() -> Self {
        Self::with_defaults()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_openai_client_new() {
        let client = OpenAIClient::new("key123".to_string(), "gpt-4o-mini".to_string());
        assert_eq!(client.api_key, "key123");
        assert_eq!(client.model, "gpt-4o-mini");
    }

    #[test]
    fn test_openai_response_serialization() {
        let resp = OpenAIResponse {
            text: "Hello".to_string(),
            tokens_used: 20,
            finish_reason: "stop".to_string(),
        };

        let json = serde_json::to_string(&resp).unwrap();
        let deserialized: OpenAIResponse = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.text, "Hello");
        assert_eq!(deserialized.tokens_used, 20);
    }

    #[tokio::test]
    async fn test_complete_without_api_key() {
        let client = OpenAIClient::new("".to_string(), "gpt-4o-mini".to_string());
        let result = client.complete("test", 100).await;
        assert!(result.is_err());
    }
}

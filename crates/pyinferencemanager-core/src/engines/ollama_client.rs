use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateResponse {
    pub response: String,
    pub eval_count: u32,
    pub eval_duration_ns: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingResponse {
    pub embedding: Vec<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub name: String,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelsResponse {
    pub models: Vec<ModelInfo>,
}

pub struct OllamaClient {
    base_url: String,
    client: reqwest::Client,
}

impl OllamaClient {
    pub fn new(base_url: &str) -> Self {
        OllamaClient {
            base_url: base_url.to_string(),
            client: reqwest::Client::new(),
        }
    }

    pub async fn is_available(&self) -> bool {
        let url = format!("{}/api/tags", self.base_url);
        self.client
            .get(&url)
            .send()
            .await
            .map(|resp| resp.status().is_success())
            .unwrap_or(false)
    }

    pub async fn generate(&self, model: &str, prompt: &str) -> crate::Result<GenerateResponse> {
        let url = format!("{}/api/generate", self.base_url);

        let body = serde_json::json!({
            "model": model,
            "prompt": prompt,
            "stream": false
        });

        let response = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| crate::Error::OllamaError(format!("Request failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(crate::Error::OllamaError(format!(
                "HTTP {}: {}",
                response.status(),
                response.text().await.unwrap_or_default()
            )));
        }

        let result = response
            .json::<GenerateResponse>()
            .await
            .map_err(|e| crate::Error::OllamaError(format!("JSON parse error: {}", e)))?;

        Ok(result)
    }

    pub async fn embed(&self, model: &str, text: &str) -> crate::Result<Vec<f32>> {
        let url = format!("{}/api/embeddings", self.base_url);

        let body = serde_json::json!({
            "model": model,
            "prompt": text
        });

        let response = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| crate::Error::OllamaError(format!("Request failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(crate::Error::OllamaError(format!(
                "HTTP {}: {}",
                response.status(),
                response.text().await.unwrap_or_default()
            )));
        }

        let result = response
            .json::<EmbeddingResponse>()
            .await
            .map_err(|e| crate::Error::OllamaError(format!("JSON parse error: {}", e)))?;

        Ok(result.embedding)
    }

    pub async fn list_models(&self) -> crate::Result<Vec<ModelInfo>> {
        let url = format!("{}/api/tags", self.base_url);

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| crate::Error::OllamaError(format!("Request failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(crate::Error::OllamaError(format!(
                "HTTP {}: {}",
                response.status(),
                response.text().await.unwrap_or_default()
            )));
        }

        let result = response
            .json::<ModelsResponse>()
            .await
            .map_err(|e| crate::Error::OllamaError(format!("JSON parse error: {}", e)))?;

        Ok(result.models)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ollama_client_new() {
        let client = OllamaClient::new("http://localhost:11434");
        assert_eq!(client.base_url, "http://localhost:11434");
    }

    #[test]
    fn test_generate_response_serialization() {
        let resp = GenerateResponse {
            response: "test".to_string(),
            eval_count: 42,
            eval_duration_ns: 1000,
        };

        let json = serde_json::to_string(&resp).unwrap();
        let deserialized: GenerateResponse = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.response, "test");
        assert_eq!(deserialized.eval_count, 42);
    }

    #[test]
    fn test_model_info_serialization() {
        let model = ModelInfo {
            name: "llama3.2:latest".to_string(),
            size: 1_073_741_824,
        };

        let json = serde_json::to_string(&model).unwrap();
        let deserialized: ModelInfo = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.name, "llama3.2:latest");
    }
}

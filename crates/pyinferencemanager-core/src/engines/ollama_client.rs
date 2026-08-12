use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateResponse {
    pub response: String,
    // `#[serde(default)]` here because streaming/partial responses (or
    // Ollama versions that omit stats) may not include these — better to
    // deserialize with 0 than fail the whole response.
    #[serde(default)]
    pub eval_count: u32,
    // Real field name is `eval_duration` (nanoseconds) — this used to be
    // named `eval_duration_ns`, which doesn't exist in Ollama's actual
    // /api/generate response body. Since the field had no default, serde
    // rejected every real (non-fixture) response as a deserialization
    // error, silently breaking every live Ollama call while the
    // hand-written unit test fixture (which used the wrong name too)
    // stayed green. Found by actually exercising this against a running
    // local Ollama instance rather than only the unit test's fixture JSON.
    #[serde(default, rename = "eval_duration")]
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
            // reqwest::Client::new() has NO timeout at all by default, which
            // means is_available() — used as a health/hardware-probe check —
            // could hang indefinitely if Ollama isn't reachable (found via a
            // test suite run that hung for minutes with nothing listening on
            // localhost:11434). connect_timeout is short since "can't even
            // open a TCP connection" should fail fast; the overall request
            // timeout is left more generous since real generation calls
            // against a reachable Ollama can legitimately take a while.
            client: reqwest::Client::builder()
                .connect_timeout(std::time::Duration::from_secs(3))
                .timeout(std::time::Duration::from_secs(120))
                .build()
                .unwrap_or_default(),
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

    /// Regression test for a real bug: this struct used to declare a field
    /// named `eval_duration_ns`, but Ollama's actual /api/generate response
    /// (confirmed against a live local Ollama instance) calls it
    /// `eval_duration` — so every real, non-streaming generate() call
    /// failed to deserialize and errored out, even though
    /// test_generate_response_serialization above stayed green (it
    /// round-tripped the same wrong field name through both serialize and
    /// deserialize, so it never caught the mismatch against Ollama's real
    /// shape). This fixture is copied verbatim from a real
    /// `curl localhost:11434/api/generate -d '{"stream": false, ...}'`
    /// response.
    #[test]
    fn test_generate_response_parses_real_ollama_shape() {
        let real_ollama_json = r#"{
            "model": "qwen2.5:0.5b",
            "created_at": "2026-08-12T06:07:35.425359Z",
            "response": "The capital of France is Paris.",
            "done": true,
            "done_reason": "stop",
            "context": [1, 2, 3],
            "total_duration": 131312583,
            "load_duration": 83500625,
            "prompt_eval_count": 36,
            "prompt_eval_duration": 14343000,
            "eval_count": 8,
            "eval_duration": 32424000
        }"#;

        let parsed: GenerateResponse = serde_json::from_str(real_ollama_json)
            .expect("must deserialize a real Ollama /api/generate response");
        assert_eq!(parsed.response, "The capital of France is Paris.");
        assert_eq!(parsed.eval_count, 8);
        assert_eq!(parsed.eval_duration_ns, 32424000);
    }

    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn test_generate_real_http_against_ollama_shaped_mock() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/api/generate"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(
                r#"{"model":"qwen2.5:0.5b","response":"Paris.","done":true,"eval_count":3,"eval_duration":1500000}"#,
                "application/json",
            ))
            .mount(&mock_server)
            .await;

        let client = OllamaClient::new(&mock_server.uri());
        let response = client
            .generate("qwen2.5:0.5b", "What is the capital of France?")
            .await
            .expect("mocked generate() should succeed");

        assert_eq!(response.response, "Paris.");
        assert_eq!(response.eval_count, 3);
    }
}

use crate::engines::OllamaClient;
use crate::types::ModelTier;
use crate::Result;

pub struct OllamaProbe;

impl OllamaProbe {
    pub async fn probe_models(base_url: &str, recommended_tier: &ModelTier) -> Result<(Vec<String>, Option<String>, Option<String>)> {
        let client = OllamaClient::new(base_url);

        if !client.is_available().await {
            return Ok((Vec::new(), None, None));
        }

        let models = client.list_models().await.unwrap_or_default();
        let model_names: Vec<String> = models.iter().map(|m| m.name.clone()).collect();

        let best_model = Self::find_best_model(&model_names, recommended_tier);
        let embedding_model = Self::find_embedding_model(&model_names);

        Ok((model_names, best_model, embedding_model))
    }

    fn find_best_model(models: &[String], tier: &ModelTier) -> Option<String> {
        let tier_pattern = tier.default_model();

        models
            .iter()
            .find(|m| m.contains(tier_pattern) || Self::model_matches_tier(m, tier))
            .cloned()
            .or_else(|| {
                models.first().cloned()
            })
    }

    fn find_embedding_model(models: &[String]) -> Option<String> {
        let embedding_patterns = ["embed", "embedding", "nomic"];

        models
            .iter()
            .find(|m| {
                let lower = m.to_lowercase();
                embedding_patterns.iter().any(|p| lower.contains(p))
            })
            .cloned()
    }

    fn model_matches_tier(model: &str, tier: &ModelTier) -> bool {
        let lower = model.to_lowercase();
        match tier {
            ModelTier::Tiny => lower.contains("1.5b") || lower.contains("3b"),
            ModelTier::Small => lower.contains("7b"),
            ModelTier::Medium => lower.contains("14b"),
            ModelTier::Large => lower.contains("32b"),
            ModelTier::XLarge => lower.contains("70b"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_best_model() {
        let models = vec![
            "llama3.2:latest".to_string(),
            "qwen2.5:14b".to_string(),
            "qwen2.5:32b".to_string(),
        ];

        let result = OllamaProbe::find_best_model(&models, &ModelTier::Medium);
        assert_eq!(result, Some("qwen2.5:14b".to_string()));
    }

    #[test]
    fn test_find_embedding_model() {
        let models = vec![
            "llama3.2:latest".to_string(),
            "nomic-embed-text".to_string(),
            "qwen2.5:7b".to_string(),
        ];

        let result = OllamaProbe::find_embedding_model(&models);
        assert_eq!(result, Some("nomic-embed-text".to_string()));
    }

    #[test]
    fn test_find_embedding_model_with_embedding_keyword() {
        let models = vec![
            "my-embedding-model".to_string(),
            "llama3.2:latest".to_string(),
        ];

        let result = OllamaProbe::find_embedding_model(&models);
        assert_eq!(result, Some("my-embedding-model".to_string()));
    }

    #[test]
    fn test_model_matches_tier_tiny() {
        assert!(OllamaProbe::model_matches_tier("qwen2.5:1.5b", &ModelTier::Tiny));
        assert!(OllamaProbe::model_matches_tier("phi3:3b", &ModelTier::Tiny));
    }

    #[test]
    fn test_model_matches_tier_large() {
        assert!(OllamaProbe::model_matches_tier("qwen2.5:32b", &ModelTier::Large));
        assert!(OllamaProbe::model_matches_tier("llama2:32b", &ModelTier::Large));
    }

    #[test]
    fn test_find_best_model_fallback_to_first() {
        let models = vec!["some-random-model".to_string()];

        let result = OllamaProbe::find_best_model(&models, &ModelTier::Medium);
        assert_eq!(result, Some("some-random-model".to_string()));
    }

    #[test]
    fn test_find_best_model_empty() {
        let models: Vec<String> = vec![];

        let result = OllamaProbe::find_best_model(&models, &ModelTier::Medium);
        assert_eq!(result, None);
    }
}

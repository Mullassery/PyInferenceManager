use crate::types::{CloudModelEntry, CloudProvider, ModelRegistry, OrchestratorConfig};

pub struct MultiProviderRouter;

impl MultiProviderRouter {
    pub fn select_provider(
        config: &OrchestratorConfig,
        complexity: f32,
    ) -> Option<CloudProvider> {
        let mut models = config.models.cloud.clone();
        models.sort_by_key(|m| m.priority);

        for model in &models {
            match &model.provider {
                CloudProvider::Anthropic { .. } => {
                    if complexity > 0.6 {
                        return Some(model.provider.clone());
                    }
                }
                CloudProvider::OpenAI { .. } => {
                    if complexity <= 0.6 {
                        return Some(model.provider.clone());
                    }
                }
            }
        }

        models.first().map(|m| m.provider.clone())
    }

    pub fn fallback_order(config: &OrchestratorConfig) -> Vec<CloudProvider> {
        let mut models = config.models.cloud.clone();
        models.sort_by_key(|m| m.priority);
        models.iter().map(|m| m.provider.clone()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_config_with_providers() -> OrchestratorConfig {
        let mut config = OrchestratorConfig::default();
        config.models.add_cloud(
            CloudModelEntry::new(
                CloudProvider::Anthropic {
                    model: "claude-opus-4-1".to_string(),
                },
                "claude-opus-4-1".to_string(),
                0.003,
                0.015,
                200_000,
            )
            .with_priority(1),
        );

        config.models.add_cloud(
            CloudModelEntry::new(
                CloudProvider::OpenAI {
                    model: "gpt-4o-mini".to_string(),
                },
                "gpt-4o-mini".to_string(),
                0.00015,
                0.0006,
                128_000,
            )
            .with_priority(2),
        );

        config
    }

    #[test]
    fn test_select_provider_high_complexity_prefers_anthropic() {
        let config = create_config_with_providers();
        let provider = MultiProviderRouter::select_provider(&config, 0.8);

        match provider {
            Some(CloudProvider::Anthropic { model }) => {
                assert_eq!(model, "claude-opus-4-1");
            }
            _ => panic!("Expected Anthropic for high complexity"),
        }
    }

    #[test]
    fn test_select_provider_low_complexity_prefers_openai() {
        let config = create_config_with_providers();
        let provider = MultiProviderRouter::select_provider(&config, 0.3);

        match provider {
            Some(CloudProvider::OpenAI { model }) => {
                assert_eq!(model, "gpt-4o-mini");
            }
            _ => panic!("Expected OpenAI for low complexity"),
        }
    }

    #[test]
    fn test_fallback_order_respects_priority() {
        let config = create_config_with_providers();
        let order = MultiProviderRouter::fallback_order(&config);

        assert_eq!(order.len(), 2);
        match &order[0] {
            CloudProvider::Anthropic { .. } => {}
            _ => panic!("Expected Anthropic at priority 1"),
        }
        match &order[1] {
            CloudProvider::OpenAI { .. } => {}
            _ => panic!("Expected OpenAI at priority 2"),
        }
    }

    #[test]
    fn test_empty_model_registry() {
        let config = OrchestratorConfig::default();
        let provider = MultiProviderRouter::select_provider(&config, 0.5);
        assert!(provider.is_none());
    }
}

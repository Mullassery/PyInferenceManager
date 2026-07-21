use crate::types::{CloudModelEntry, CloudProvider, OrchestratorConfig};

#[derive(Debug, Clone)]
pub struct CostEstimate {
    pub provider: CloudProvider,
    pub model_id: String,
    pub estimated_input_tokens: u32,
    pub estimated_output_tokens: u32,
    pub estimated_input_cost: f32,
    pub estimated_output_cost: f32,
    pub total_estimated_cost: f32,
}

impl CostEstimate {
    pub fn new(
        provider: CloudProvider,
        model_id: String,
        estimated_input_tokens: u32,
        estimated_output_tokens: u32,
        cost_per_1k_input: f32,
        cost_per_1k_output: f32,
    ) -> Self {
        let input_cost = (estimated_input_tokens as f32 / 1000.0) * cost_per_1k_input;
        let output_cost = (estimated_output_tokens as f32 / 1000.0) * cost_per_1k_output;
        let total = input_cost + output_cost;

        CostEstimate {
            provider,
            model_id,
            estimated_input_tokens,
            estimated_output_tokens,
            estimated_input_cost: input_cost,
            estimated_output_cost: output_cost,
            total_estimated_cost: total,
        }
    }
}

pub struct CostEstimator;

impl CostEstimator {
    /// Estimate input tokens based on text length (rough heuristic)
    /// ~4 characters = 1 token (OpenAI average)
    pub fn estimate_input_tokens(description: &str, attachment_size: usize) -> u32 {
        let text_tokens = (description.len() as f32 / 4.0) as u32;
        // Rough estimate: 1 token per 1KB of attachment data
        let attachment_tokens = (attachment_size as u32 / 1024).max(0);
        (text_tokens + attachment_tokens).max(1) // at least 1 token
    }

    /// Estimate output tokens for typical task
    /// Simple queries: ~100-200 tokens
    /// Medium queries: ~300-500 tokens
    /// Complex queries: ~500-1000 tokens
    pub fn estimate_output_tokens(complexity: f32) -> u32 {
        if complexity < 0.3 {
            150
        } else if complexity < 0.6 {
            400
        } else if complexity < 0.8 {
            700
        } else {
            1000
        }
    }

    /// Estimate cost for a task on a specific cloud provider
    pub fn estimate_cost(
        description: &str,
        attachment_size: usize,
        complexity: f32,
        model_entry: &CloudModelEntry,
    ) -> CostEstimate {
        let input_tokens = Self::estimate_input_tokens(description, attachment_size);
        let output_tokens = Self::estimate_output_tokens(complexity);

        CostEstimate::new(
            model_entry.provider.clone(),
            model_entry.model_id.clone(),
            input_tokens,
            output_tokens,
            model_entry.cost_per_1k_input,
            model_entry.cost_per_1k_output,
        )
    }

    /// Compare costs across all registered cloud providers
    pub fn compare_costs(
        config: &OrchestratorConfig,
        description: &str,
        attachment_size: usize,
        complexity: f32,
    ) -> Vec<CostEstimate> {
        config
            .models
            .cloud
            .iter()
            .map(|entry| Self::estimate_cost(description, attachment_size, complexity, entry))
            .collect()
    }

    /// Find cheapest provider for task
    pub fn cheapest_provider(estimates: &[CostEstimate]) -> Option<&CostEstimate> {
        estimates
            .iter()
            .min_by(|a, b| a.total_estimated_cost.partial_cmp(&b.total_estimated_cost).unwrap())
    }

    /// Find most capable provider (highest cost typically means more capable)
    pub fn most_capable_provider(estimates: &[CostEstimate]) -> Option<&CostEstimate> {
        estimates
            .iter()
            .max_by(|a, b| a.total_estimated_cost.partial_cmp(&b.total_estimated_cost).unwrap())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_estimate_input_tokens_text_only() {
        let text = "This is a test query";
        let tokens = CostEstimator::estimate_input_tokens(text, 0);
        assert!(tokens > 0);
        assert!(tokens < 100);
    }

    #[test]
    fn test_estimate_input_tokens_with_attachment() {
        let text = "Analyze this document";
        let tokens_no_attachment = CostEstimator::estimate_input_tokens(text, 0);
        let tokens_with_attachment = CostEstimator::estimate_input_tokens(text, 10_000);
        assert!(tokens_with_attachment > tokens_no_attachment);
    }

    #[test]
    fn test_estimate_output_tokens_by_complexity() {
        let low = CostEstimator::estimate_output_tokens(0.2);
        let mid = CostEstimator::estimate_output_tokens(0.5);
        let high = CostEstimator::estimate_output_tokens(0.8);

        assert!(low < mid);
        assert!(mid < high);
    }

    #[test]
    fn test_cost_estimate_new() {
        let estimate = CostEstimate::new(
            CloudProvider::Anthropic {
                model: "claude-haiku-4-5".to_string(),
            },
            "claude-haiku-4-5".to_string(),
            1000,
            500,
            0.0003,
            0.0015,
        );

        assert_eq!(estimate.estimated_input_tokens, 1000);
        assert_eq!(estimate.estimated_output_tokens, 500);
        assert!(estimate.estimated_input_cost > 0.0);
        assert!(estimate.estimated_output_cost > 0.0);
        assert_eq!(
            estimate.total_estimated_cost,
            estimate.estimated_input_cost + estimate.estimated_output_cost
        );
    }

    #[test]
    fn test_compare_costs() {
        let mut config = OrchestratorConfig::default();
        config.models.add_cloud(CloudModelEntry::new(
            CloudProvider::Anthropic {
                model: "claude-haiku-4-5".to_string(),
            },
            "claude-haiku-4-5".to_string(),
            0.0003,
            0.0015,
            200_000,
        ));

        config.models.add_cloud(CloudModelEntry::new(
            CloudProvider::OpenAI {
                model: "gpt-4o-mini".to_string(),
            },
            "gpt-4o-mini".to_string(),
            0.00015,
            0.0006,
            128_000,
        ));

        let estimates = CostEstimator::compare_costs(&config, "test query", 0, 0.5);
        assert_eq!(estimates.len(), 2);

        // OpenAI should be cheaper for simple tasks
        let cheapest = CostEstimator::cheapest_provider(&estimates);
        assert!(cheapest.is_some());
    }

    #[test]
    fn test_cheapest_provider() {
        let estimates = vec![
            CostEstimate::new(
                CloudProvider::Anthropic {
                    model: "claude-opus-4-1".to_string(),
                },
                "claude-opus-4-1".to_string(),
                1000,
                500,
                0.003,
                0.015,
            ),
            CostEstimate::new(
                CloudProvider::OpenAI {
                    model: "gpt-4o-mini".to_string(),
                },
                "gpt-4o-mini".to_string(),
                1000,
                500,
                0.00015,
                0.0006,
            ),
        ];

        let cheapest = CostEstimator::cheapest_provider(&estimates);
        assert!(cheapest.is_some());
        match &cheapest.unwrap().provider {
            CloudProvider::OpenAI { .. } => {} // OpenAI is cheaper
            _ => panic!("Expected OpenAI to be cheaper"),
        }
    }

    #[test]
    fn test_most_capable_provider() {
        let estimates = vec![
            CostEstimate::new(
                CloudProvider::Anthropic {
                    model: "claude-opus-4-1".to_string(),
                },
                "claude-opus-4-1".to_string(),
                1000,
                500,
                0.003,
                0.015,
            ),
            CostEstimate::new(
                CloudProvider::OpenAI {
                    model: "gpt-4o-mini".to_string(),
                },
                "gpt-4o-mini".to_string(),
                1000,
                500,
                0.00015,
                0.0006,
            ),
        ];

        let most_capable = CostEstimator::most_capable_provider(&estimates);
        assert!(most_capable.is_some());
        match &most_capable.unwrap().provider {
            CloudProvider::Anthropic { .. } => {} // Anthropic is more expensive (more capable)
            _ => panic!("Expected Anthropic to be most capable"),
        }
    }
}

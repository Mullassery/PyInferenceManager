use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Performance metrics for routing decisions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderPerformance {
    pub provider_name: String,
    pub success_rate: f32,        // 0.0-1.0
    pub avg_latency_ms: u64,
    pub cost_per_1k_tokens: f32,
    pub health_score: f32,        // 0.0-1.0: combines success rate and latency
    pub request_count: u64,
    pub total_cost_usd: f32,
}

impl ProviderPerformance {
    pub fn new(provider_name: String) -> Self {
        Self {
            provider_name,
            success_rate: 1.0,
            avg_latency_ms: 0,
            cost_per_1k_tokens: 0.0,
            health_score: 1.0,
            request_count: 0,
            total_cost_usd: 0.0,
        }
    }

    pub fn calculate_health_score(&mut self) {
        // Health = success_rate (70% weight) + inverted_latency (30% weight)
        // Normalize latency: assume 5000ms is worst (0.0), 100ms is best (1.0)
        let max_latency = 5000.0;
        let latency_score = 1.0 - (self.avg_latency_ms as f32 / max_latency).min(1.0);

        self.health_score = (self.success_rate * 0.7) + (latency_score * 0.3);
    }
}

/// Dynamic router that adjusts routing based on real-time performance
#[derive(Debug, Clone)]
pub struct DynamicRouter {
    providers: HashMap<String, ProviderPerformance>,
    complexity_threshold: f32,
    update_interval_ms: u64,
}

impl DynamicRouter {
    pub fn new() -> Self {
        Self {
            providers: HashMap::new(),
            complexity_threshold: 0.7,
            update_interval_ms: 60000, // 1 minute
        }
    }

    pub fn register_provider(&mut self, name: String) {
        self.providers
            .insert(name.clone(), ProviderPerformance::new(name));
    }

    pub fn update_performance(
        &mut self,
        provider_name: &str,
        success: bool,
        latency_ms: u64,
        cost_usd: f32,
    ) {
        if let Some(perf) = self.providers.get_mut(provider_name) {
            let old_count = perf.request_count;
            perf.request_count += 1;

            // Update success rate (exponential moving average)
            let alpha = 0.1; // Learning rate
            let success_rate = if success { 1.0 } else { 0.0 };
            perf.success_rate = (alpha * success_rate) + ((1.0 - alpha) * perf.success_rate);

            // Update latency (exponential moving average)
            perf.avg_latency_ms =
                (alpha as u64 * latency_ms) + ((1.0 - alpha) as u64 * perf.avg_latency_ms);

            // Update cost
            if old_count > 0 {
                perf.cost_per_1k_tokens = (perf.cost_per_1k_tokens * old_count as f32 + cost_usd)
                    / perf.request_count as f32;
            } else {
                perf.cost_per_1k_tokens = cost_usd;
            }

            perf.total_cost_usd += cost_usd;

            // Recalculate health score
            perf.calculate_health_score();
        }
    }

    pub fn select_provider_for_complexity(&self, complexity: f32) -> Option<String> {
        let mut candidates: Vec<_> = self
            .providers
            .iter()
            .filter(|(_, perf)| perf.success_rate > 0.5)
            .collect();

        if candidates.is_empty() {
            return None;
        }

        // Sort by health score (descending)
        candidates.sort_by(|a, b| {
            b.1.health_score
                .partial_cmp(&a.1.health_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // For high complexity, prefer most reliable providers
        if complexity > self.complexity_threshold {
            return candidates
                .first()
                .map(|(name, _)| name.to_string());
        }

        // For low complexity, prefer lowest cost providers
        candidates.sort_by(|a, b| {
            a.1.cost_per_1k_tokens
                .partial_cmp(&b.1.cost_per_1k_tokens)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        candidates.first().map(|(name, _)| name.to_string())
    }

    pub fn get_provider_ranking(&self) -> Vec<(String, f32)> {
        let mut ranking: Vec<_> = self
            .providers
            .iter()
            .map(|(name, perf)| (name.clone(), perf.health_score))
            .collect();

        ranking.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        ranking
    }

    pub fn get_provider_metrics(&self) -> HashMap<String, ProviderPerformance> {
        self.providers.clone()
    }

    pub fn is_provider_healthy(&self, provider_name: &str) -> bool {
        self.providers
            .get(provider_name)
            .map(|perf| perf.health_score > 0.6 && perf.success_rate > 0.8)
            .unwrap_or(false)
    }
}

impl Default for DynamicRouter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_performance_new() {
        let perf = ProviderPerformance::new("test".to_string());
        assert_eq!(perf.provider_name, "test");
        assert_eq!(perf.success_rate, 1.0);
        assert_eq!(perf.health_score, 1.0);
    }

    #[test]
    fn test_provider_health_score_calculation() {
        let mut perf = ProviderPerformance::new("test".to_string());
        perf.success_rate = 0.95;
        perf.avg_latency_ms = 200;
        perf.calculate_health_score();
        assert!(perf.health_score > 0.8);
    }

    #[test]
    fn test_dynamic_router_new() {
        let router = DynamicRouter::new();
        assert_eq!(router.providers.len(), 0);
    }

    #[test]
    fn test_dynamic_router_register_provider() {
        let mut router = DynamicRouter::new();
        router.register_provider("anthropic".to_string());
        router.register_provider("openai".to_string());
        assert_eq!(router.providers.len(), 2);
    }

    #[test]
    fn test_dynamic_router_update_performance() {
        let mut router = DynamicRouter::new();
        router.register_provider("test".to_string());
        router.update_performance("test", true, 150, 0.5);
        router.update_performance("test", true, 160, 0.5);

        let metrics = router.get_provider_metrics();
        assert_eq!(metrics["test"].request_count, 2);
        assert!(metrics["test"].success_rate > 0.9);
    }

    #[test]
    fn test_dynamic_router_select_provider_high_complexity() {
        let mut router = DynamicRouter::new();
        router.register_provider("anthropic".to_string());
        router.register_provider("openai".to_string());

        // Make anthropic more reliable
        for _ in 0..10 {
            router.update_performance("anthropic", true, 200, 0.5);
        }
        for _ in 0..5 {
            router.update_performance("openai", true, 100, 0.3);
        }

        // High complexity should prefer more reliable provider
        let selected = router.select_provider_for_complexity(0.8);
        assert_eq!(selected, Some("anthropic".to_string()));
    }

    #[test]
    fn test_dynamic_router_select_provider_low_complexity() {
        let mut router = DynamicRouter::new();
        router.register_provider("anthropic".to_string());
        router.register_provider("openai".to_string());

        // Make both reliable but anthropic more expensive
        for _ in 0..5 {
            router.update_performance("anthropic", true, 150, 2.0);
            router.update_performance("openai", true, 160, 0.5);
        }

        // Low complexity should prefer cheaper provider
        let selected = router.select_provider_for_complexity(0.3);
        assert_eq!(selected, Some("openai".to_string()));
    }

    #[test]
    fn test_dynamic_router_provider_ranking() {
        let mut router = DynamicRouter::new();
        router.register_provider("provider1".to_string());
        router.register_provider("provider2".to_string());

        // Make provider1 better
        for _ in 0..10 {
            router.update_performance("provider1", true, 100, 0.5);
        }
        for _ in 0..5 {
            router.update_performance("provider2", true, 300, 0.8);
        }

        let ranking = router.get_provider_ranking();
        assert_eq!(ranking[0].0, "provider1");
    }

    #[test]
    fn test_is_provider_healthy() {
        let mut router = DynamicRouter::new();
        router.register_provider("test".to_string());

        // Make provider healthy
        for _ in 0..10 {
            router.update_performance("test", true, 150, 0.5);
        }

        assert!(router.is_provider_healthy("test"));
    }
}

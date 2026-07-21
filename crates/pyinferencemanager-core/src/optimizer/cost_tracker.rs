use chrono::{DateTime, Utc};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct ExecutionRecord {
    pub engine: String,
    pub tokens: u32,
    pub latency_ms: u64,
    pub cost_usd: f32,
    pub timestamp: DateTime<Utc>,
}

impl ExecutionRecord {
    pub fn new(engine: String, tokens: u32, latency_ms: u64, cost_usd: f32) -> Self {
        ExecutionRecord {
            engine,
            tokens,
            latency_ms,
            cost_usd,
            timestamp: Utc::now(),
        }
    }
}

pub struct CostTracker {
    records: Vec<ExecutionRecord>,
    engine_stats: HashMap<String, EngineStats>,
}

#[derive(Debug, Clone, Default)]
struct EngineStats {
    total_tokens: u64,
    total_latency_ms: u64,
    total_cost_usd: f32,
    invocation_count: u64,
    avg_tokens: f32,
    avg_latency_ms: f32,
    avg_cost_usd: f32,
}

impl EngineStats {
    fn update(&mut self, tokens: u32, latency_ms: u64, cost_usd: f32) {
        self.total_tokens += tokens as u64;
        self.total_latency_ms += latency_ms;
        self.total_cost_usd += cost_usd;
        self.invocation_count += 1;

        self.avg_tokens = self.total_tokens as f32 / self.invocation_count as f32;
        self.avg_latency_ms = self.total_latency_ms as f32 / self.invocation_count as f32;
        self.avg_cost_usd = self.total_cost_usd / self.invocation_count as f32;
    }
}

impl CostTracker {
    pub fn new() -> Self {
        CostTracker {
            records: Vec::new(),
            engine_stats: HashMap::new(),
        }
    }

    pub fn record(&mut self, record: ExecutionRecord) {
        let engine = record.engine.clone();
        let tokens = record.tokens;
        let latency = record.latency_ms;
        let cost = record.cost_usd;

        self.records.push(record);

        self.engine_stats
            .entry(engine)
            .or_insert_with(EngineStats::default)
            .update(tokens, latency, cost);
    }

    pub fn total_cost_usd(&self) -> f32 {
        self.records.iter().map(|r| r.cost_usd).sum()
    }

    pub fn total_tokens(&self) -> u64 {
        self.records.iter().map(|r| r.tokens as u64).sum()
    }

    pub fn total_latency_ms(&self) -> u64 {
        self.records.iter().map(|r| r.latency_ms).sum()
    }

    pub fn invocation_count(&self) -> u64 {
        self.records.len() as u64
    }

    pub fn avg_latency_ms(&self, engine: &str) -> Option<f32> {
        self.engine_stats.get(engine).map(|s| s.avg_latency_ms)
    }

    pub fn avg_cost_usd(&self, engine: &str) -> Option<f32> {
        self.engine_stats.get(engine).map(|s| s.avg_cost_usd)
    }

    pub fn cost_per_1k_tokens(&self, engine: &str) -> Option<f32> {
        self.engine_stats.get(engine).and_then(|s| {
            if s.total_tokens == 0 {
                None
            } else {
                Some((s.total_cost_usd / s.total_tokens as f32) * 1000.0)
            }
        })
    }

    pub fn stats_for_engine(&self, engine: &str) -> Option<EngineStats> {
        self.engine_stats.get(engine).cloned()
    }

    pub fn all_stats(&self) -> HashMap<String, EngineStats> {
        self.engine_stats.clone()
    }

    pub fn clear(&mut self) {
        self.records.clear();
        self.engine_stats.clear();
    }
}

impl Default for CostTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cost_tracker_new() {
        let tracker = CostTracker::new();
        assert_eq!(tracker.invocation_count(), 0);
        assert_eq!(tracker.total_cost_usd(), 0.0);
    }

    #[test]
    fn test_record_single_execution() {
        let mut tracker = CostTracker::new();
        let record = ExecutionRecord::new("local_llm".to_string(), 100, 500, 0.0);

        tracker.record(record);

        assert_eq!(tracker.invocation_count(), 1);
        assert_eq!(tracker.total_tokens(), 100);
        assert_eq!(tracker.total_latency_ms(), 500);
        assert_eq!(tracker.total_cost_usd(), 0.0);
    }

    #[test]
    fn test_record_multiple_executions() {
        let mut tracker = CostTracker::new();

        tracker.record(ExecutionRecord::new("local_llm".to_string(), 100, 500, 0.0));
        tracker.record(ExecutionRecord::new("cloud_llm".to_string(), 50, 1000, 0.01));
        tracker.record(ExecutionRecord::new("local_llm".to_string(), 80, 400, 0.0));

        assert_eq!(tracker.invocation_count(), 3);
        assert_eq!(tracker.total_tokens(), 230);
        assert_eq!(tracker.total_latency_ms(), 1900);
        assert!(tracker.total_cost_usd() > 0.009 && tracker.total_cost_usd() < 0.011);
    }

    #[test]
    fn test_avg_latency_per_engine() {
        let mut tracker = CostTracker::new();

        tracker.record(ExecutionRecord::new("local_llm".to_string(), 100, 500, 0.0));
        tracker.record(ExecutionRecord::new("local_llm".to_string(), 100, 600, 0.0));

        let avg = tracker.avg_latency_ms("local_llm");
        assert!(avg.is_some());
        assert_eq!(avg.unwrap(), 550.0);
    }

    #[test]
    fn test_cost_per_1k_tokens() {
        let mut tracker = CostTracker::new();

        tracker.record(ExecutionRecord::new("cloud_llm".to_string(), 1000, 1000, 0.25));

        let cost_per_k = tracker.cost_per_1k_tokens("cloud_llm");
        assert!(cost_per_k.is_some());
        assert_eq!(cost_per_k.unwrap(), 0.25);
    }

    #[test]
    fn test_stats_for_engine() {
        let mut tracker = CostTracker::new();

        tracker.record(ExecutionRecord::new("local_llm".to_string(), 100, 500, 0.0));
        tracker.record(ExecutionRecord::new("local_llm".to_string(), 50, 300, 0.0));

        let stats = tracker.stats_for_engine("local_llm");
        assert!(stats.is_some());
        let s = stats.unwrap();
        assert_eq!(s.total_tokens, 150);
        assert_eq!(s.total_latency_ms, 800);
        assert_eq!(s.invocation_count, 2);
    }

    #[test]
    fn test_clear_tracker() {
        let mut tracker = CostTracker::new();

        tracker.record(ExecutionRecord::new("local_llm".to_string(), 100, 500, 0.0));
        tracker.clear();

        assert_eq!(tracker.invocation_count(), 0);
        assert_eq!(tracker.total_tokens(), 0);
    }

    #[test]
    fn test_engine_stats_update() {
        let mut stats = EngineStats::default();

        stats.update(100, 500, 0.01);
        assert_eq!(stats.avg_tokens, 100.0);
        assert_eq!(stats.avg_latency_ms, 500.0);
        assert_eq!(stats.avg_cost_usd, 0.01);

        stats.update(200, 1000, 0.02);
        assert_eq!(stats.avg_tokens, 150.0);
        assert_eq!(stats.avg_latency_ms, 750.0);
        assert!(stats.avg_cost_usd > 0.014 && stats.avg_cost_usd < 0.016);
    }
}

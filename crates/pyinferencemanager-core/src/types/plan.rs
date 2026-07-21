use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::dag::Dag;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionStage {
    pub stage_index: u32,
    pub parallel_node_ids: Vec<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionPlan {
    pub id: String,
    pub task_id: String,
    pub dag: Dag,
    pub stages: Vec<ExecutionStage>,
    pub estimated_cost_usd: f32,
    pub estimated_latency_ms: u64,
    pub local_first: bool,
}

impl ExecutionPlan {
    pub fn new(task_id: String, dag: Dag) -> Self {
        let stages = dag
            .execution_stages()
            .into_iter()
            .enumerate()
            .map(|(idx, parallel_node_ids)| ExecutionStage {
                stage_index: idx as u32,
                parallel_node_ids,
            })
            .collect();

        ExecutionPlan {
            id: Uuid::new_v4().to_string(),
            task_id,
            dag,
            stages,
            estimated_cost_usd: 0.0,
            estimated_latency_ms: 0,
            local_first: true,
        }
    }

    pub fn with_cost_and_latency(
        mut self,
        cost: f32,
        latency_ms: u64,
    ) -> Self {
        self.estimated_cost_usd = cost;
        self.estimated_latency_ms = latency_ms;
        self
    }

    pub fn with_local_first(mut self, local_first: bool) -> Self {
        self.local_first = local_first;
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeResult {
    pub node_id: usize,
    pub output: String,
    pub tokens_used: u32,
    pub latency_ms: u64,
    pub engine_used: String,
    pub cache_hit: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkloadResult {
    pub task_id: String,
    pub plan_id: String,
    pub output: String,
    pub node_results: Vec<NodeResult>,
    pub total_tokens: u32,
    pub total_cost_usd: f32,
    pub estimated_cost_usd: f32,
    pub total_latency_ms: u64,
    pub engines_used: Vec<String>,
    pub cache_hits: u32,
    pub retry_attempts: u32,
    pub provider_used: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl WorkloadResult {
    pub fn new(task_id: String, plan_id: String, output: String) -> Self {
        WorkloadResult {
            task_id,
            plan_id,
            output,
            node_results: Vec::new(),
            total_tokens: 0,
            total_cost_usd: 0.0,
            estimated_cost_usd: 0.0,
            total_latency_ms: 0,
            engines_used: Vec::new(),
            cache_hits: 0,
            retry_attempts: 0,
            provider_used: None,
            created_at: Utc::now(),
        }
    }

    pub fn add_node_result(&mut self, result: NodeResult) {
        self.total_tokens += result.tokens_used;
        self.total_latency_ms += result.latency_ms;
        if result.cache_hit {
            self.cache_hits += 1;
        }
        if !self.engines_used.contains(&result.engine_used) {
            self.engines_used.push(result.engine_used.clone());
        }
        self.node_results.push(result);
    }

    pub fn with_retry_attempts(mut self, attempts: u32) -> Self {
        self.retry_attempts = attempts;
        self
    }

    pub fn with_provider_used(mut self, provider: String) -> Self {
        self.provider_used = Some(provider);
        self
    }

    pub fn with_estimated_cost(mut self, cost: f32) -> Self {
        self.estimated_cost_usd = cost;
        self
    }

    pub fn to_dict(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::dag::{DagNode, ExecutionEngine};

    #[test]
    fn test_execution_plan_new() {
        let mut dag = Dag::new("task1".to_string());
        dag.add_node(DagNode::new(0, "node0".to_string(), ExecutionEngine::CacheLookup));

        let plan = ExecutionPlan::new("task1".to_string(), dag);
        assert_eq!(plan.task_id, "task1");
        assert!(!plan.stages.is_empty());
    }

    #[test]
    fn test_execution_plan_builders() {
        let mut dag = Dag::new("task1".to_string());
        dag.add_node(DagNode::new(0, "node0".to_string(), ExecutionEngine::CacheLookup));

        let plan = ExecutionPlan::new("task1".to_string(), dag)
            .with_cost_and_latency(0.05, 1000)
            .with_local_first(false);

        assert_eq!(plan.estimated_cost_usd, 0.05);
        assert_eq!(plan.estimated_latency_ms, 1000);
        assert!(!plan.local_first);
    }

    #[test]
    fn test_workload_result_new() {
        let result = WorkloadResult::new(
            "task1".to_string(),
            "plan1".to_string(),
            "output".to_string(),
        );
        assert_eq!(result.task_id, "task1");
        assert_eq!(result.output, "output");
        assert_eq!(result.total_tokens, 0);
        assert_eq!(result.cache_hits, 0);
    }

    #[test]
    fn test_workload_result_add_node_result() {
        let mut result = WorkloadResult::new(
            "task1".to_string(),
            "plan1".to_string(),
            "output".to_string(),
        );

        result.add_node_result(NodeResult {
            node_id: 0,
            output: "node_output".to_string(),
            tokens_used: 100,
            latency_ms: 500,
            engine_used: "local_llm".to_string(),
            cache_hit: false,
        });

        assert_eq!(result.total_tokens, 100);
        assert_eq!(result.total_latency_ms, 500);
        assert_eq!(result.cache_hits, 0);
        assert!(result.engines_used.contains(&"local_llm".to_string()));

        result.add_node_result(NodeResult {
            node_id: 1,
            output: "cached".to_string(),
            tokens_used: 0,
            latency_ms: 10,
            engine_used: "cache_lookup".to_string(),
            cache_hit: true,
        });

        assert_eq!(result.total_tokens, 100);
        assert_eq!(result.total_latency_ms, 510);
        assert_eq!(result.cache_hits, 1);
    }

    #[test]
    fn test_workload_result_to_dict() {
        let result = WorkloadResult::new(
            "task1".to_string(),
            "plan1".to_string(),
            "output".to_string(),
        );
        let dict = result.to_dict();
        assert!(dict.is_object());
        assert_eq!(dict["task_id"], "task1");
    }
}

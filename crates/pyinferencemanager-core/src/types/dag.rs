use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ExecutionEngine {
    LocalLlm { model: String },
    CloudLlm { provider: CloudProvider },
    Embedding { model: String },
    CacheLookup,
    RulesBased,
    Tool { name: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum CloudProvider {
    Anthropic { model: String },
    OpenAI { model: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum NodeStatus {
    Pending,
    Running,
    Completed,
    Failed { reason: String },
    CacheHit,
}

impl Default for NodeStatus {
    fn default() -> Self {
        NodeStatus::Pending
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DagNode {
    pub id: usize,
    pub label: String,
    pub engine: ExecutionEngine,
    pub prompt_template: String,
    pub depends_on: Vec<usize>,
    pub cacheable: bool,
    pub complexity_score: f32,
    pub status: NodeStatus,
    pub result: Option<String>,
    pub tokens_used: u32,
    pub latency_ms: u64,
}

impl DagNode {
    pub fn new(id: usize, label: String, engine: ExecutionEngine) -> Self {
        DagNode {
            id,
            label,
            engine,
            prompt_template: String::new(),
            depends_on: Vec::new(),
            cacheable: false,
            complexity_score: 0.5,
            status: NodeStatus::Pending,
            result: None,
            tokens_used: 0,
            latency_ms: 0,
        }
    }

    pub fn with_template(mut self, template: String) -> Self {
        self.prompt_template = template;
        self
    }

    pub fn with_depends_on(mut self, depends_on: Vec<usize>) -> Self {
        self.depends_on = depends_on;
        self
    }

    pub fn with_cacheable(mut self, cacheable: bool) -> Self {
        self.cacheable = cacheable;
        self
    }

    pub fn with_complexity(mut self, score: f32) -> Self {
        self.complexity_score = score.clamp(0.0, 1.0);
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dag {
    pub id: String,
    pub task_id: String,
    pub nodes: Vec<DagNode>,
    pub edges: Vec<(usize, usize)>,
}

impl Dag {
    pub fn new(task_id: String) -> Self {
        Dag {
            id: Uuid::new_v4().to_string(),
            task_id,
            nodes: Vec::new(),
            edges: Vec::new(),
        }
    }

    pub fn add_node(&mut self, node: DagNode) {
        self.nodes.push(node);
    }

    pub fn add_edge(&mut self, from: usize, to: usize) {
        self.edges.push((from, to));
    }

    pub fn execution_stages(&self) -> Vec<Vec<usize>> {
        if self.nodes.is_empty() {
            return Vec::new();
        }

        let mut in_degree = vec![0usize; self.nodes.len()];
        for (_, to) in &self.edges {
            if *to < in_degree.len() {
                in_degree[*to] += 1;
            }
        }

        let mut stages = Vec::new();
        let mut processed = vec![false; self.nodes.len()];

        loop {
            let mut current_stage = Vec::new();
            for (id, _) in self.nodes.iter().enumerate() {
                if !processed[id] && in_degree[id] == 0 {
                    current_stage.push(id);
                    processed[id] = true;
                }
            }

            if current_stage.is_empty() {
                break;
            }

            for (from, to) in &self.edges {
                if processed[*from] && *to < in_degree.len() {
                    in_degree[*to] = in_degree[*to].saturating_sub(1);
                }
            }

            stages.push(current_stage);
        }

        stages
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dag_node_new() {
        let node = DagNode::new(0, "test".to_string(), ExecutionEngine::CacheLookup);
        assert_eq!(node.id, 0);
        assert_eq!(node.label, "test");
        assert_eq!(node.status, NodeStatus::Pending);
        assert_eq!(node.depends_on.len(), 0);
    }

    #[test]
    fn test_dag_node_builders() {
        let node = DagNode::new(0, "test".to_string(), ExecutionEngine::CacheLookup)
            .with_template("template".to_string())
            .with_cacheable(true)
            .with_complexity(0.7);

        assert_eq!(node.prompt_template, "template");
        assert!(node.cacheable);
        assert_eq!(node.complexity_score, 0.7);
    }

    #[test]
    fn test_complexity_clamping() {
        let node = DagNode::new(0, "test".to_string(), ExecutionEngine::CacheLookup)
            .with_complexity(2.0);
        assert_eq!(node.complexity_score, 1.0);

        let node = DagNode::new(0, "test".to_string(), ExecutionEngine::CacheLookup)
            .with_complexity(-0.5);
        assert_eq!(node.complexity_score, 0.0);
    }

    #[test]
    fn test_dag_new() {
        let dag = Dag::new("task1".to_string());
        assert_eq!(dag.task_id, "task1");
        assert!(dag.nodes.is_empty());
        assert!(dag.edges.is_empty());
    }

    #[test]
    fn test_dag_add_nodes_and_edges() {
        let mut dag = Dag::new("task1".to_string());
        dag.add_node(DagNode::new(0, "node0".to_string(), ExecutionEngine::CacheLookup));
        dag.add_node(DagNode::new(1, "node1".to_string(), ExecutionEngine::CacheLookup));
        dag.add_edge(0, 1);

        assert_eq!(dag.nodes.len(), 2);
        assert_eq!(dag.edges.len(), 1);
    }

    #[test]
    fn test_execution_stages_linear() {
        let mut dag = Dag::new("task1".to_string());
        dag.add_node(DagNode::new(0, "node0".to_string(), ExecutionEngine::CacheLookup));
        dag.add_node(DagNode::new(1, "node1".to_string(), ExecutionEngine::CacheLookup));
        dag.add_node(DagNode::new(2, "node2".to_string(), ExecutionEngine::CacheLookup));
        dag.add_edge(0, 1);
        dag.add_edge(1, 2);

        let stages = dag.execution_stages();
        assert_eq!(stages.len(), 3);
        assert_eq!(stages[0], vec![0]);
        assert_eq!(stages[1], vec![1]);
        assert_eq!(stages[2], vec![2]);
    }

    #[test]
    fn test_execution_stages_parallel() {
        let mut dag = Dag::new("task1".to_string());
        dag.add_node(DagNode::new(0, "node0".to_string(), ExecutionEngine::CacheLookup));
        dag.add_node(DagNode::new(1, "node1".to_string(), ExecutionEngine::CacheLookup));
        dag.add_node(DagNode::new(2, "node2".to_string(), ExecutionEngine::CacheLookup));
        dag.add_edge(0, 1);
        dag.add_edge(0, 2);

        let stages = dag.execution_stages();
        assert_eq!(stages.len(), 2);
        assert_eq!(stages[0], vec![0]);
        let mut second_stage = stages[1].clone();
        second_stage.sort();
        assert_eq!(second_stage, vec![1, 2]);
    }

    #[test]
    fn test_execution_stages_diamond() {
        let mut dag = Dag::new("task1".to_string());
        dag.add_node(DagNode::new(0, "node0".to_string(), ExecutionEngine::CacheLookup));
        dag.add_node(DagNode::new(1, "node1".to_string(), ExecutionEngine::CacheLookup));
        dag.add_node(DagNode::new(2, "node2".to_string(), ExecutionEngine::CacheLookup));
        dag.add_node(DagNode::new(3, "node3".to_string(), ExecutionEngine::CacheLookup));
        dag.add_edge(0, 1);
        dag.add_edge(0, 2);
        dag.add_edge(1, 3);
        dag.add_edge(2, 3);

        let stages = dag.execution_stages();
        assert_eq!(stages.len(), 3);
        assert_eq!(stages[0], vec![0]);
        let mut second_stage = stages[1].clone();
        second_stage.sort();
        assert_eq!(second_stage, vec![1, 2]);
        assert_eq!(stages[2], vec![3]);
    }

    #[test]
    fn test_execution_stages_empty() {
        let dag = Dag::new("task1".to_string());
        let stages = dag.execution_stages();
        assert!(stages.is_empty());
    }
}

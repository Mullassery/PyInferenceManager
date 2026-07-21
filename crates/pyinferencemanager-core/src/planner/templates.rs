use crate::types::{Dag, DagNode, ExecutionEngine};

#[derive(Debug, Clone, PartialEq)]
pub enum DagTemplate {
    DocumentAnalysis,
    QuestionAnswering,
}

impl DagTemplate {
    pub fn build_dag(&self, task_id: String) -> Dag {
        let mut dag = Dag::new(task_id);

        match self {
            DagTemplate::DocumentAnalysis => {
                dag.add_node(
                    DagNode::new(0, "cache_lookup".to_string(), ExecutionEngine::CacheLookup)
                        .with_cacheable(false)
                        .with_complexity(0.0),
                );

                dag.add_node(
                    DagNode::new(
                        1,
                        "structure_detection".to_string(),
                        ExecutionEngine::LocalLlm {
                            model: "llama3.2:latest".to_string(),
                        },
                    )
                    .with_template(
                        "Analyze the document structure. Identify headings, sections, tables, and hierarchy. Output JSON with structure details."
                            .to_string(),
                    )
                    .with_cacheable(true)
                    .with_complexity(0.4),
                );

                dag.add_node(
                    DagNode::new(
                        2,
                        "metadata_extraction".to_string(),
                        ExecutionEngine::LocalLlm {
                            model: "llama3.2:latest".to_string(),
                        },
                    )
                    .with_template(
                        "Extract metadata: title, author, dates, entities, document type, key topics. Output JSON."
                            .to_string(),
                    )
                    .with_cacheable(true)
                    .with_complexity(0.5)
                    .with_depends_on(vec![1]),
                );

                dag.add_node(
                    DagNode::new(
                        3,
                        "synthesis".to_string(),
                        ExecutionEngine::LocalLlm {
                            model: "llama3.2:latest".to_string(),
                        },
                    )
                    .with_template(
                        "Synthesize the document analysis into a concise summary."
                            .to_string(),
                    )
                    .with_cacheable(true)
                    .with_complexity(0.6)
                    .with_depends_on(vec![2]),
                );

                dag.add_edge(1, 2);
                dag.add_edge(2, 3);
            }
            DagTemplate::QuestionAnswering => {
                dag.add_node(
                    DagNode::new(0, "cache_lookup".to_string(), ExecutionEngine::CacheLookup)
                        .with_cacheable(false)
                        .with_complexity(0.0),
                );

                dag.add_node(
                    DagNode::new(
                        1,
                        "answer_generation".to_string(),
                        ExecutionEngine::LocalLlm {
                            model: "llama3.2:latest".to_string(),
                        },
                    )
                    .with_template("Answer the user's question based on the provided context."
                        .to_string())
                    .with_cacheable(true)
                    .with_complexity(0.5)
                    .with_depends_on(vec![0]),
                );

                dag.add_edge(0, 1);
            }
        }

        dag
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_document_analysis_template() {
        let dag = DagTemplate::DocumentAnalysis.build_dag("task1".to_string());
        assert_eq!(dag.nodes.len(), 4);
        assert_eq!(dag.nodes[0].label, "cache_lookup");
        assert_eq!(dag.nodes[1].label, "structure_detection");
        assert_eq!(dag.nodes[2].label, "metadata_extraction");
        assert_eq!(dag.nodes[3].label, "synthesis");
    }

    #[test]
    fn test_question_answering_template() {
        let dag = DagTemplate::QuestionAnswering.build_dag("task2".to_string());
        assert_eq!(dag.nodes.len(), 2);
        assert_eq!(dag.nodes[0].label, "cache_lookup");
        assert_eq!(dag.nodes[1].label, "answer_generation");
    }

    #[test]
    fn test_document_analysis_dependencies() {
        let dag = DagTemplate::DocumentAnalysis.build_dag("task1".to_string());
        assert_eq!(dag.edges.len(), 2);
        assert_eq!(dag.edges[0], (1, 2));
        assert_eq!(dag.edges[1], (2, 3));
    }

    #[test]
    fn test_document_analysis_stages() {
        let dag = DagTemplate::DocumentAnalysis.build_dag("task1".to_string());
        let stages = dag.execution_stages();
        assert_eq!(stages.len(), 3);
        assert_eq!(stages[0], vec![0, 1]);
        assert_eq!(stages[1], vec![2]);
        assert_eq!(stages[2], vec![3]);
    }

    #[test]
    fn test_question_answering_stages() {
        let dag = DagTemplate::QuestionAnswering.build_dag("task2".to_string());
        let stages = dag.execution_stages();
        assert_eq!(stages.len(), 2);
        assert_eq!(stages[0], vec![0]);
        assert_eq!(stages[1], vec![1]);
    }

    #[test]
    fn test_template_equality() {
        assert_eq!(
            DagTemplate::DocumentAnalysis,
            DagTemplate::DocumentAnalysis
        );
        assert_ne!(
            DagTemplate::DocumentAnalysis,
            DagTemplate::QuestionAnswering
        );
    }
}

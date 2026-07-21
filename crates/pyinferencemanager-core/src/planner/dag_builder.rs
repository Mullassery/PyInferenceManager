use crate::analyzer::{ComplexityScorer, TaskClassifier};
use crate::types::{Dag, DagNode, ExecutionEngine, Task};
use crate::Result;
use super::templates::DagTemplate;

#[derive(Debug, Clone, Copy)]
pub struct DagBuilder;

impl DagBuilder {
    pub fn new() -> Self {
        DagBuilder
    }

    pub fn build(task: &Task) -> Result<Dag> {
        let task_kind = TaskClassifier::classify(&task.description);
        let attachment_size = task
            .attachments
            .iter()
            .map(|a| a.content.len())
            .sum();
        let complexity = ComplexityScorer::score(&task.description, attachment_size);

        let template = Self::select_template(&task_kind, complexity);
        let mut dag = template.build_dag(task.id.clone());

        for node in &mut dag.nodes {
            node.complexity_score = complexity;
        }

        Ok(dag)
    }

    fn select_template(task_kind: &crate::types::TaskKind, complexity: f32) -> DagTemplate {
        match task_kind {
            crate::types::TaskKind::DocumentAnalysis => DagTemplate::DocumentAnalysis,
            crate::types::TaskKind::QuestionAnswering => DagTemplate::QuestionAnswering,
            crate::types::TaskKind::CustomerSupport => {
                if complexity > 0.6 {
                    DagTemplate::DocumentAnalysis
                } else {
                    DagTemplate::QuestionAnswering
                }
            }
            crate::types::TaskKind::CodeAnalysis => {
                if complexity > 0.7 {
                    DagTemplate::DocumentAnalysis
                } else {
                    DagTemplate::QuestionAnswering
                }
            }
            crate::types::TaskKind::DataExtraction => DagTemplate::DocumentAnalysis,
            crate::types::TaskKind::Summarization => DagTemplate::DocumentAnalysis,
            crate::types::TaskKind::Unknown => DagTemplate::QuestionAnswering,
        }
    }
}

impl Default for DagBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dag_builder_new() {
        let builder = DagBuilder::new();
        assert_eq!(builder, DagBuilder);
    }

    #[test]
    fn test_build_document_analysis() {
        let task = Task::new("Analyze this PDF contract".to_string());
        let dag = DagBuilder::build(&task).unwrap();

        assert_eq!(dag.task_id, task.id);
        assert!(dag.nodes.len() > 0);
    }

    #[test]
    fn test_build_question_answering() {
        let task = Task::new("What is the invoice number?".to_string());
        let dag = DagBuilder::build(&task).unwrap();

        assert_eq!(dag.task_id, task.id);
        assert!(dag.nodes.len() > 0);
    }

    #[test]
    fn test_build_inherits_complexity() {
        let task = Task::new("Analyze and compare data".to_string());
        let dag = DagBuilder::build(&task).unwrap();

        for node in dag.nodes {
            if node.id > 0 {
                assert!(node.complexity_score > 0.0);
            }
        }
    }

    #[test]
    fn test_build_with_attachment() {
        let mut task = Task::new("Extract data from document".to_string());
        task.attachments.push(crate::types::Attachment {
            kind: crate::types::AttachmentKind::File,
            content: vec![0u8; 100_000],
            mime_type: "application/pdf".to_string(),
            name: "document.pdf".to_string(),
        });

        let dag = DagBuilder::build(&task).unwrap();
        assert!(dag.nodes.len() > 0);

        for node in dag.nodes {
            if node.id > 0 {
                assert!(node.complexity_score > 0.0);
            }
        }
    }

    #[test]
    fn test_builder_default() {
        let builder = DagBuilder::default();
        let builder2 = DagBuilder::new();
        assert_eq!(builder, builder2);
    }
}

impl PartialEq for DagBuilder {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

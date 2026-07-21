use crate::types::TaskKind;

pub struct TaskClassifier;

impl TaskClassifier {
    pub fn classify(description: &str) -> TaskKind {
        let desc = description.to_lowercase();

        if desc.contains("support") || desc.contains("customer") || desc.contains("complaint") {
            return TaskKind::CustomerSupport;
        }

        if desc.contains("extract") && desc.contains("data") {
            return TaskKind::DataExtraction;
        }

        if desc.contains("code") || desc.contains("debug") || desc.contains("analyze code") {
            return TaskKind::CodeAnalysis;
        }

        if desc.contains("question")
            || desc.contains("?")
            || desc.contains("what")
            || desc.contains("how")
        {
            return TaskKind::QuestionAnswering;
        }

        if desc.contains("summarize") || desc.contains("summary") {
            return TaskKind::Summarization;
        }

        if desc.contains("document")
            || desc.contains("pdf")
            || desc.contains("contract")
            || (desc.contains("extract") && !desc.contains("data"))
        {
            return TaskKind::DocumentAnalysis;
        }

        TaskKind::Unknown
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_document_analysis() {
        let kind = TaskClassifier::classify("Analyze this PDF document");
        assert_eq!(kind, TaskKind::DocumentAnalysis);
    }

    #[test]
    fn test_classify_question_answering() {
        let kind = TaskClassifier::classify("What is in this contract?");
        assert_eq!(kind, TaskKind::QuestionAnswering);
    }

    #[test]
    fn test_classify_customer_support() {
        let kind = TaskClassifier::classify("Handle customer complaint");
        assert_eq!(kind, TaskKind::CustomerSupport);
    }

    #[test]
    fn test_classify_code_analysis() {
        let kind = TaskClassifier::classify("Debug this code");
        assert_eq!(kind, TaskKind::CodeAnalysis);
    }

    #[test]
    fn test_classify_data_extraction() {
        let kind = TaskClassifier::classify("Extract data from forms");
        assert_eq!(kind, TaskKind::DataExtraction);
    }

    #[test]
    fn test_classify_summarization() {
        let kind = TaskClassifier::classify("Summarize this report");
        assert_eq!(kind, TaskKind::Summarization);
    }

    #[test]
    fn test_classify_unknown() {
        let kind = TaskClassifier::classify("Some random text");
        assert_eq!(kind, TaskKind::Unknown);
    }
}

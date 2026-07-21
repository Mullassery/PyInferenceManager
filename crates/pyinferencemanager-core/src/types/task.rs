use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TaskKind {
    DocumentAnalysis,
    QuestionAnswering,
    CustomerSupport,
    CodeAnalysis,
    DataExtraction,
    Summarization,
    Unknown,
}

impl Default for TaskKind {
    fn default() -> Self {
        TaskKind::Unknown
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PrivacyLevel {
    Low,
    High,
}

impl Default for PrivacyLevel {
    fn default() -> Self {
        PrivacyLevel::Low
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskOptions {
    pub privacy: PrivacyLevel,
    pub max_cloud_tokens: Option<u32>,
    pub timeout_ms: Option<u64>,
    pub preferred_speed: f32,
}

impl Default for TaskOptions {
    fn default() -> Self {
        TaskOptions {
            privacy: PrivacyLevel::Low,
            max_cloud_tokens: Some(4096),
            timeout_ms: Some(30_000),
            preferred_speed: 0.5,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AttachmentKind {
    File,
    RawText,
    Url,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attachment {
    pub kind: AttachmentKind,
    pub content: Vec<u8>,
    pub mime_type: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub description: String,
    pub kind: TaskKind,
    pub options: TaskOptions,
    pub attachments: Vec<Attachment>,
    pub created_at: DateTime<Utc>,
}

impl Task {
    pub fn new(description: String) -> Self {
        Task {
            id: Uuid::new_v4().to_string(),
            description,
            kind: TaskKind::Unknown,
            options: TaskOptions::default(),
            attachments: Vec::new(),
            created_at: Utc::now(),
        }
    }

    pub fn with_kind(mut self, kind: TaskKind) -> Self {
        self.kind = kind;
        self
    }

    pub fn with_options(mut self, options: TaskOptions) -> Self {
        self.options = options;
        self
    }

    pub fn with_attachment(mut self, attachment: Attachment) -> Self {
        self.attachments.push(attachment);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_new() {
        let task = Task::new("Test task".to_string());
        assert_eq!(task.description, "Test task");
        assert_eq!(task.kind, TaskKind::Unknown);
        assert_eq!(task.options.privacy, PrivacyLevel::Low);
    }

    #[test]
    fn test_task_with_kind() {
        let task = Task::new("Test".to_string()).with_kind(TaskKind::DocumentAnalysis);
        assert_eq!(task.kind, TaskKind::DocumentAnalysis);
    }

    #[test]
    fn test_task_uuid_uniqueness() {
        let task1 = Task::new("Test".to_string());
        let task2 = Task::new("Test".to_string());
        assert_ne!(task1.id, task2.id);
    }

    #[test]
    fn test_privacy_level_default() {
        assert_eq!(PrivacyLevel::default(), PrivacyLevel::Low);
    }

    #[test]
    fn test_task_kind_default() {
        assert_eq!(TaskKind::default(), TaskKind::Unknown);
    }

    #[test]
    fn test_task_serialization() {
        let task = Task::new("Test".to_string());
        let json = serde_json::to_string(&task).unwrap();
        let deserialized: Task = serde_json::from_str(&json).unwrap();
        assert_eq!(task.description, deserialized.description);
        assert_eq!(task.kind, deserialized.kind);
    }
}

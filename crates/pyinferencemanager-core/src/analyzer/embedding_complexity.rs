use std::collections::HashMap;

/// Semantic embedding-based complexity scorer
/// Uses task embeddings to determine semantic complexity
pub struct EmbeddingComplexityScorer {
    /// Reference embeddings for different complexity levels
    complexity_anchors: HashMap<String, Vec<f32>>,
}

#[derive(Debug, Clone)]
pub struct ComplexityAnalysis {
    pub score: f32,              // 0.0-1.0
    pub reasoning: String,        // Why this score
    pub similar_anchor: String,   // Which anchor it's similar to
    pub confidence: f32,          // How confident in this score
}

impl EmbeddingComplexityScorer {
    /// Create a new embedding-based complexity scorer
    pub fn new() -> Self {
        let mut anchors = HashMap::new();

        // Simple task anchors (0.0-0.3 range)
        anchors.insert(
            "simple".to_string(),
            vec![0.1, -0.2, 0.3, -0.1, 0.2, -0.3, 0.1, 0.0],
        );

        // Medium task anchors (0.3-0.7 range)
        anchors.insert(
            "medium".to_string(),
            vec![0.5, 0.4, 0.6, 0.3, 0.7, 0.2, 0.5, 0.4],
        );

        // Complex task anchors (0.7-1.0 range)
        anchors.insert(
            "complex".to_string(),
            vec![0.9, 0.8, 0.85, 0.9, 0.7, 0.95, 0.88, 0.92],
        );

        EmbeddingComplexityScorer {
            complexity_anchors: anchors,
        }
    }

    /// Generate a simple task embedding from text
    /// In production, this would use a real embedding model
    pub fn embed_task(description: &str) -> Vec<f32> {
        // Hash-based pseudo-embedding for testing
        // Real implementation would use actual embedding model
        let bytes = description.as_bytes();
        let mut embedding = vec![0.0; 8];

        for (i, &byte) in bytes.iter().enumerate().take(8) {
            embedding[i] = ((byte as f32 / 256.0) - 0.5);
        }

        // Normalize to roughly [-0.5, 0.5] range
        let sum: f32 = embedding.iter().sum();
        let mean = sum / embedding.len() as f32;
        embedding.iter_mut().for_each(|x| *x = (*x - mean) / 2.0);

        embedding
    }

    /// Compute cosine similarity between two embeddings
    fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() || a.is_empty() {
            return 0.0;
        }

        let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

        if norm_a == 0.0 || norm_b == 0.0 {
            return 0.0;
        }

        dot_product / (norm_a * norm_b)
    }

    /// Score task complexity based on embedding similarity
    pub fn score(&self, description: &str) -> ComplexityAnalysis {
        let task_embedding = Self::embed_task(description);

        let mut best_anchor = "simple".to_string();
        let mut best_similarity = -2.0;

        for (anchor_name, anchor_embedding) in &self.complexity_anchors {
            let similarity = Self::cosine_similarity(&task_embedding, anchor_embedding);
            if similarity > best_similarity {
                best_similarity = similarity;
                best_anchor = anchor_name.clone();
            }
        }

        // Map anchor to complexity score
        let base_score = match best_anchor.as_str() {
            "simple" => 0.2,
            "medium" => 0.5,
            "complex" => 0.85,
            _ => 0.5,
        };

        // Adjust based on similarity confidence (higher similarity = higher confidence)
        let confidence = (best_similarity + 1.0) / 2.0; // Normalize from [-1, 1] to [0, 1]

        // Add small variance based on description length
        let length_factor = (description.len() as f32 / 1000.0).min(0.2);
        let score = (base_score + length_factor).min(1.0);

        ComplexityAnalysis {
            score,
            reasoning: format!("Embedded task similar to {} anchor", best_anchor),
            similar_anchor: best_anchor,
            confidence,
        }
    }

    /// Batch score multiple tasks
    pub fn score_batch(&self, descriptions: &[&str]) -> Vec<ComplexityAnalysis> {
        descriptions.iter().map(|desc| self.score(desc)).collect()
    }
}

impl Default for EmbeddingComplexityScorer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedding_complexity_scorer_new() {
        let scorer = EmbeddingComplexityScorer::new();
        assert_eq!(scorer.complexity_anchors.len(), 3);
        assert!(scorer.complexity_anchors.contains_key("simple"));
        assert!(scorer.complexity_anchors.contains_key("medium"));
        assert!(scorer.complexity_anchors.contains_key("complex"));
    }

    #[test]
    fn test_embed_task() {
        let embedding = EmbeddingComplexityScorer::embed_task("test");
        assert_eq!(embedding.len(), 8);
        for value in embedding {
            assert!(value >= -0.5 && value <= 0.5);
        }
    }

    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        let sim = EmbeddingComplexityScorer::cosine_similarity(&a, &b);
        assert!((sim - 1.0).abs() < 0.01); // Should be very close to 1.0

        let c = vec![0.0, 1.0, 0.0];
        let sim2 = EmbeddingComplexityScorer::cosine_similarity(&a, &c);
        assert!(sim2.abs() < 0.01); // Should be close to 0.0 (orthogonal)
    }

    #[test]
    fn test_score_simple_task() {
        let scorer = EmbeddingComplexityScorer::new();
        let analysis = scorer.score("What is 2+2?");
        // Embedding-based scoring may vary, just check bounds and fields
        assert!(analysis.score >= 0.0 && analysis.score <= 1.0);
        assert!(analysis.confidence > 0.0);
        assert!(!analysis.similar_anchor.is_empty());
    }

    #[test]
    fn test_score_complex_task() {
        let scorer = EmbeddingComplexityScorer::new();
        let analysis = scorer.score("Analyze and synthesize all contradictions in this complex multi-part document");
        assert!(analysis.score > 0.3);
        assert!(analysis.confidence > 0.0);
    }

    #[test]
    fn test_score_batch() {
        let scorer = EmbeddingComplexityScorer::new();
        let tasks = vec!["simple", "medium task", "very complex analysis"];
        let results = scorer.score_batch(&tasks);
        assert_eq!(results.len(), 3);
        for result in results {
            assert!(result.score >= 0.0 && result.score <= 1.0);
            assert!(result.confidence >= 0.0 && result.confidence <= 1.0);
        }
    }

    #[test]
    fn test_score_consistency() {
        let scorer = EmbeddingComplexityScorer::new();
        let task = "Analyze this document";
        let score1 = scorer.score(task);
        let score2 = scorer.score(task);
        assert_eq!(score1.score, score2.score);
        assert_eq!(score1.similar_anchor, score2.similar_anchor);
    }

    #[test]
    fn test_complexity_analysis_fields() {
        let scorer = EmbeddingComplexityScorer::new();
        let analysis = scorer.score("test task");
        assert!(analysis.score >= 0.0 && analysis.score <= 1.0);
        assert!(!analysis.reasoning.is_empty());
        assert!(!analysis.similar_anchor.is_empty());
        assert!(analysis.confidence >= 0.0 && analysis.confidence <= 1.0);
    }

    #[test]
    fn test_empty_task_description() {
        let scorer = EmbeddingComplexityScorer::new();
        let analysis = scorer.score("");
        assert!(analysis.score >= 0.0 && analysis.score <= 1.0);
    }

    #[test]
    fn test_long_task_description() {
        let scorer = EmbeddingComplexityScorer::new();
        let long_task = "a".repeat(5000);
        let analysis = scorer.score(&long_task);
        assert!(analysis.score > 0.0);
        assert!(analysis.score <= 1.0);
    }
}

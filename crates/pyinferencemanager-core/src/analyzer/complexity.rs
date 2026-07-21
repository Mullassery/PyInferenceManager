pub struct ComplexityScorer;

impl ComplexityScorer {
    pub fn score(description: &str, attachment_size: usize) -> f32 {
        let mut score = 0.0_f32;

        // Length signal: longer descriptions often indicate complexity
        let words = description.split_whitespace().count();
        score += (words as f32 / 500.0).min(0.25);

        // Attachment signal: large files = more processing needed
        if attachment_size > 1_000_000 {
            score += 0.3;
        } else if attachment_size > 100_000 {
            score += 0.2;
        } else if attachment_size > 10_000 {
            score += 0.1;
        }

        let desc_lower = description.to_lowercase();

        // Multi-word phrases indicating high complexity
        let complex_phrases = [
            "across all", "compare", "contradictions", "analyze", "synthesize",
            "pattern", "trend", "relationship", "correlation", "root cause",
            "evaluate", "assess", "reasoning", "critical analysis",
        ];

        for phrase in &complex_phrases {
            if desc_lower.contains(phrase) {
                score += 0.15;
            }
        }

        // Simple queries
        let simple_patterns = [
            "what is", "define", "list", "where is", "when was",
            "who is", "how many", "give me",
        ];

        for pattern in &simple_patterns {
            if desc_lower.contains(pattern) {
                score -= 0.1;
            }
        }

        // Multi-part complexity: conjunctions + multiple sentences
        let commas = description.matches(',').count();
        let semicolons = description.matches(';').count();
        if commas + semicolons > 3 {
            score += 0.2;
        }

        score.clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_complexity_score_low() {
        let score = ComplexityScorer::score("What is the invoice number?", 0);
        assert!(score < 0.5);
    }

    #[test]
    fn test_complexity_score_high() {
        let long_desc = "Analyze and synthesize contradictions and reasons across this document";
        let score = ComplexityScorer::score(long_desc, 0);
        assert!(score > 0.3);
    }

    #[test]
    fn test_complexity_with_small_attachment() {
        let score = ComplexityScorer::score("Analyze document", 10_000);
        assert!(score > 0.1);
    }

    #[test]
    fn test_complexity_with_large_attachment() {
        let score = ComplexityScorer::score("Analyze", 1_000_000);
        assert!(score > 0.3);
    }

    #[test]
    fn test_complexity_multi_sentence() {
        let desc = "First question. Second query; third point, and fourth thing here";
        let score = ComplexityScorer::score(desc, 0);
        assert!(score > 0.0);
    }

    #[test]
    fn test_complexity_simple_query() {
        let score = ComplexityScorer::score("How many items?", 0);
        assert!(score < 0.3);
    }

    #[test]
    fn test_complexity_analytical() {
        let desc = "Identify patterns and correlations; assess the root cause of this trend";
        let score = ComplexityScorer::score(desc, 0);
        assert!(score > 0.5);
    }
}

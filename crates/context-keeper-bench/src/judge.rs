//! LLM-as-Judge for answer-level evaluation.
//!
//! Sends the question, gold answer, and predicted answer to an LLM and asks
//! it to score the prediction on a 0.0–1.0 scale. Follows the approach used
//! by Mem0 and LongMemEval evaluations.

use rig::client::CompletionClient;
use rig::completion::Prompt;
use rig::providers::openai;

use crate::config::ProviderConfig;

const JUDGE_PROMPT: &str = "\
You are an expert evaluator scoring a memory system's answers against gold-standard references.

Given:
- A question asked to the memory system
- The gold (correct) answer
- The predicted answer from the memory system

Score the predicted answer on a scale from 0.0 to 1.0:
- 1.0 = Predicted answer is fully correct and captures the essential meaning of the gold answer
- 0.75 = Mostly correct with minor omissions or extra but non-contradictory info
- 0.5 = Partially correct; captures some key facts but misses important ones
- 0.25 = Minimally correct; only tangentially related to the gold answer
- 0.0 = Completely wrong, contradicts the gold answer, or is irrelevant

Focus on semantic correctness, not exact wording. Paraphrases that convey the same meaning should score 1.0.

Return ONLY a JSON object: {\"score\": <float>, \"reasoning\": \"<one sentence>\"}
No markdown, no explanation — just the JSON object.";

/// An LLM-backed answer judge.
pub struct LlmJudge {
    client: openai::Client,
    model: String,
}

impl LlmJudge {
    pub fn from_provider(provider: &ProviderConfig) -> Self {
        let client = openai::Client::builder()
            .base_url(&provider.api_url)
            .api_key(&provider.api_key)
            .build()
            .expect("Failed to create OpenAI client for judge");

        Self {
            client,
            model: provider.extraction_model.clone(),
        }
    }

    /// Score a predicted answer against a gold answer for a given question.
    /// Returns a score in [0.0, 1.0], or None if the LLM call fails.
    pub async fn score(&self, question: &str, gold_answer: &str, predicted: &str) -> Option<f64> {
        let user_msg = format!(
            "Question: {question}\nGold answer: {gold_answer}\nPredicted answer: {predicted}"
        );

        let agent = self
            .client
            .agent(&self.model)
            .preamble(JUDGE_PROMPT)
            .build();

        let raw: String = match agent.prompt(user_msg.as_str()).await {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(error = %e, "LLM judge call failed");
                return None;
            }
        };

        parse_judge_score(&raw)
    }
}

fn parse_judge_score(raw: &str) -> Option<f64> {
    let trimmed = raw.trim();

    // Try JSON parse first
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(trimmed) {
        if let Some(score) = v.get("score").and_then(|s| s.as_f64()) {
            return Some(score.clamp(0.0, 1.0));
        }
    }

    // Fallback: extract JSON from markdown fences or preamble
    let json_str = if let (Some(start), Some(end)) = (trimmed.find('{'), trimmed.rfind('}')) {
        &trimmed[start..=end]
    } else {
        return None;
    };

    if let Ok(v) = serde_json::from_str::<serde_json::Value>(json_str) {
        if let Some(score) = v.get("score").and_then(|s| s.as_f64()) {
            return Some(score.clamp(0.0, 1.0));
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_clean_json() {
        let raw = r#"{"score": 0.85, "reasoning": "Mostly correct"}"#;
        assert!((parse_judge_score(raw).unwrap() - 0.85).abs() < 1e-9);
    }

    #[test]
    fn parse_with_preamble() {
        let raw = "Here is my evaluation:\n{\"score\": 0.5, \"reasoning\": \"Partial match\"}";
        assert!((parse_judge_score(raw).unwrap() - 0.5).abs() < 1e-9);
    }

    #[test]
    fn parse_markdown_fenced() {
        let raw = "```json\n{\"score\": 1.0, \"reasoning\": \"Perfect\"}\n```";
        assert!((parse_judge_score(raw).unwrap() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn parse_clamps_out_of_range() {
        let raw = r#"{"score": 1.5, "reasoning": "Over-scored"}"#;
        assert!((parse_judge_score(raw).unwrap() - 1.0).abs() < 1e-9);

        let raw = r#"{"score": -0.5, "reasoning": "Under-scored"}"#;
        assert!((parse_judge_score(raw).unwrap() - 0.0).abs() < 1e-9);
    }

    #[test]
    fn parse_missing_score() {
        let raw = r#"{"reasoning": "No score"}"#;
        assert!(parse_judge_score(raw).is_none());
    }

    #[test]
    fn parse_garbage() {
        assert!(parse_judge_score("not json at all").is_none());
    }
}

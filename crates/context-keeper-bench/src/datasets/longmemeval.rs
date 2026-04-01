use std::path::Path;

use serde::Deserialize;

use crate::config::{BehavioralStep, Operation, ScenarioConfig};

/// A LongMemEval dataset entry with multi-session conversations and temporal questions.
///
/// Expected format (from the LongMemEval paper):
/// ```json
/// {
///   "id": "lme_001",
///   "sessions": [
///     {
///       "session_id": 1,
///       "messages": [
///         { "role": "user", "content": "..." },
///         { "role": "assistant", "content": "..." }
///       ]
///     }
///   ],
///   "questions": [
///     {
///       "question": "...",
///       "gold_answer": "...",
///       "category": "temporal_reasoning"
///     }
///   ]
/// }
/// ```
#[derive(Debug, Deserialize)]
pub struct LongMemEvalEntry {
    pub id: String,
    pub sessions: Vec<Session>,
    #[serde(default)]
    pub questions: Vec<Question>,
}

#[derive(Debug, Deserialize)]
pub struct Session {
    pub session_id: u32,
    pub messages: Vec<Message>,
}

#[derive(Debug, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Deserialize)]
pub struct Question {
    pub question: String,
    pub gold_answer: String,
    #[serde(default)]
    pub category: Option<String>,
}

/// Load a LongMemEval JSON file and convert each entry into a behavioral scenario.
///
/// Each entry becomes a scenario with:
/// 1. One `ingest` step per session (messages concatenated)
/// 2. One `search` step per question
///
/// The `category` field can be used to filter for temporal reasoning subsets.
pub fn load(path: &Path) -> anyhow::Result<Vec<ScenarioConfig>> {
    let raw = std::fs::read_to_string(path)?;
    let entries: Vec<LongMemEvalEntry> = serde_json::from_str(&raw)?;

    let mut scenarios = Vec::with_capacity(entries.len());
    for entry in &entries {
        let mut steps = Vec::new();

        for session in &entry.sessions {
            let text: String = session
                .messages
                .iter()
                .map(|m| format!("{}: {}", m.role, m.content))
                .collect::<Vec<_>>()
                .join("\n");

            steps.push(BehavioralStep::Ingest {
                text,
                source: Some(format!("longmemeval/session_{}", session.session_id)),
            });
        }

        for q in &entry.questions {
            let answer_tokens: Vec<String> = q
                .gold_answer
                .split_whitespace()
                .filter(|w| w.len() > 2 && w.chars().next().is_some_and(|c| c.is_uppercase()))
                .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()).to_string())
                .filter(|w| !w.is_empty())
                .collect();

            steps.push(BehavioralStep::Search {
                query: q.question.clone(),
                expected_entities: answer_tokens,
                unexpected_entities: vec![],
            });
        }

        let category_suffix = entry
            .questions
            .first()
            .and_then(|q| q.category.as_deref())
            .unwrap_or("general");

        scenarios.push(ScenarioConfig {
            name: format!("lme_{}_{}", entry.id, category_suffix),
            operation: Operation::Behavioral,
            iterations: 1,
            inputs: vec![],
            steps,
        });
    }

    Ok(scenarios)
}

/// Load only temporal reasoning questions from a LongMemEval dataset.
pub fn load_temporal_subset(path: &Path) -> anyhow::Result<Vec<ScenarioConfig>> {
    let all = load(path)?;
    Ok(all
        .into_iter()
        .filter(|s| s.name.contains("temporal"))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_longmemeval_entry() {
        let json = r#"[{
            "id": "lme_001",
            "sessions": [
                {
                    "session_id": 1,
                    "messages": [
                        { "role": "user", "content": "I started a new job at TechCo last week." },
                        { "role": "assistant", "content": "Congratulations! What role?" }
                    ]
                }
            ],
            "questions": [
                {
                    "question": "Where did the user start working?",
                    "gold_answer": "TechCo",
                    "category": "single_session"
                }
            ]
        }]"#;

        let entries: Vec<LongMemEvalEntry> = serde_json::from_str(json).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].sessions.len(), 1);
        assert_eq!(entries[0].questions[0].gold_answer, "TechCo");
    }

    #[test]
    fn convert_to_scenario() {
        let json = r#"[{
            "id": "lme_001",
            "sessions": [
                {
                    "session_id": 1,
                    "messages": [
                        { "role": "user", "content": "Alice joined BigCo as CTO." }
                    ]
                }
            ],
            "questions": [
                {
                    "question": "What role does Alice have?",
                    "gold_answer": "CTO at BigCo",
                    "category": "temporal_reasoning"
                }
            ]
        }]"#;

        let tmp = std::env::temp_dir().join("ck_test_longmemeval.json");
        std::fs::write(&tmp, json).unwrap();

        let scenarios = load(&tmp).unwrap();
        assert_eq!(scenarios.len(), 1);
        assert!(scenarios[0].name.contains("temporal_reasoning"));
        assert_eq!(scenarios[0].steps.len(), 2);

        let temporal = load_temporal_subset(&tmp).unwrap();
        assert_eq!(temporal.len(), 1);

        let _ = std::fs::remove_file(&tmp);
    }
}

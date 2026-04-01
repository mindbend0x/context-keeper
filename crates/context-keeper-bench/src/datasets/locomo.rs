use std::path::Path;

use serde::Deserialize;

use crate::config::{BehavioralStep, Operation, ScenarioConfig};

/// A LoCoMo dataset entry with multi-session conversations and Q&A pairs.
///
/// Expected format (from the LoCoMo paper's public dataset):
/// ```json
/// {
///   "conversation_id": "conv_001",
///   "sessions": [
///     {
///       "session_id": 1,
///       "turns": [
///         { "role": "user", "content": "..." },
///         { "role": "assistant", "content": "..." }
///       ]
///     }
///   ],
///   "questions": [
///     {
///       "question": "Where does Alice work?",
///       "answer": "Acme Corp",
///       "reasoning_type": "single_session"
///     }
///   ]
/// }
/// ```
#[derive(Debug, Deserialize)]
pub struct LoCoMoEntry {
    pub conversation_id: String,
    pub sessions: Vec<Session>,
    #[serde(default)]
    pub questions: Vec<Question>,
}

#[derive(Debug, Deserialize)]
pub struct Session {
    pub session_id: u32,
    pub turns: Vec<Turn>,
}

#[derive(Debug, Deserialize)]
pub struct Turn {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Deserialize)]
pub struct Question {
    pub question: String,
    pub answer: String,
    #[serde(default)]
    pub reasoning_type: Option<String>,
}

/// Load a LoCoMo JSON file and convert each conversation into a behavioral scenario.
///
/// Each conversation becomes a scenario with:
/// 1. One `ingest` step per session (all turns concatenated)
/// 2. One `search` step per question
pub fn load(path: &Path) -> anyhow::Result<Vec<ScenarioConfig>> {
    let raw = std::fs::read_to_string(path)?;
    let entries: Vec<LoCoMoEntry> = serde_json::from_str(&raw)?;

    let mut scenarios = Vec::with_capacity(entries.len());
    for entry in &entries {
        let mut steps = Vec::new();

        for session in &entry.sessions {
            let text: String = session
                .turns
                .iter()
                .map(|t| format!("{}: {}", t.role, t.content))
                .collect::<Vec<_>>()
                .join("\n");

            steps.push(BehavioralStep::Ingest {
                text,
                source: Some(format!("locomo/session_{}", session.session_id)),
            });
        }

        for q in &entry.questions {
            let answer_tokens: Vec<String> = q
                .answer
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

        scenarios.push(ScenarioConfig {
            name: format!("locomo_{}", entry.conversation_id),
            operation: Operation::Behavioral,
            iterations: 1,
            inputs: vec![],
            steps,
        });
    }

    Ok(scenarios)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_locomo_entry() {
        let json = r#"[{
            "conversation_id": "test_001",
            "sessions": [
                {
                    "session_id": 1,
                    "turns": [
                        { "role": "user", "content": "Alice works at Acme Corp." },
                        { "role": "assistant", "content": "Got it, noted." }
                    ]
                },
                {
                    "session_id": 2,
                    "turns": [
                        { "role": "user", "content": "Alice got promoted to VP." }
                    ]
                }
            ],
            "questions": [
                {
                    "question": "What is Alice's role?",
                    "answer": "VP at Acme Corp",
                    "reasoning_type": "multi_session"
                }
            ]
        }]"#;

        let entries: Vec<LoCoMoEntry> = serde_json::from_str(json).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].sessions.len(), 2);
        assert_eq!(entries[0].questions.len(), 1);
    }

    #[test]
    fn convert_to_scenario() {
        let json = r#"[{
            "conversation_id": "conv_001",
            "sessions": [
                {
                    "session_id": 1,
                    "turns": [
                        { "role": "user", "content": "Alice works at Acme." }
                    ]
                }
            ],
            "questions": [
                {
                    "question": "Where does Alice work?",
                    "answer": "Acme",
                    "reasoning_type": "single_session"
                }
            ]
        }]"#;

        let tmp = std::env::temp_dir().join("ck_test_locomo.json");
        std::fs::write(&tmp, json).unwrap();

        let scenarios = load(&tmp).unwrap();
        assert_eq!(scenarios.len(), 1);
        assert_eq!(scenarios[0].operation, Operation::Behavioral);
        assert_eq!(scenarios[0].steps.len(), 2);

        match &scenarios[0].steps[0] {
            BehavioralStep::Ingest { text, .. } => {
                assert!(text.contains("Alice works at Acme"));
            }
            _ => panic!("expected ingest"),
        }

        match &scenarios[0].steps[1] {
            BehavioralStep::Search { query, expected_entities, .. } => {
                assert_eq!(query, "Where does Alice work?");
                assert!(expected_entities.contains(&"Acme".to_string()));
            }
            _ => panic!("expected search"),
        }

        let _ = std::fs::remove_file(&tmp);
    }
}

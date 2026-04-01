use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// Top-level benchmark configuration loaded from a YAML file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchConfig {
    pub providers: Vec<ProviderConfig>,
    pub scenarios: Vec<ScenarioConfig>,
    #[serde(default)]
    pub settings: Settings,
}

/// An LLM provider endpoint and its model configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub name: String,
    pub api_url: String,
    pub api_key: String,
    pub extraction_model: String,
    pub embedding_model: String,
    #[serde(default = "default_embedding_dims")]
    pub embedding_dims: usize,
}

fn default_embedding_dims() -> usize {
    1536
}

/// A single benchmark scenario describing what to test and how many times.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioConfig {
    pub name: String,
    pub operation: Operation,
    #[serde(default = "default_iterations")]
    pub iterations: usize,
    #[serde(default)]
    pub inputs: Vec<BenchInput>,
    #[serde(default)]
    pub steps: Vec<BehavioralStep>,
}

fn default_iterations() -> usize {
    3
}

/// A benchmark input: either a plain string or a detailed input with ground truth.
///
/// Plain strings are backward-compatible with old configs. Detailed inputs carry
/// optional expected entities, entity types, and relations for quality scoring.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum BenchInput {
    Simple(String),
    Detailed(DetailedInput),
}

impl BenchInput {
    pub fn text(&self) -> &str {
        match self {
            Self::Simple(s) => s,
            Self::Detailed(d) => &d.text,
        }
    }

    pub fn has_ground_truth(&self) -> bool {
        match self {
            Self::Simple(_) => false,
            Self::Detailed(d) => {
                !d.expected_entities.is_empty() || !d.expected_relations.is_empty()
            }
        }
    }

    pub fn as_detailed(&self) -> Option<&DetailedInput> {
        match self {
            Self::Simple(_) => None,
            Self::Detailed(d) => Some(d),
        }
    }
}

/// A benchmark input with optional ground-truth expectations for quality scoring.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetailedInput {
    pub text: String,
    #[serde(default)]
    pub expected_entities: Vec<String>,
    #[serde(default)]
    pub expected_entity_types: HashMap<String, String>,
    #[serde(default)]
    pub expected_relations: Vec<ExpectedRelation>,
}

/// A ground-truth relation triple for quality scoring.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct ExpectedRelation {
    pub subject: String,
    pub predicate: String,
    pub object: String,
}

/// The operation type a scenario exercises.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Operation {
    EntityExtraction,
    RelationExtraction,
    Ingestion,
    Search,
    QueryRewriting,
    Behavioral,
}

impl std::fmt::Display for Operation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EntityExtraction => write!(f, "entity_extraction"),
            Self::RelationExtraction => write!(f, "relation_extraction"),
            Self::Ingestion => write!(f, "ingestion"),
            Self::Search => write!(f, "search"),
            Self::QueryRewriting => write!(f, "query_rewriting"),
            Self::Behavioral => write!(f, "behavioral"),
        }
    }
}

/// Global benchmark settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default = "default_storage_backend")]
    pub storage_backend: String,
    #[serde(default = "default_warmup")]
    pub warmup_iterations: usize,
}

/// A step in a behavioral scenario: either ingest data or verify search results.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum BehavioralStep {
    Ingest {
        text: String,
        source: Option<String>,
    },
    Search {
        query: String,
        #[serde(default)]
        expected_entities: Vec<String>,
        #[serde(default)]
        unexpected_entities: Vec<String>,
    },
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            storage_backend: default_storage_backend(),
            warmup_iterations: default_warmup(),
        }
    }
}

fn default_storage_backend() -> String {
    "memory".to_string()
}

fn default_warmup() -> usize {
    1
}

/// Load a benchmark config from a YAML file, expanding `${VAR}` references
/// against the process environment.
pub fn load_config(path: &Path) -> anyhow::Result<BenchConfig> {
    let raw = std::fs::read_to_string(path)?;
    let expanded = expand_env_vars(&raw);
    let config: BenchConfig = serde_yaml_ng::from_str(&expanded)?;
    validate(&config)?;
    Ok(config)
}

/// Replace every `${VAR_NAME}` token with the corresponding environment variable.
/// Missing variables are replaced with an empty string.
fn expand_env_vars(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '$' && chars.peek() == Some(&'{') {
            chars.next(); // consume '{'
            let mut var_name = String::new();
            for c in chars.by_ref() {
                if c == '}' {
                    break;
                }
                var_name.push(c);
            }
            match std::env::var(&var_name) {
                Ok(val) => result.push_str(&val),
                Err(_) => tracing::warn!(var = %var_name, "env var not set, using empty string"),
            }
        } else {
            result.push(ch);
        }
    }

    result
}

fn validate(config: &BenchConfig) -> anyhow::Result<()> {
    anyhow::ensure!(!config.providers.is_empty(), "at least one provider required");
    anyhow::ensure!(
        !config.scenarios.is_empty(),
        "at least one scenario required"
    );
    for p in &config.providers {
        anyhow::ensure!(!p.name.is_empty(), "provider name must not be empty");
        anyhow::ensure!(!p.api_url.is_empty(), "provider '{}' has empty api_url", p.name);
        anyhow::ensure!(!p.api_key.is_empty(), "provider '{}' has empty api_key — set the env var", p.name);
    }
    for s in &config.scenarios {
        anyhow::ensure!(!s.name.is_empty(), "scenario name must not be empty");
        if s.operation == Operation::Behavioral {
            anyhow::ensure!(!s.steps.is_empty(), "behavioral scenario '{}' has no steps", s.name);
        } else {
            anyhow::ensure!(!s.inputs.is_empty(), "scenario '{}' has no inputs", s.name);
        }
        anyhow::ensure!(s.iterations > 0, "scenario '{}' needs at least 1 iteration", s.name);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expand_env_vars_replaces_set_var() {
        std::env::set_var("CK_BENCH_TEST_VAR", "hello");
        let result = expand_env_vars("prefix-${CK_BENCH_TEST_VAR}-suffix");
        assert_eq!(result, "prefix-hello-suffix");
    }

    #[test]
    fn expand_env_vars_missing_var_becomes_empty() {
        std::env::remove_var("CK_BENCH_MISSING");
        let result = expand_env_vars("before-${CK_BENCH_MISSING}-after");
        assert_eq!(result, "before--after");
    }

    #[test]
    fn expand_env_vars_no_placeholders_unchanged() {
        let input = "no placeholders here";
        assert_eq!(expand_env_vars(input), input);
    }

    #[test]
    fn deserialize_simple_inputs() {
        let yaml = r#"
providers:
  - name: test
    api_url: "http://localhost"
    api_key: "sk-test"
    extraction_model: "gpt-4o-mini"
    embedding_model: "text-embedding-3-small"
scenarios:
  - name: basic
    operation: entity_extraction
    inputs:
      - "Alice works at Acme."
"#;
        let config: BenchConfig = serde_yaml_ng::from_str(yaml).unwrap();
        assert_eq!(config.providers.len(), 1);
        assert_eq!(config.scenarios[0].iterations, 3);
        assert_eq!(config.scenarios[0].inputs[0].text(), "Alice works at Acme.");
        assert!(!config.scenarios[0].inputs[0].has_ground_truth());
    }

    #[test]
    fn deserialize_detailed_inputs() {
        let yaml = r#"
providers:
  - name: test
    api_url: "http://localhost"
    api_key: "sk-test"
    extraction_model: "gpt-4o-mini"
    embedding_model: "text-embedding-3-small"
scenarios:
  - name: quality
    operation: entity_extraction
    inputs:
      - text: "Alice works at Acme Corp."
        expected_entities: ["Alice", "Acme Corp"]
        expected_entity_types:
          Alice: person
          Acme Corp: organization
        expected_relations:
          - { subject: Alice, predicate: works_at, object: Acme Corp }
"#;
        let config: BenchConfig = serde_yaml_ng::from_str(yaml).unwrap();
        let input = &config.scenarios[0].inputs[0];
        assert_eq!(input.text(), "Alice works at Acme Corp.");
        assert!(input.has_ground_truth());

        let detailed = input.as_detailed().unwrap();
        assert_eq!(detailed.expected_entities.len(), 2);
        assert_eq!(detailed.expected_entity_types.get("Alice").unwrap(), "person");
        assert_eq!(detailed.expected_relations.len(), 1);
        assert_eq!(detailed.expected_relations[0].predicate, "works_at");
    }

    #[test]
    fn deserialize_mixed_inputs() {
        let yaml = r#"
providers:
  - name: test
    api_url: "http://localhost"
    api_key: "sk-test"
    extraction_model: "m"
    embedding_model: "e"
scenarios:
  - name: mixed
    operation: entity_extraction
    inputs:
      - "Plain text input."
      - text: "Detailed input."
        expected_entities: ["Foo"]
"#;
        let config: BenchConfig = serde_yaml_ng::from_str(yaml).unwrap();
        assert!(!config.scenarios[0].inputs[0].has_ground_truth());
        assert!(config.scenarios[0].inputs[1].has_ground_truth());
    }

    #[test]
    fn validate_rejects_empty_providers() {
        let config = BenchConfig {
            providers: vec![],
            scenarios: vec![ScenarioConfig {
                name: "x".into(),
                operation: Operation::EntityExtraction,
                iterations: 1,
                inputs: vec![BenchInput::Simple("text".into())],
                steps: vec![],
            }],
            settings: Settings::default(),
        };
        assert!(validate(&config).is_err());
    }

    #[test]
    fn deserialize_behavioral_scenario() {
        let yaml = r#"
providers:
  - name: test
    api_url: "http://localhost"
    api_key: "sk-test"
    extraction_model: "m"
    embedding_model: "e"
scenarios:
  - name: negation_test
    operation: behavioral
    iterations: 1
    steps:
      - action: ingest
        text: "Alice works at Acme Corp."
      - action: search
        query: "Where does Alice work?"
        expected_entities: ["Alice", "Acme Corp"]
      - action: ingest
        text: "Alice left Acme Corp."
      - action: search
        query: "Where does Alice work?"
        expected_entities: ["Alice"]
        unexpected_entities: ["Acme Corp"]
"#;
        let config: BenchConfig = serde_yaml_ng::from_str(yaml).unwrap();
        assert_eq!(config.scenarios[0].operation, Operation::Behavioral);
        assert_eq!(config.scenarios[0].steps.len(), 4);

        match &config.scenarios[0].steps[0] {
            BehavioralStep::Ingest { text, .. } => {
                assert_eq!(text, "Alice works at Acme Corp.");
            }
            _ => panic!("expected ingest step"),
        }

        match &config.scenarios[0].steps[1] {
            BehavioralStep::Search {
                query,
                expected_entities,
                unexpected_entities,
            } => {
                assert_eq!(query, "Where does Alice work?");
                assert_eq!(expected_entities.len(), 2);
                assert!(unexpected_entities.is_empty());
            }
            _ => panic!("expected search step"),
        }

        match &config.scenarios[0].steps[3] {
            BehavioralStep::Search {
                unexpected_entities,
                ..
            } => {
                assert_eq!(unexpected_entities, &["Acme Corp"]);
            }
            _ => panic!("expected search step"),
        }
    }
}

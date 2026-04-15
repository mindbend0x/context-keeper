use std::collections::HashSet;

pub struct Scenario {
    pub name: &'static str,
    pub episodes: Vec<(&'static str, &'static str)>,
    pub expected_entities: Vec<&'static str>,
    pub expected_relations: Vec<(&'static str, &'static str)>,
    pub queries: Vec<QueryExpectation>,
}

pub struct QueryExpectation {
    pub query: &'static str,
    pub relevant_entity_names: Vec<&'static str>,
    pub min_recall_at_5: f64,
}

impl Scenario {
    pub fn expected_entity_set(&self) -> HashSet<String> {
        self.expected_entities
            .iter()
            .map(|s| s.to_string())
            .collect()
    }
}

/// People & organizations scenario.
///
/// MockEntityExtractor picks up capitalized words > 1 char that are fully
/// alphanumeric. "Corp." has a period so it's filtered. "CTO"/"CEO" are
/// uppercase abbreviations that pass the filter.
pub fn people_and_orgs() -> Scenario {
    Scenario {
        name: "people_and_orgs",
        episodes: vec![
            ("Alice is the CTO of Acme Corp and Bob is the CEO", "test"),
            ("They work together in Berlin on Project Alpha", "test"),
        ],
        expected_entities: vec![
            "Alice", "CTO", "Acme", "Bob", "CEO", "They", "Berlin", "Project", "Alpha",
        ],
        expected_relations: vec![
            ("Alice", "CTO"),
            ("CTO", "Acme"),
            ("Acme", "Bob"),
            ("Bob", "CEO"),
            ("They", "Berlin"),
            ("Berlin", "Project"),
            ("Project", "Alpha"),
        ],
        queries: vec![
            QueryExpectation {
                query: "Alice",
                relevant_entity_names: vec!["Alice"],
                min_recall_at_5: 1.0,
            },
            QueryExpectation {
                query: "Berlin",
                relevant_entity_names: vec!["Berlin"],
                min_recall_at_5: 1.0,
            },
        ],
    }
}

/// Technical domain scenario.
///
/// "Rust" and "SurrealDB" are capitalized alphanumeric. "LLVM" also qualifies.
/// Words like "uses", "for", "is", "in" are lowercase and filtered out.
pub fn technical_domain() -> Scenario {
    Scenario {
        name: "technical_domain",
        episodes: vec![
            (
                "Rust uses LLVM for code generation and optimization",
                "docs",
            ),
            (
                "SurrealDB is written in Rust and supports HNSW indexes",
                "docs",
            ),
        ],
        expected_entities: vec!["Rust", "LLVM", "SurrealDB", "HNSW"],
        expected_relations: vec![("Rust", "LLVM"), ("SurrealDB", "Rust"), ("Rust", "HNSW")],
        queries: vec![
            QueryExpectation {
                query: "Rust",
                relevant_entity_names: vec!["Rust"],
                min_recall_at_5: 1.0,
            },
            QueryExpectation {
                query: "SurrealDB",
                relevant_entity_names: vec!["SurrealDB"],
                min_recall_at_5: 1.0,
            },
        ],
    }
}

/// Overlapping context: same entities mentioned across episodes with evolving facts.
pub fn overlapping_context() -> Scenario {
    Scenario {
        name: "overlapping_context",
        episodes: vec![
            ("Alice works at Acme as an Engineer", "chat"),
            ("Alice was promoted to Director at Acme", "chat"),
            ("Bob joined Acme as a Manager", "chat"),
        ],
        expected_entities: vec!["Alice", "Acme", "Engineer", "Director", "Bob", "Manager"],
        expected_relations: vec![
            ("Alice", "Acme"),
            ("Alice", "Director"),
            ("Director", "Acme"),
            ("Bob", "Acme"),
            ("Acme", "Manager"),
        ],
        queries: vec![
            QueryExpectation {
                query: "Alice",
                relevant_entity_names: vec!["Alice"],
                min_recall_at_5: 1.0,
            },
            QueryExpectation {
                query: "Acme",
                relevant_entity_names: vec!["Acme"],
                min_recall_at_5: 1.0,
            },
        ],
    }
}

/// Sparse input: minimal text that may produce zero or one entity.
pub fn sparse_input() -> Scenario {
    Scenario {
        name: "sparse_input",
        episodes: vec![("Hello", "test"), ("ok", "test"), ("Go", "test")],
        expected_entities: vec!["Hello", "Go"],
        expected_relations: vec![],
        queries: vec![],
    }
}

/// Dense input: a paragraph packed with capitalized proper nouns.
pub fn dense_input() -> Scenario {
    Scenario {
        name: "dense_input",
        episodes: vec![(
            "Microsoft Azure and Amazon AWS compete with Google Cloud while Meta \
                 builds React and Apple ships Swift for iOS development alongside Oracle \
                 maintaining Java",
            "article",
        )],
        expected_entities: vec![
            "Microsoft",
            "Azure",
            "Amazon",
            "AWS",
            "Google",
            "Cloud",
            "Meta",
            "React",
            "Apple",
            "Swift",
            "Oracle",
            "Java",
        ],
        expected_relations: vec![
            ("Microsoft", "Azure"),
            ("Azure", "Amazon"),
            ("Amazon", "AWS"),
            ("AWS", "Google"),
            ("Google", "Cloud"),
            ("Cloud", "Meta"),
            ("Meta", "React"),
            ("React", "Apple"),
            ("Apple", "Swift"),
            ("Oracle", "Java"),
        ],
        queries: vec![
            QueryExpectation {
                query: "Microsoft",
                relevant_entity_names: vec!["Microsoft"],
                min_recall_at_5: 1.0,
            },
            QueryExpectation {
                query: "React",
                relevant_entity_names: vec!["React"],
                min_recall_at_5: 1.0,
            },
        ],
    }
}

/// Returns all scenarios for aggregate tests.
pub fn all_scenarios() -> Vec<Scenario> {
    vec![
        people_and_orgs(),
        technical_domain(),
        overlapping_context(),
        sparse_input(),
        dense_input(),
    ]
}

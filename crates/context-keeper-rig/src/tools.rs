//! Rig `Tool` and `ToolEmbedding` trait implementations for Context Keeper.
//!
//! These expose `add_memory`, `search`, and `expand_search` as agent-callable
//! tools, allowing Rig agents to interact with the knowledge graph.

use serde::{Deserialize, Serialize};

/// Input schema for the add_memory tool.
#[derive(Debug, Serialize, Deserialize)]
pub struct AddMemoryInput {
    pub text: String,
    pub source: String,
}

/// Input schema for the search_memory tool.
#[derive(Debug, Serialize, Deserialize)]
pub struct SearchMemoryInput {
    pub query: String,
    pub limit: Option<usize>,
}

/// Input schema for the expand_search tool.
#[derive(Debug, Serialize, Deserialize)]
pub struct ExpandSearchInput {
    pub query: String,
    pub limit: Option<usize>,
}

// TODO: When rig-core is added as a dependency, implement:
// impl Tool for AddMemoryTool { ... }
// impl Tool for SearchMemoryTool { ... }
// impl Tool for ExpandSearchTool { ... }

//! Long-Term Memory: End-to-end demo of the note-based memory system.
//!
//! Run with: cargo run --example long_term_memory
//!
//! No API keys required — uses mock embeddings throughout.

use anyhow::Result;
use chrono::Utc;
use context_keeper_core::{models::Note, traits::MockEmbedder, Embedder};
use context_keeper_surreal::{apply_schema, connect_memory, Repository, SurrealConfig};
use uuid::Uuid;

fn banner(title: &str) {
    let width: usize = 64;
    let pad = width.saturating_sub(title.len() + 4);
    let left = pad / 2;
    let right = pad - left;
    println!();
    println!("  ╔{}╗", "═".repeat(width));
    println!("  ║{} {} {}║", " ".repeat(left), title, " ".repeat(right));
    println!("  ╚{}╝", "═".repeat(width));
    println!();
}

fn section(title: &str) {
    println!();
    println!(
        "  ┌─── {} {}",
        title,
        "─".repeat(50usize.saturating_sub(title.len()))
    );
    println!("  │");
}

fn end_section() {
    println!("  │");
    println!("  └{}─", "─".repeat(56));
}

fn item(text: &str) {
    println!("  │  ▸ {text}");
}

fn blank() {
    println!("  │");
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_target(false)
        .with_level(true)
        .with_timer(tracing_subscriber::fmt::time::uptime())
        .init();

    banner("CONTEXT KEEPER  —  Long-Term Memory Demo");

    // ── Setup ────────────────────────────────────────────────────────

    section("1 · Setup (in-memory SurrealDB + mock embeddings)");

    let config = SurrealConfig {
        embedding_dimensions: 64,
        ..SurrealConfig::default()
    };
    let db = connect_memory(&config).await?;
    apply_schema(&db, &config).await?;
    let repo = Repository::new(db);
    let embedder = MockEmbedder::new(64);

    item("In-memory SurrealDB initialized with 64-dim mock embeddings");

    end_section();

    // ── Save notes ───────────────────────────────────────────────────

    section("2 · Save several notes with different tags");

    let notes_data = vec![
        (
            "rust-async",
            "Rust async/await uses a poll-based Future model. Tokio is the most popular runtime.",
            vec!["rust", "async", "programming"],
        ),
        (
            "surreal-vectors",
            "SurrealDB supports HNSW vector indexes with configurable distance metrics like cosine and euclidean.",
            vec!["surrealdb", "vectors", "database"],
        ),
        (
            "mcp-protocol",
            "The Model Context Protocol (MCP) enables AI assistants to use external tools via a standardized JSON-RPC interface.",
            vec!["mcp", "ai", "protocol"],
        ),
        (
            "project-decisions",
            "We chose SurrealDB over PostgreSQL for its native graph traversal and built-in vector search. Trade-off: less mature ecosystem.",
            vec!["architecture", "decisions"],
        ),
        (
            "meeting-2026-04",
            "Team decided to prioritize the long-term memory feature before the MCP resource templates. Ship target: end of April.",
            vec!["meeting", "planning"],
        ),
    ];

    for (key, content, tags) in &notes_data {
        let embedding = embedder.embed(content).await?;
        let now = Utc::now();
        let note = Note {
            id: Uuid::new_v4(),
            key: key.to_string(),
            content: content.to_string(),
            embedding,
            tags: tags.iter().map(|s| s.to_string()).collect(),
            namespace: None,
            created_at: now,
            updated_at: now,
        };
        repo.upsert_note(&note).await?;
        item(&format!("[{}] saved ({} tags: {:?})", key, tags.len(), tags));
    }

    end_section();

    // ── Get by key ───────────────────────────────────────────────────

    section("3 · Retrieve a note by key");

    let fetched = repo.get_note_by_key("mcp-protocol", None).await?;
    match &fetched {
        Some(note) => {
            item(&format!("Key     : {}", note.key));
            item(&format!("Content : {:.80}", note.content));
            item(&format!("Tags    : {:?}", note.tags));
        }
        None => item("NOT FOUND (unexpected)"),
    }

    end_section();

    // ── Search by content ────────────────────────────────────────────

    section("4 · Search notes by content similarity");

    let search_query = "vector similarity search database";
    let search_embedding = embedder.embed(search_query).await?;

    item(&format!("Query: \"{search_query}\""));
    blank();

    let results = repo
        .search_notes_by_vector(&search_embedding, 3, None, None)
        .await?;

    for (i, (note, score)) in results.iter().enumerate() {
        item(&format!(
            "  {}. [{}] score: {:.4}  \"{:.60}\"",
            i + 1,
            note.key,
            score,
            note.content,
        ));
    }

    end_section();

    // ── Filter by tags ───────────────────────────────────────────────

    section("5 · List notes filtered by tags");

    let filter_tags = vec!["database".to_string(), "surrealdb".to_string()];
    item(&format!("Filter tags: {:?}", filter_tags));
    blank();

    let filtered = repo.list_notes(Some(&filter_tags), 10, None).await?;

    for note in &filtered {
        item(&format!(
            "  [{}] tags: {:?}  \"{:.60}\"",
            note.key, note.tags, note.content,
        ));
    }

    if filtered.is_empty() {
        item("  (no notes matched the tag filter)");
    }

    end_section();

    // ── Update a note ────────────────────────────────────────────────

    section("6 · Update an existing note (same key)");

    let updated_content =
        "We chose SurrealDB for graph traversal + vector search. Ecosystem is maturing fast — v3 released.";
    let updated_embedding = embedder.embed(updated_content).await?;
    let now = Utc::now();

    let existing = repo
        .get_note_by_key("project-decisions", None)
        .await?
        .unwrap();
    item(&format!("Before: \"{:.70}\"", existing.content));

    let updated_note = Note {
        id: existing.id,
        key: existing.key,
        content: updated_content.to_string(),
        embedding: updated_embedding,
        tags: existing.tags,
        namespace: existing.namespace,
        created_at: existing.created_at,
        updated_at: now,
    };
    repo.upsert_note(&updated_note).await?;

    let after = repo
        .get_note_by_key("project-decisions", None)
        .await?
        .unwrap();
    item(&format!("After : \"{:.70}\"", after.content));
    item(&format!(
        "created_at unchanged: {}",
        after.created_at == existing.created_at
    ));

    end_section();

    // ── Delete a note ────────────────────────────────────────────────

    section("7 · Delete a note");

    let deleted = repo.delete_note("meeting-2026-04", None).await?;
    item(&format!(
        "delete_note(\"meeting-2026-04\"): deleted={}",
        deleted
    ));

    let gone = repo.get_note_by_key("meeting-2026-04", None).await?;
    item(&format!(
        "get_note after delete: {}",
        gone.map(|_| "Some").unwrap_or("None")
    ));

    let second_delete = repo.delete_note("meeting-2026-04", None).await?;
    item(&format!(
        "delete again (idempotent): deleted={}",
        second_delete
    ));

    end_section();

    // ── Final listing ────────────────────────────────────────────────

    section("8 · Final listing of all remaining notes");

    let all_notes = repo.list_notes(None, 20, None).await?;
    item(&format!("{} notes remaining:", all_notes.len()));
    blank();

    for note in &all_notes {
        item(&format!("  [{}] tags: {:?}", note.key, note.tags));
        item(&format!("    \"{:.70}\"", note.content));
    }

    end_section();

    banner("Long-Term Memory Demo Complete");

    println!("  Demonstrated:");
    println!("    1.  In-memory SurrealDB setup with mock embeddings");
    println!("    2.  Saving notes with keys and tags");
    println!("    3.  Retrieving notes by key");
    println!("    4.  Searching notes by content similarity (vector search)");
    println!("    5.  Filtering notes by tags");
    println!("    6.  Updating notes (upsert by key)");
    println!("    7.  Deleting notes");
    println!("    8.  Listing all notes");
    println!();

    Ok(())
}

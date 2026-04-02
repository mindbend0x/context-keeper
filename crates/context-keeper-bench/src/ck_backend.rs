use async_trait::async_trait;
use chrono::Utc;
use context_keeper_core::ingestion;
use context_keeper_core::models::Episode;
use context_keeper_core::search::fuse_rrf;
use context_keeper_core::traits::{Embedder, EntityExtractor, EntityResolver, RelationExtractor};
use context_keeper_rig::embeddings::RigEmbedder;
use context_keeper_rig::extraction::{RigEntityExtractor, RigRelationExtractor};
use context_keeper_rig::rewriting::RigQueryRewriter;
use context_keeper_surreal::{apply_schema, connect_memory, Repository, SurrealConfig};
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::backend::{
    BenchBackend, EntityExtractionOutput, IngestionOutput, QueryRewriteOutput,
    RelationExtractionOutput, SearchOutput,
};
use crate::config::ProviderConfig;

/// Context Keeper backend that delegates to real Rig-based extractors.
///
/// When `init_storage()` is called, an in-memory SurrealDB is spun up so that
/// `ingestion` persists to the graph and `search` runs actual hybrid search.
pub struct ContextKeeperBackend {
    provider_name: String,
    entity_extractor: RigEntityExtractor,
    relation_extractor: RigRelationExtractor,
    embedder: RigEmbedder,
    query_rewriter: RigQueryRewriter,
    repo: Mutex<Option<Repository>>,
    embedding_dims: usize,
    estimated_tokens: AtomicU64,
}

/// Approximate token count from text length (chars / 4 is standard for English + OpenAI BPE).
fn estimate_tokens(text: &str) -> u64 {
    (text.len() as u64 + 3) / 4
}

impl ContextKeeperBackend {
    pub fn from_provider(provider: &ProviderConfig) -> Self {
        let entity_extractor = RigEntityExtractor::new(
            &provider.api_url,
            &provider.api_key,
            &provider.extraction_model,
        );
        let relation_extractor = RigRelationExtractor::new(
            &provider.api_url,
            &provider.api_key,
            &provider.extraction_model,
        );
        let embedder = RigEmbedder::new(
            &provider.api_url,
            &provider.api_key,
            &provider.embedding_model,
            provider.embedding_dims,
        );
        let query_rewriter = RigQueryRewriter::new(
            &provider.api_url,
            &provider.api_key,
            &provider.extraction_model,
        );

        Self {
            provider_name: provider.name.clone(),
            entity_extractor,
            relation_extractor,
            embedder,
            query_rewriter,
            repo: Mutex::new(None),
            embedding_dims: provider.embedding_dims,
            estimated_tokens: AtomicU64::new(0),
        }
    }

    async fn ensure_repo(&self) -> anyhow::Result<()> {
        let mut guard = self.repo.lock().await;
        if guard.is_none() {
            let config = SurrealConfig {
                embedding_dimensions: self.embedding_dims,
                ..SurrealConfig::default()
            };
            let db = connect_memory(&config).await?;
            apply_schema(&db, &config).await?;
            *guard = Some(Repository::new(db));
        }
        Ok(())
    }
}

#[async_trait]
impl BenchBackend for ContextKeeperBackend {
    fn name(&self) -> &str {
        &self.provider_name
    }

    async fn entity_extraction(&self, text: &str) -> anyhow::Result<EntityExtractionOutput> {
        self.estimated_tokens
            .fetch_add(estimate_tokens(text), Ordering::Relaxed);
        let entities = self.entity_extractor.extract_entities(text).await?;
        Ok(EntityExtractionOutput { entities })
    }

    async fn relation_extraction(&self, text: &str) -> anyhow::Result<RelationExtractionOutput> {
        self.estimated_tokens
            .fetch_add(estimate_tokens(text), Ordering::Relaxed);
        let entities = self.entity_extractor.extract_entities(text).await?;
        self.estimated_tokens
            .fetch_add(estimate_tokens(text), Ordering::Relaxed);
        let relations = self
            .relation_extractor
            .extract_relations(text, &entities)
            .await?;
        Ok(RelationExtractionOutput {
            entities,
            relations,
        })
    }

    async fn ingestion(&self, text: &str, source: &str) -> anyhow::Result<IngestionOutput> {
        self.ensure_repo().await?;

        // Entity extraction + relation extraction + embedding = ~3 LLM calls
        self.estimated_tokens
            .fetch_add(estimate_tokens(text) * 3, Ordering::Relaxed);

        let episode = Episode {
            id: Uuid::new_v4(),
            content: text.to_string(),
            source: source.to_string(),
            session_id: None,
            agent: None,
            namespace: None,
            created_at: Utc::now(),
        };

        let guard = self.repo.lock().await;
        let repo = guard.as_ref().unwrap();
        let resolver: &dyn EntityResolver = repo;

        let result = ingestion::ingest(
            &episode,
            &self.embedder,
            &self.entity_extractor,
            &self.relation_extractor,
            Some(resolver),
            None,
            None,
        )
        .await?;

        for inv in &result.diff.entities_invalidated {
            let existing = repo.find_entities_by_name(&inv.name, None, None).await?;
            for entity in existing {
                repo.invalidate_entity(entity.id).await?;
            }
        }
        for entity_id in &result.diff.entity_ids_to_invalidate_relations {
            repo.invalidate_relations_for_entity(*entity_id).await?;
        }

        repo.create_episode(&episode).await?;
        for entity in &result.entities {
            repo.upsert_entity(entity).await?;
        }
        for relation in &result.relations {
            repo.create_relation(relation).await?;
        }
        for memory in &result.memories {
            repo.create_memory(memory).await?;
        }

        Ok(IngestionOutput {
            entity_count: result.entities.len(),
            relation_count: result.relations.len(),
            memory_count: result.memories.len(),
            entities_created: result.diff.entities_created.len(),
            entities_updated: result.diff.entities_updated.len(),
        })
    }

    async fn search(&self, query: &str) -> anyhow::Result<SearchOutput> {
        self.ensure_repo().await?;
        self.estimated_tokens
            .fetch_add(estimate_tokens(query), Ordering::Relaxed);

        let guard = self.repo.lock().await;
        let repo = guard.as_ref().unwrap();

        let query_embedding = self.embedder.embed(query).await?;

        let vector_results = repo
            .search_entities_by_vector(&query_embedding, 10, None, None)
            .await?;
        let keyword_results = repo.search_entities_by_keyword(query, None, None).await?;

        let fused = fuse_rrf(vec![
            vector_results.into_iter().map(|(e, _)| e).collect(),
            keyword_results,
        ]);

        Ok(SearchOutput {
            result_count: fused.len(),
        })
    }

    async fn query_rewrite(&self, query: &str) -> anyhow::Result<QueryRewriteOutput> {
        use context_keeper_core::traits::QueryRewriter;
        self.estimated_tokens
            .fetch_add(estimate_tokens(query), Ordering::Relaxed);
        let variants = self.query_rewriter.rewrite(query).await?;
        Ok(QueryRewriteOutput { variants })
    }

    async fn search_entity_names(&self, query: &str) -> anyhow::Result<Vec<String>> {
        let (names, _) = self.search_with_text(query).await?;
        Ok(names)
    }

    async fn search_with_text(&self, query: &str) -> anyhow::Result<(Vec<String>, String)> {
        self.ensure_repo().await?;
        self.estimated_tokens
            .fetch_add(estimate_tokens(query), Ordering::Relaxed);

        let guard = self.repo.lock().await;
        let repo = guard.as_ref().unwrap();

        let query_embedding = self.embedder.embed(query).await?;

        let vector_results = repo
            .search_entities_by_vector(&query_embedding, 10, None, None)
            .await?;
        let keyword_results = repo.search_entities_by_keyword(query, None, None).await?;

        let fused = fuse_rrf(vec![
            vector_results.into_iter().map(|(e, _)| e).collect(),
            keyword_results,
        ]);

        let mut names = Vec::new();
        let mut text_parts = Vec::new();
        for sr in fused {
            if let Some(e) = sr.entity {
                names.push(e.name.clone());
                text_parts.push(format!("{}: {}", e.name, e.summary));
            }
            if let Some(m) = sr.memory {
                text_parts.push(m.content);
            }
        }

        Ok((names, text_parts.join("\n")))
    }

    async fn reset(&self) -> anyhow::Result<()> {
        self.estimated_tokens.store(0, Ordering::Relaxed);
        let mut guard = self.repo.lock().await;
        if guard.is_some() {
            let config = SurrealConfig {
                embedding_dimensions: self.embedding_dims,
                ..SurrealConfig::default()
            };
            let db = connect_memory(&config).await?;
            apply_schema(&db, &config).await?;
            *guard = Some(Repository::new(db));
        }
        Ok(())
    }

    fn token_count(&self) -> Option<u64> {
        let count = self.estimated_tokens.load(Ordering::Relaxed);
        if count > 0 {
            Some(count)
        } else {
            None
        }
    }
}

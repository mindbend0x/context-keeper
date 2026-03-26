use anyhow::Result;
use chrono::Utc;
use context_keeper_core::{
    ingestion,
    ingestion::IngestionResult,
    models::*,
    search::fuse_rrf,
    traits::*,
};
use context_keeper_surreal::{apply_schema, connect_memory, Repository, SurrealConfig};
use uuid::Uuid;

pub const EMBED_DIM: usize = 8;

pub struct TestEnv {
    pub repo: Repository,
    pub embedder: MockEmbedder,
    pub entity_extractor: MockEntityExtractor,
    pub relation_extractor: MockRelationExtractor,
    pub query_rewriter: MockQueryRewriter,
}

impl TestEnv {
    pub async fn new() -> Result<Self> {
        let config = SurrealConfig {
            embedding_dimensions: EMBED_DIM,
            ..SurrealConfig::default()
        };
        let db = connect_memory(&config).await?;
        apply_schema(&db, &config).await?;
        Ok(Self {
            repo: Repository::new(db),
            embedder: MockEmbedder::new(EMBED_DIM),
            entity_extractor: MockEntityExtractor,
            relation_extractor: MockRelationExtractor,
            query_rewriter: MockQueryRewriter,
        })
    }

    pub async fn ingest_text(&self, text: &str, source: &str) -> Result<IngestionResult> {
        self.ingest_text_with_resolver(text, source, false).await
    }

    pub async fn ingest_text_with_resolver(
        &self,
        text: &str,
        source: &str,
        use_resolver: bool,
    ) -> Result<IngestionResult> {
        let episode = Episode {
            id: Uuid::new_v4(),
            content: text.to_string(),
            source: source.to_string(),
            session_id: None,
            agent: None,
            namespace: None,
            created_at: Utc::now(),
        };

        let resolver: &dyn EntityResolver = &self.repo;
        let result = ingestion::ingest(
            &episode,
            &self.embedder,
            &self.entity_extractor,
            &self.relation_extractor,
            if use_resolver { Some(resolver) } else { None },
            None,
        )
        .await?;

        for inv in &result.diff.entities_invalidated {
            let existing = self.repo.find_entities_by_name(&inv.name, None).await?;
            for entity in existing {
                self.repo.invalidate_entity(entity.id).await?;
            }
        }

        self.repo.create_episode(&episode).await?;
        for entity in &result.entities {
            self.repo.upsert_entity(entity).await?;
        }
        for relation in &result.relations {
            self.repo.create_relation(relation).await?;
        }
        for memory in &result.memories {
            self.repo.create_memory(memory).await?;
        }

        Ok(result)
    }

    pub async fn ingest_batch(&self, texts: &[(&str, &str)]) -> Result<Vec<IngestionResult>> {
        let mut results = Vec::with_capacity(texts.len());
        for (text, source) in texts {
            results.push(self.ingest_text(text, source).await?);
        }
        Ok(results)
    }

    /// Ingest text as a specific agent within a namespace.
    pub async fn ingest_as_agent(
        &self,
        text: &str,
        source: &str,
        agent_id: &str,
        namespace: Option<&str>,
    ) -> Result<IngestionResult> {
        let episode = Episode {
            id: Uuid::new_v4(),
            content: text.to_string(),
            source: source.to_string(),
            session_id: None,
            agent: Some(AgentInfo {
                agent_id: agent_id.to_string(),
                agent_name: Some(agent_id.to_string()),
                machine_id: None,
            }),
            namespace: namespace.map(|s| s.to_string()),
            created_at: Utc::now(),
        };

        let resolver: &dyn EntityResolver = &self.repo;
        let result = ingestion::ingest(
            &episode,
            &self.embedder,
            &self.entity_extractor,
            &self.relation_extractor,
            Some(resolver),
            None,
        )
        .await?;

        for inv in &result.diff.entities_invalidated {
            let existing = self.repo.find_entities_by_name(&inv.name, namespace).await?;
            for entity in existing {
                self.repo.invalidate_entity(entity.id).await?;
            }
        }

        self.repo.create_episode(&episode).await?;
        for entity in &result.entities {
            self.repo.upsert_entity(entity).await?;
        }
        for relation in &result.relations {
            self.repo.create_relation(relation).await?;
        }
        for memory in &result.memories {
            self.repo.create_memory(memory).await?;
        }

        Ok(result)
    }

    pub async fn search_hybrid(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        let query_embedding = self.embedder.embed(query).await?;

        let vector_results = self
            .repo
            .search_entities_by_vector(&query_embedding, limit, None, None)
            .await?;

        let keyword_results = self
            .repo
            .search_entities_by_keyword(query, None, None)
            .await?;

        let fused = fuse_rrf(vec![
            vector_results.into_iter().map(|(e, _)| e).collect(),
            keyword_results,
        ]);

        Ok(fused.into_iter().take(limit).collect())
    }
}

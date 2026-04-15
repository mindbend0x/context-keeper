//! Open storage and construct Rig vs mock extractors (parity with `context-keeper-cli` / MCP).

use std::path::Path;
use std::sync::Arc;

use context_keeper_core::traits::*;
use context_keeper_rig::{
    embeddings::RigEmbedder,
    extraction::{RigEntityExtractor, RigRelationExtractor},
    rewriting::RigQueryRewriter,
};
use context_keeper_surreal::{
    apply_schema, connect, parse_storage_backend, Repository, StorageBackend, SurrealConfig,
};
use tracing::info;

use crate::backend::LocalBackend;

#[allow(clippy::too_many_arguments)]
pub async fn open_local_repository(
    storage: &str,
    db_file_path: &str,
    embedding_dims: usize,
    surreal_user: Option<String>,
    surreal_pass: Option<String>,
    _api_url: Option<&str>,
    _api_key: Option<&str>,
    _embedding_model_name: Option<&str>,
    _extraction_model_name: Option<&str>,
    _embedding_api_url: Option<&str>,
    _embedding_api_key: Option<&str>,
) -> anyhow::Result<Repository> {
    let config = SurrealConfig {
        embedding_dimensions: embedding_dims,
        storage: parse_storage_backend(storage),
        username: surreal_user,
        password: surreal_pass,
        ..SurrealConfig::default()
    };

    if let StorageBackend::RocksDb(ref path) = config.storage {
        if let Some(parent) = Path::new(path).parent() {
            std::fs::create_dir_all(parent).ok();
        }
        std::fs::create_dir_all(path).ok();
    }

    let db = connect(&config).await?;
    apply_schema(&db, &config).await?;
    let repo = Repository::new(db);

    if Path::new(db_file_path).exists() && matches!(config.storage, StorageBackend::Memory) {
        repo.import_from_file(db_file_path).await?;
    }

    Ok(repo)
}

#[allow(clippy::too_many_arguments)]
pub fn build_llm_stack(
    embedding_dims: usize,
    api_url: Option<&str>,
    api_key: Option<&str>,
    embedding_model_name: Option<&str>,
    extraction_model_name: Option<&str>,
    embedding_api_url: Option<&str>,
    embedding_api_key: Option<&str>,
) -> (
    Arc<dyn Embedder>,
    Arc<dyn EntityExtractor>,
    Arc<dyn RelationExtractor>,
    Arc<dyn QueryRewriter>,
) {
    let emb_api_url = embedding_api_url.or(api_url);
    let emb_api_key = embedding_api_key.or(api_key);

    match (
        api_url,
        api_key,
        embedding_model_name,
        extraction_model_name,
    ) {
        (Some(api_url), Some(api_key), Some(emb_model), Some(ext_model)) => {
            let emb_url = emb_api_url.unwrap_or(api_url);
            let emb_key = emb_api_key.unwrap_or(api_key);
            info!(
                extraction_url = api_url,
                embedding_url = emb_url,
                "Using LLM-powered extraction"
            );
            (
                Arc::new(RigEmbedder::new(
                    emb_url,
                    emb_key,
                    emb_model,
                    embedding_dims,
                )),
                Arc::new(RigEntityExtractor::new(api_url, api_key, ext_model)),
                Arc::new(RigRelationExtractor::new(api_url, api_key, ext_model)),
                Arc::new(RigQueryRewriter::new(api_url, api_key, ext_model)),
            )
        }
        _ => {
            info!("No full LLM config — using mock extraction/embeddings");
            (
                Arc::new(MockEmbedder::new(embedding_dims)),
                Arc::new(MockEntityExtractor),
                Arc::new(MockRelationExtractor),
                Arc::new(MockQueryRewriter),
            )
        }
    }
}

/// Wire repository + extractors into a [`LocalBackend`].
#[allow(clippy::too_many_arguments)]
pub async fn build_local_backend(
    storage: &str,
    db_file_path: &str,
    embedding_dims: usize,
    surreal_user: Option<String>,
    surreal_pass: Option<String>,
    namespace: Option<String>,
    agent_id: Option<String>,
    api_url: Option<&str>,
    api_key: Option<&str>,
    embedding_model_name: Option<&str>,
    extraction_model_name: Option<&str>,
    embedding_api_url: Option<&str>,
    embedding_api_key: Option<&str>,
) -> anyhow::Result<LocalBackend> {
    let repo = open_local_repository(
        storage,
        db_file_path,
        embedding_dims,
        surreal_user,
        surreal_pass,
        api_url,
        api_key,
        embedding_model_name,
        extraction_model_name,
        embedding_api_url,
        embedding_api_key,
    )
    .await?;

    let (embedder, entity_extractor, relation_extractor, query_rewriter) = build_llm_stack(
        embedding_dims,
        api_url,
        api_key,
        embedding_model_name,
        extraction_model_name,
        embedding_api_url,
        embedding_api_key,
    );

    Ok(LocalBackend::new(
        repo,
        embedder,
        entity_extractor,
        relation_extractor,
        query_rewriter,
        namespace,
        agent_id,
    ))
}

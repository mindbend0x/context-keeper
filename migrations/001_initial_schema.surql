-- Context Keeper: Initial Schema Migration
-- Creates core tables with vector (HNSW) and full-text (BM25) indexes.

DEFINE TABLE episode SCHEMAFULL;
DEFINE FIELD content    ON episode TYPE string;
DEFINE FIELD source     ON episode TYPE string;
DEFINE FIELD session_id ON episode TYPE option<string>;
DEFINE FIELD created_at ON episode TYPE datetime;

DEFINE TABLE entity SCHEMAFULL;
DEFINE FIELD name        ON entity TYPE string;
DEFINE FIELD entity_type ON entity TYPE string;
DEFINE FIELD summary     ON entity TYPE string;
DEFINE FIELD embedding   ON entity TYPE array<float>;
DEFINE FIELD valid_from  ON entity TYPE datetime;
DEFINE FIELD valid_until ON entity TYPE option<datetime>;

DEFINE INDEX idx_entity_vec ON TABLE entity
  FIELDS embedding HNSW DIMENSION 1536 DIST COSINE;
DEFINE INDEX idx_entity_ft ON TABLE entity
  FIELDS name, summary FULLTEXT ANALYZER simple BM25;

DEFINE TABLE relation SCHEMAFULL;
DEFINE FIELD in          ON relation TYPE record<entity>;
DEFINE FIELD out         ON relation TYPE record<entity>;
DEFINE FIELD rel_type    ON relation TYPE string;
DEFINE FIELD confidence  ON relation TYPE float;
DEFINE FIELD valid_from  ON relation TYPE datetime;
DEFINE FIELD valid_until ON relation TYPE option<datetime>;

DEFINE TABLE memory SCHEMAFULL;
DEFINE FIELD content          ON memory TYPE string;
DEFINE FIELD embedding        ON memory TYPE array<float>;
DEFINE FIELD source_episode   ON memory TYPE record<episode>;
DEFINE FIELD entity_ids       ON memory TYPE array<record<entity>>;
DEFINE FIELD created_at       ON memory TYPE datetime;

DEFINE INDEX idx_memory_vec ON TABLE memory
  FIELDS embedding HNSW DIMENSION 1536 DIST COSINE;
DEFINE INDEX idx_memory_ft ON TABLE memory
  FIELDS content FULLTEXT ANALYZER simple BM25;

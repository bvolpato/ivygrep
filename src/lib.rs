pub mod chunking;
pub mod cli;
pub mod config;
pub mod daemon;
pub mod doctor;
pub mod embedding;
pub mod indexer;
pub mod ipc;
pub mod jobs;
pub mod mcp;
pub mod merkle;
pub mod path_glob;
pub mod protocol;
pub(crate) mod query_aliases;
pub mod regex_search;
pub(crate) mod reranker;
pub mod search;
pub mod symbols;
pub mod text;
pub mod tui;
pub mod vector_store;
pub mod walker;
pub mod workspace;

/// Legacy constant kept for tests. Prefer [`embedding::model_dimensions`].
pub const EMBEDDING_DIMENSIONS: usize = 256;

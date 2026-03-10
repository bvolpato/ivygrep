pub mod chunking;
pub mod cli;
pub mod config;
pub mod daemon;
pub mod embedding;
pub mod indexer;
pub mod mcp;
pub mod merkle;
pub mod protocol;
pub mod regex_search;
pub mod search;
pub mod vector_store;
pub mod workspace;

pub const EMBEDDING_DIMENSIONS: usize = 256;

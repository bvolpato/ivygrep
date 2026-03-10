use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub enum ChunkKind {
    Function,
    Class,
    Module,
    Text,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chunk {
    pub id: Uuid,
    pub file_path: PathBuf,
    pub start_line: usize,
    pub end_line: usize,
    pub text: String,
    pub language: String,
    pub kind: ChunkKind,
    pub content_hash: String,
}

pub fn language_for_path(path: &Path) -> Option<&'static str> {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("rs") => Some("rust"),
        Some("py") => Some("python"),
        Some("ts") | Some("tsx") => Some("typescript"),
        Some("js") | Some("jsx") => Some("javascript"),
        Some("java") => Some("java"),
        Some("go") => Some("go"),
        Some("rb") => Some("ruby"),
        Some("md") => Some("markdown"),
        Some("toml") | Some("yaml") | Some("yml") => Some("config"),
        _ => None,
    }
}

pub fn is_indexable_path(path: &Path) -> bool {
    language_for_path(path).is_some()
}

pub fn chunk_source(rel_path: &Path, text: &str) -> Vec<Chunk> {
    let language = language_for_path(rel_path).unwrap_or("text").to_string();
    let lines: Vec<&str> = text.lines().collect();

    if lines.is_empty() {
        return vec![];
    }

    let signatures = collect_signatures(&language, &lines);

    if signatures.is_empty() {
        return fallback_chunks(rel_path, &language, &lines);
    }

    let mut chunks = Vec::new();
    for (idx, (start_line, kind)) in signatures.iter().enumerate() {
        let start = *start_line;
        let end = signatures
            .get(idx + 1)
            .map(|(next, _)| next.saturating_sub(1))
            .unwrap_or(lines.len());

        if end < start {
            continue;
        }

        let block = lines[start.saturating_sub(1)..end].join("\n");
        let text = format!("// {}\n\n{}", rel_path.to_string_lossy(), block);

        chunks.push(make_chunk(
            rel_path,
            start,
            end,
            text,
            language.clone(),
            kind.clone(),
        ));
    }

    chunks
}

fn collect_signatures(language: &str, lines: &[&str]) -> Vec<(usize, ChunkKind)> {
    let mut out = Vec::new();

    for (idx, line) in lines.iter().enumerate() {
        let line_no = idx + 1;
        let trimmed = line.trim_start();

        let maybe_kind = match language {
            "rust" => {
                if trimmed.starts_with("fn ")
                    || trimmed.starts_with("pub fn ")
                    || trimmed.starts_with("pub(crate) fn ")
                {
                    Some(ChunkKind::Function)
                } else if trimmed.starts_with("struct ")
                    || trimmed.starts_with("pub struct ")
                    || trimmed.starts_with("enum ")
                    || trimmed.starts_with("pub enum ")
                    || trimmed.starts_with("trait ")
                    || trimmed.starts_with("pub trait ")
                {
                    Some(ChunkKind::Class)
                } else if trimmed.starts_with("impl ") || trimmed.starts_with("mod ") {
                    Some(ChunkKind::Module)
                } else {
                    None
                }
            }
            "python" => {
                if trimmed.starts_with("def ") || trimmed.starts_with("async def ") {
                    Some(ChunkKind::Function)
                } else if trimmed.starts_with("class ") {
                    Some(ChunkKind::Class)
                } else {
                    None
                }
            }
            "typescript" | "javascript" => {
                if trimmed.starts_with("function ")
                    || trimmed.contains(" => ")
                    || trimmed.starts_with("export function ")
                {
                    Some(ChunkKind::Function)
                } else if trimmed.starts_with("class ") || trimmed.starts_with("export class ") {
                    Some(ChunkKind::Class)
                } else {
                    None
                }
            }
            "java" => {
                if trimmed.contains(" class ") || trimmed.starts_with("class ") {
                    Some(ChunkKind::Class)
                } else if trimmed.contains("(") && trimmed.ends_with('{') {
                    Some(ChunkKind::Function)
                } else {
                    None
                }
            }
            "go" => {
                if trimmed.starts_with("func ") {
                    Some(ChunkKind::Function)
                } else if trimmed.starts_with("type ") {
                    Some(ChunkKind::Class)
                } else {
                    None
                }
            }
            _ => None,
        };

        if let Some(kind) = maybe_kind {
            out.push((line_no, kind));
        }
    }

    out
}

fn fallback_chunks(rel_path: &Path, language: &str, lines: &[&str]) -> Vec<Chunk> {
    let window = 80usize;
    let overlap = 20usize;

    let mut chunks = Vec::new();
    let mut start = 1usize;

    while start <= lines.len() {
        let end = (start + window - 1).min(lines.len());
        let block = lines[start - 1..end].join("\n");
        let text = format!("// {}\n\n{}", rel_path.to_string_lossy(), block);

        chunks.push(make_chunk(
            rel_path,
            start,
            end,
            text,
            language.to_string(),
            ChunkKind::Text,
        ));

        if end == lines.len() {
            break;
        }

        start = end.saturating_sub(overlap) + 1;
    }

    chunks
}

fn make_chunk(
    rel_path: &Path,
    start_line: usize,
    end_line: usize,
    text: String,
    language: String,
    kind: ChunkKind,
) -> Chunk {
    let mut hasher = Sha256::new();
    hasher.update(rel_path.to_string_lossy().as_bytes());
    hasher.update(start_line.to_le_bytes());
    hasher.update(end_line.to_le_bytes());
    hasher.update(text.as_bytes());
    let content_hash = hex::encode(hasher.finalize());

    Chunk {
        id: Uuid::new_v4(),
        file_path: rel_path.to_path_buf(),
        start_line,
        end_line,
        text,
        language,
        kind,
        content_hash,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rust_chunker_extracts_functions() {
        let src = r#"
pub fn calculate_tax(amount: f64) -> f64 {
    amount * 0.2
}

pub fn calculate_total(amount: f64) -> f64 {
    amount + calculate_tax(amount)
}
"#;

        let chunks = chunk_source(Path::new("src/tax.rs"), src);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].kind, ChunkKind::Function);
        assert!(chunks[0].text.contains("calculate_tax"));
    }

    #[test]
    fn fallback_chunker_splits_large_text() {
        let src = (0..250)
            .map(|i| format!("line_{i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let chunks = chunk_source(Path::new("README.md"), &src);
        assert!(chunks.len() > 1);
        assert!(chunks.iter().all(|c| c.kind == ChunkKind::Text));
    }
}
